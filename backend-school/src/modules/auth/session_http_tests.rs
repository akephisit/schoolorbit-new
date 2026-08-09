use std::{net::SocketAddr, sync::Arc};

use axum::{
    body::{to_bytes, Body},
    http::{header, HeaderMap, HeaderValue, Request, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Extension, Router,
};
use chrono::{Duration, Utc};
use sqlx::PgPool;
use tokio::sync::broadcast;
use tower::ServiceExt;
use uuid::Uuid;

use crate::{
    db::{
        admin_client::{AdminClient, AdminClientConfig},
        permission_cache::PermissionCache,
        pool_manager::PoolManager,
    },
    test_helpers::{create_named_test_pool, run_test_migrations},
    utils::{
        subdomain::{parse_realtime_tenant_hint, TenantOriginPolicy},
        tenant::TenantContext,
    },
};

use super::{
    config::SessionConfig,
    http::{expire_auth_cookies, presented_session_token, set_session_cookie, validate_csrf},
    models::{CurrentUserResponse, LoginRequest},
    runtime::AuthRuntime,
    session_crypto::{identifier_bucket, session_csrf_token, RawSessionToken, SessionHmacKey},
    session_handlers::{
        change_password, list_sessions, login_with_tenant, logout_all, logout_with_tenant, me,
        revoke_session,
    },
    session_policy::normalize_login_identifier,
    session_service::AuthenticatedSession,
};

struct HandlerFixture {
    runtime: AuthRuntime,
    tenant: TenantContext,
    pool: PgPool,
    now: chrono::DateTime<Utc>,
}

impl HandlerFixture {
    async fn new(test_name: &str) -> Self {
        let pool = create_named_test_pool(test_name).await;
        run_test_migrations(&pool).await;
        let config = Arc::new(SessionConfig::for_tests(SessionHmacKey::for_tests(
            [31; 32],
        )));
        let tenant = TenantContext {
            tenant_id: Uuid::new_v4(),
            subdomain: format!("{test_name}-school"),
            pool: pool.clone(),
        };
        let (session_events, _) = broadcast::channel(32);
        let runtime = AuthRuntime {
            admin_client: Arc::new(AdminClient::new(
                "http://127.0.0.1:9".to_string(),
                "test-secret".to_string(),
                AdminClientConfig::from_env().expect("default admin client config must be valid"),
            )),
            pool_manager: Arc::new(PoolManager::new()),
            permission_cache: Arc::new(PermissionCache::new()),
            config,
            session_events,
        };

        Self {
            runtime,
            tenant,
            pool,
            now: Utc::now(),
        }
    }

    async fn insert_user(&self, username: &str, password: &str) -> Uuid {
        let password = password.to_string();
        let password_hash = tokio::task::spawn_blocking(move || bcrypt::hash(password, 4))
            .await
            .expect("test password task must join")
            .expect("test password must hash");
        sqlx::query_scalar(
            "INSERT INTO users \
             (username, email, password_hash, first_name, last_name, user_type, status) \
             VALUES ($1, $2, $3, 'Test', 'Teacher', 'staff', 'active') RETURNING id",
        )
        .bind(username)
        .bind(format!("{username}@example.test"))
        .bind(password_hash)
        .fetch_one(&self.pool)
        .await
        .expect("test user must insert")
    }

    async fn insert_session(
        &self,
        user_id: Uuid,
        session_id: Uuid,
        token_byte: u8,
    ) -> RawSessionToken {
        let token = RawSessionToken::from_bytes([token_byte; 32]);
        sqlx::query(
            "INSERT INTO auth_sessions (\
                 id, user_id, current_token_hash, remember_me, device_label, created_at, \
                 last_seen_at, idle_expires_at, absolute_expires_at, rotated_at\
             ) VALUES ($1, $2, $3, false, 'Test browser', $4, $4, $5, $6, $4)",
        )
        .bind(session_id)
        .bind(user_id)
        .bind(token.token_hash().as_bytes().as_slice())
        .bind(self.now)
        .bind(self.now + Duration::hours(2))
        .bind(self.now + Duration::hours(12))
        .execute(&self.pool)
        .await
        .expect("test session must insert");
        token
    }

    fn authenticated(
        &self,
        user_id: Uuid,
        session_id: Uuid,
        username: &str,
    ) -> AuthenticatedSession {
        AuthenticatedSession {
            tenant: self.tenant.clone(),
            session_id,
            user_id,
            username: username.to_string(),
            user_type: "staff".to_string(),
        }
    }
}

fn auth_headers(fixture: &HandlerFixture, session_id: Uuid, token: &RawSessionToken) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::COOKIE,
        HeaderValue::from_str(&format!(
            "__Host-schoolorbit_session={}",
            token.encode().expose_for_cookie()
        ))
        .expect("test cookie must be valid"),
    );
    let csrf = session_csrf_token(
        fixture.runtime.config.hmac_key(),
        fixture.tenant.tenant_id,
        session_id,
    );
    headers.insert(
        "x-csrf-token",
        HeaderValue::from_str(&csrf.expose_for_header()).expect("test CSRF must be valid"),
    );
    headers
}

fn response_from_result(result: Result<Response, crate::error::AppError>) -> Response {
    result.unwrap_or_else(IntoResponse::into_response)
}

fn set_cookie_count(response: &Response) -> usize {
    response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .count()
}

#[test]
fn session_cookie_has_host_prefix_and_required_attributes() {
    let header = set_session_cookie("opaque-value", None);
    let session = header.to_str().expect("session cookie must be ASCII");
    assert!(session.starts_with("__Host-schoolorbit_session=opaque-value;"));
    for attribute in ["HttpOnly", "SameSite=Lax", "Secure", "Path=/"] {
        assert!(session.split("; ").any(|part| part == attribute));
    }
    assert!(!session.contains("Max-Age="));
    assert!(!session.contains("Domain="));

    let remembered = set_session_cookie("opaque-value", Some(2_592_000))
        .to_str()
        .expect("remembered cookie must be ASCII")
        .to_string();
    assert!(remembered.contains("Max-Age=2592000"));
    assert!(!remembered.contains("Domain="));

    let near_absolute = set_session_cookie("opaque-value", Some(86_399))
        .to_str()
        .expect("replacement cookie must be ASCII")
        .to_string();
    assert!(near_absolute.contains("Max-Age=86399"));
    assert!(!near_absolute.contains("Max-Age=2592000"));
}

#[test]
fn logout_expires_new_and_legacy_cookies() {
    let headers = expire_auth_cookies();
    assert_eq!(headers.len(), 2);
    assert!(headers.iter().any(|value| {
        value
            .to_str()
            .expect("session expiry must be ASCII")
            .starts_with("__Host-schoolorbit_session=")
    }));
    assert!(headers.iter().any(|value| {
        value
            .to_str()
            .expect("legacy expiry must be ASCII")
            .starts_with("auth_token=")
    }));
    assert!(headers.iter().all(|value| {
        value
            .to_str()
            .expect("cookie expiry must be ASCII")
            .contains("Max-Age=0")
    }));

    let session_expiry = headers
        .iter()
        .find(|value| {
            value
                .to_str()
                .expect("session expiry must be ASCII")
                .starts_with("__Host-schoolorbit_session=")
        })
        .expect("session expiry must be present")
        .to_str()
        .expect("session expiry must be ASCII");
    assert!(session_expiry.contains("Secure"));
    assert!(session_expiry.contains("Path=/"));
    assert!(!session_expiry.contains("Domain="));
}

#[test]
fn unsafe_origin_must_equal_the_resolved_tenant_origin() {
    let policy = TenantOriginPolicy::for_tests("schoolorbit.app", []);
    assert!(policy
        .validate("https://demo.schoolorbit.app", "demo")
        .is_ok());
    assert!(policy
        .validate("https://other.schoolorbit.app", "demo")
        .is_err());
    assert!(policy
        .validate("https://demo.schoolorbit.app.evil.test", "demo")
        .is_err());
}

#[test]
fn current_user_json_has_no_default_pii() {
    let value = serde_json::to_value(CurrentUserResponse {
        id: Uuid::nil(),
        username: "teacher".to_string(),
        first_name: "Test".to_string(),
        last_name: "Teacher".to_string(),
        user_type: "staff".to_string(),
        status: "active".to_string(),
        primary_role_name: Some("Teacher".to_string()),
        profile_image_file_id: None,
        permissions: vec!["academic.read".to_string()],
    })
    .expect("current user response must serialize");

    for forbidden in [
        "nationalId",
        "email",
        "phone",
        "dateOfBirth",
        "address",
        "createdAt",
    ] {
        assert!(
            value.get(forbidden).is_none(),
            "unexpected field {forbidden}"
        );
    }
}

#[test]
fn session_cookie_parser_rejects_duplicate_identity_and_ignores_legacy_identity() {
    let token = RawSessionToken::from_bytes([7; 32]);
    let encoded = token.encode();

    let mut valid = HeaderMap::new();
    valid.append(
        header::COOKIE,
        HeaderValue::from_str(&format!(
            "auth_token=legacy.jwt; __Host-schoolorbit_session={}",
            encoded.expose_for_cookie()
        ))
        .expect("cookie fixture must be valid"),
    );
    let parsed = presented_session_token(&valid)
        .expect("one opaque cookie must parse")
        .expect("opaque cookie must be present");
    assert_eq!(parsed.token_hash(), token.token_hash());

    let mut same_header_duplicate = HeaderMap::new();
    same_header_duplicate.append(
        header::COOKIE,
        HeaderValue::from_str(&format!(
            "__Host-schoolorbit_session={0}; __Host-schoolorbit_session={0}",
            encoded.expose_for_cookie()
        ))
        .expect("cookie fixture must be valid"),
    );
    assert!(presented_session_token(&same_header_duplicate).is_err());

    let mut split_duplicate = HeaderMap::new();
    for _ in 0..2 {
        split_duplicate.append(
            header::COOKIE,
            HeaderValue::from_str(&format!(
                "__Host-schoolorbit_session={}",
                encoded.expose_for_cookie()
            ))
            .expect("cookie fixture must be valid"),
        );
    }
    assert!(presented_session_token(&split_duplicate).is_err());

    let malformed = HeaderMap::from_iter([(
        header::COOKIE,
        HeaderValue::from_static("broken; also-broken=; auth_token=legacy.jwt"),
    )]);
    assert!(presented_session_token(&malformed)
        .expect("malformed unrelated pairs must not panic")
        .is_none());

    let malformed_identity = HeaderMap::from_iter([(
        header::COOKIE,
        HeaderValue::from_str(&format!(
            "__Host-schoolorbit_session= {}",
            encoded.expose_for_cookie()
        ))
        .expect("malformed cookie fixture must be representable"),
    )]);
    assert!(presented_session_token(&malformed_identity)
        .expect("one malformed identity cookie must be treated as absent")
        .is_none());

    let mut mixed = valid.clone();
    mixed.append(
        header::COOKIE,
        HeaderValue::from_str(&format!(
            "__Host-schoolorbit_session ={}",
            encoded.expose_for_cookie()
        ))
        .expect("ambiguous cookie fixture must be representable"),
    );
    assert!(presented_session_token(&mixed).is_err());
}

#[test]
fn authoritative_headers_and_realtime_hint_reject_ambiguity() {
    let policy = TenantOriginPolicy::for_tests("schoolorbit.app", ["http://localhost:5173"]);

    for name in ["origin", "referer", "x-school-subdomain"] {
        let mut headers = HeaderMap::new();
        headers.append(
            name,
            HeaderValue::from_static("https://demo.schoolorbit.app"),
        );
        headers.append(
            name,
            HeaderValue::from_static("https://demo.schoolorbit.app"),
        );
        assert!(policy.resolve_tenant(&headers, None).is_err(), "{name}");
    }

    let mut development = HeaderMap::new();
    development.insert("origin", HeaderValue::from_static("http://localhost:5173"));
    development.insert("x-school-subdomain", HeaderValue::from_static("demo"));
    assert_eq!(
        policy
            .resolve_tenant(&development, None)
            .expect("allowlisted development origin must accept a validated hint"),
        "demo"
    );

    assert_eq!(
        parse_realtime_tenant_hint(Some("other=value&school_subdomain=demo"))
            .expect("one valid query hint must parse"),
        Some("demo".to_string())
    );
    for query in [
        "school_subdomain=",
        "school_subdomain=bad_name",
        "school_subdomain=demo&school_subdomain=other",
        "school_subdomain=%FF",
    ] {
        assert!(parse_realtime_tenant_hint(Some(query)).is_err(), "{query}");
    }
}

#[test]
fn duplicate_csrf_headers_are_rejected_without_selecting_a_value() {
    let key =
        SessionHmacKey::from_secret("session-http-tests-use-a-long-stable-secret-key-material")
            .expect("test HMAC key must be valid");
    let expected = session_csrf_token(&key, Uuid::nil(), Uuid::from_u128(1));
    let encoded = expected.expose_for_header();
    let mut headers = HeaderMap::new();
    headers.append(
        "x-csrf-token",
        HeaderValue::from_str(&encoded).expect("CSRF fixture must be valid"),
    );
    headers.append(
        "x-csrf-token",
        HeaderValue::from_str(&encoded).expect("CSRF fixture must be valid"),
    );

    assert!(validate_csrf(&headers, &expected).is_err());
}

#[tokio::test]
async fn login_errors_use_generic_envelopes_and_retry_after() {
    let fixture = HandlerFixture::new("session_http_login_errors").await;
    let peer: SocketAddr = "203.0.113.10:443".parse().expect("test peer must parse");
    let headers = HeaderMap::new();
    let username = "missing-handler-user";

    let unauthorized = response_from_result(
        login_with_tenant(
            &fixture.runtime,
            fixture.tenant.clone(),
            peer,
            &headers,
            LoginRequest {
                username: username.to_string(),
                password: "wrong-password".to_string(),
                remember_me: Some(false),
            },
        )
        .await,
    );
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(set_cookie_count(&unauthorized), 0);
    let unauthorized_body = String::from_utf8(
        to_bytes(unauthorized.into_body(), usize::MAX)
            .await
            .expect("error body must be readable")
            .to_vec(),
    )
    .expect("error body must be UTF-8");
    assert!(!unauthorized_body.contains(username));
    assert!(!unauthorized_body.contains("wrong-password"));

    let identifier = identifier_bucket(
        fixture.runtime.config.hmac_key(),
        fixture.tenant.tenant_id,
        &normalize_login_identifier(username),
    );
    sqlx::query(
        r#"
        INSERT INTO auth_login_throttles (
            bucket_kind, bucket_hash, failure_count, window_started_at, blocked_until, updated_at
        )
        VALUES ('identifier', $1, 5, $2, $3, $2)
        ON CONFLICT (bucket_kind, bucket_hash) DO UPDATE
        SET failure_count = 5,
            window_started_at = EXCLUDED.window_started_at,
            blocked_until = EXCLUDED.blocked_until,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(identifier.as_bytes().as_slice())
    .bind(fixture.now)
    .bind(fixture.now + Duration::seconds(10))
    .execute(&fixture.pool)
    .await
    .expect("throttle fixture must insert");

    let limited = response_from_result(
        login_with_tenant(
            &fixture.runtime,
            fixture.tenant.clone(),
            peer,
            &headers,
            LoginRequest {
                username: username.to_string(),
                password: "still-wrong".to_string(),
                remember_me: Some(false),
            },
        )
        .await,
    );
    assert_eq!(limited.status(), StatusCode::TOO_MANY_REQUESTS);
    assert!(limited.headers().get(header::RETRY_AFTER).is_some());
    assert_eq!(set_cookie_count(&limited), 0);

    let successful_username = "successful-handler-user";
    fixture
        .insert_user(successful_username, "ValidPassword9!")
        .await;
    let successful = response_from_result(
        login_with_tenant(
            &fixture.runtime,
            fixture.tenant.clone(),
            peer,
            &headers,
            LoginRequest {
                username: successful_username.to_string(),
                password: "ValidPassword9!".to_string(),
                remember_me: Some(true),
            },
        )
        .await,
    );
    assert_eq!(successful.status(), StatusCode::OK);
    assert_eq!(set_cookie_count(&successful), 2);
    let session_cookie = successful
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .find(|value| {
            value
                .to_str()
                .expect("login cookie must be ASCII")
                .starts_with("__Host-schoolorbit_session=")
        })
        .expect("login must set the opaque session cookie")
        .to_str()
        .expect("session cookie must be ASCII")
        .to_string();
    let csrf = successful
        .headers()
        .get("x-csrf-token")
        .expect("login must expose CSRF")
        .to_str()
        .expect("CSRF must be ASCII")
        .to_string();
    let successful_body = String::from_utf8(
        to_bytes(successful.into_body(), usize::MAX)
            .await
            .expect("login body must be readable")
            .to_vec(),
    )
    .expect("login body must be UTF-8");
    assert!(!successful_body.contains(&csrf));
    let credential = session_cookie
        .split(';')
        .next()
        .and_then(|pair| pair.split_once('='))
        .map(|(_, value)| value)
        .expect("session cookie must contain a credential value");
    assert!(!successful_body.contains(credential));
}

#[tokio::test]
async fn logout_expires_credentials_only_after_revocation_can_commit() {
    let fixture = HandlerFixture::new("session_http_logout_commit").await;
    let user_id = fixture
        .insert_user("logout-handler", "CurrentPassword9!")
        .await;
    let session_id = Uuid::new_v4();
    let token = fixture.insert_session(user_id, session_id, 41).await;
    let headers = auth_headers(&fixture, session_id, &token);

    let malformed_headers = HeaderMap::from_iter([(
        header::COOKIE,
        HeaderValue::from_static("__Host-schoolorbit_session=malformed"),
    )]);
    let malformed = response_from_result(
        logout_with_tenant(&fixture.runtime, fixture.tenant.clone(), &malformed_headers).await,
    );
    assert_eq!(malformed.status(), StatusCode::OK);
    assert_eq!(set_cookie_count(&malformed), 2);

    let stale_token = RawSessionToken::from_bytes([99; 32]);
    let stale_headers = auth_headers(&fixture, Uuid::new_v4(), &stale_token);
    let stale = response_from_result(
        logout_with_tenant(&fixture.runtime, fixture.tenant.clone(), &stale_headers).await,
    );
    assert_eq!(stale.status(), StatusCode::OK);
    assert_eq!(set_cookie_count(&stale), 2);

    let mut duplicate_headers = headers.clone();
    duplicate_headers.append(
        header::COOKIE,
        HeaderValue::from_str(&format!(
            "__Host-schoolorbit_session={}",
            token.encode().expose_for_cookie()
        ))
        .expect("duplicate cookie fixture must be valid"),
    );
    let duplicate = response_from_result(
        logout_with_tenant(&fixture.runtime, fixture.tenant.clone(), &duplicate_headers).await,
    );
    assert_eq!(duplicate.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(set_cookie_count(&duplicate), 0);

    let success = response_from_result(
        logout_with_tenant(&fixture.runtime, fixture.tenant.clone(), &headers).await,
    );
    assert_eq!(success.status(), StatusCode::OK);
    assert_eq!(set_cookie_count(&success), 2);
    assert!(success
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .all(|value| value
            .to_str()
            .expect("expiry must be ASCII")
            .contains("Max-Age=0")));

    let unavailable_session_id = Uuid::new_v4();
    let unavailable_token = fixture
        .insert_session(user_id, unavailable_session_id, 42)
        .await;
    let unavailable_headers = auth_headers(&fixture, unavailable_session_id, &unavailable_token);
    fixture.pool.close().await;

    let unavailable = response_from_result(
        logout_with_tenant(
            &fixture.runtime,
            fixture.tenant.clone(),
            &unavailable_headers,
        )
        .await,
    );
    assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(set_cookie_count(&unavailable), 0);
}

#[tokio::test]
async fn protected_mutation_headers_follow_committed_results() {
    let fixture = HandlerFixture::new("session_http_mutation_headers").await;
    let username = "mutation-handler";
    let user_id = fixture.insert_user(username, "CurrentPassword9!").await;
    let foreign_user_id = fixture
        .insert_user("foreign-handler", "ForeignPassword9!")
        .await;
    let current_id = Uuid::new_v4();
    fixture.insert_session(user_id, current_id, 51).await;
    let other_id = Uuid::new_v4();
    fixture.insert_session(user_id, other_id, 52).await;
    let foreign_id = Uuid::new_v4();
    fixture
        .insert_session(foreign_user_id, foreign_id, 53)
        .await;
    let session = fixture.authenticated(user_id, current_id, username);

    let app = Router::new()
        .route("/me", get(me))
        .route("/sessions", get(list_sessions))
        .route("/sessions/{id}", delete(revoke_session))
        .route("/logout-all", post(logout_all))
        .route("/change-password", post(change_password))
        .layer(Extension(session.clone()))
        .with_state(fixture.runtime.clone());

    let current_user = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/me")
                .body(Body::empty())
                .expect("current-user request must build"),
        )
        .await
        .expect("current-user response must resolve");
    assert_eq!(current_user.status(), StatusCode::OK);
    let current_user: serde_json::Value = serde_json::from_slice(
        &to_bytes(current_user.into_body(), usize::MAX)
            .await
            .expect("current-user body must be readable"),
    )
    .expect("current-user body must be JSON");
    for forbidden in [
        "nationalId",
        "email",
        "phone",
        "dateOfBirth",
        "address",
        "createdAt",
    ] {
        assert!(current_user["data"][forbidden].is_null(), "{forbidden}");
    }

    let listed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/sessions")
                .body(Body::empty())
                .expect("session list request must build"),
        )
        .await
        .expect("session list response must resolve");
    assert_eq!(listed.status(), StatusCode::OK);
    assert_eq!(set_cookie_count(&listed), 0);
    let listed: serde_json::Value = serde_json::from_slice(
        &to_bytes(listed.into_body(), usize::MAX)
            .await
            .expect("session list body must be readable"),
    )
    .expect("session list body must be JSON");
    let sessions = listed["data"]["sessions"]
        .as_array()
        .expect("session list must contain an array");
    assert_eq!(sessions.len(), 2);
    assert_eq!(
        sessions
            .iter()
            .filter(|session| session["isCurrent"] == true)
            .count(),
        1
    );
    assert!(sessions.iter().any(|session| {
        session["id"] == current_id.to_string() && session["isCurrent"] == true
    }));

    let foreign = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/sessions/{foreign_id}"))
                .body(Body::empty())
                .expect("foreign revoke request must build"),
        )
        .await
        .expect("foreign revoke response must resolve");
    assert_eq!(foreign.status(), StatusCode::NOT_FOUND);
    assert_eq!(set_cookie_count(&foreign), 0);

    let malformed_password = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/change-password")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{"))
                .expect("malformed password request must build"),
        )
        .await
        .expect("malformed password response must resolve");
    assert_eq!(malformed_password.status(), StatusCode::BAD_REQUEST);
    assert_eq!(set_cookie_count(&malformed_password), 0);
    assert!(malformed_password.headers().get("x-csrf-token").is_none());

    let password_failure = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/change-password")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "currentPassword": "wrong-password",
                        "newPassword": "NewPassword9!"
                    }))
                    .expect("password request must serialize"),
                ))
                .expect("password failure request must build"),
        )
        .await
        .expect("password failure response must resolve");
    assert_eq!(password_failure.status(), StatusCode::BAD_REQUEST);
    assert_eq!(set_cookie_count(&password_failure), 0);
    assert!(password_failure.headers().get("x-csrf-token").is_none());

    let password_success = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/change-password")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    serde_json::to_vec(&serde_json::json!({
                        "currentPassword": "CurrentPassword9!",
                        "newPassword": "NewPassword9!"
                    }))
                    .expect("password request must serialize"),
                ))
                .expect("password success request must build"),
        )
        .await
        .expect("password success response must resolve");
    assert_eq!(password_success.status(), StatusCode::OK);
    assert_eq!(set_cookie_count(&password_success), 1);
    let replacement_cookie = password_success
        .headers()
        .get(header::SET_COOKIE)
        .expect("password success must set replacement cookie")
        .to_str()
        .expect("replacement cookie must be ASCII")
        .to_string();
    assert!(replacement_cookie.starts_with("__Host-schoolorbit_session="));
    assert!(!replacement_cookie.contains("Max-Age=0"));
    let csrf = password_success
        .headers()
        .get("x-csrf-token")
        .expect("password success must expose CSRF")
        .to_str()
        .expect("CSRF must be ASCII")
        .to_string();
    let password_body = String::from_utf8(
        to_bytes(password_success.into_body(), usize::MAX)
            .await
            .expect("password response body must be readable")
            .to_vec(),
    )
    .expect("password response body must be UTF-8");
    assert!(!password_body.contains(&csrf));
    assert!(!password_body.contains("__Host-schoolorbit_session"));

    let current_revoke = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/sessions/{current_id}"))
                .body(Body::empty())
                .expect("current revoke request must build"),
        )
        .await
        .expect("current revoke response must resolve");
    assert_eq!(current_revoke.status(), StatusCode::OK);
    assert_eq!(set_cookie_count(&current_revoke), 2);

    let logout_all_id = Uuid::new_v4();
    fixture.insert_session(user_id, logout_all_id, 54).await;
    let logout_all_app = Router::new()
        .route("/logout-all", post(logout_all))
        .layer(Extension(fixture.authenticated(
            user_id,
            logout_all_id,
            username,
        )))
        .with_state(fixture.runtime.clone());
    let logout_all_response = logout_all_app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/logout-all")
                .body(Body::empty())
                .expect("logout-all request must build"),
        )
        .await
        .expect("logout-all response must resolve");
    assert_eq!(logout_all_response.status(), StatusCode::OK);
    assert_eq!(set_cookie_count(&logout_all_response), 2);

    let unavailable_id = Uuid::new_v4();
    fixture.insert_session(user_id, unavailable_id, 55).await;
    let unavailable_app = Router::new()
        .route("/logout-all", post(logout_all))
        .layer(Extension(fixture.authenticated(
            user_id,
            unavailable_id,
            username,
        )))
        .with_state(fixture.runtime.clone());
    fixture.pool.close().await;
    let unavailable = unavailable_app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/logout-all")
                .body(Body::empty())
                .expect("unavailable logout-all request must build"),
        )
        .await
        .expect("unavailable logout-all response must resolve");
    assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(set_cookie_count(&unavailable), 0);
}

mod protected_router {
    use std::{
        collections::HashMap,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };

    use axum::{
        body::{Body, Bytes},
        extract::{rejection::JsonRejection, DefaultBodyLimit, Extension, Path, State},
        http::{header, HeaderValue, Method, Request, StatusCode},
        middleware::from_fn_with_state,
        response::{IntoResponse, Response},
        routing::{delete, get, patch, post, put},
        Json, Router,
    };
    use chrono::{Duration, Utc};
    use serde_json::json;
    use tokio::{net::TcpListener, sync::broadcast, task::JoinHandle};
    use tower::ServiceExt;
    use uuid::Uuid;

    use crate::{
        app::{APPLICATION_BODY_LIMIT, AUTH_JSON_BODY_LIMIT},
        db::{
            admin_client::{AdminClient, AdminClientConfig},
            permission_cache::PermissionCache,
            pool_manager::PoolManager,
        },
        middleware::session::{maintenance_mode, session_middleware},
        modules::auth::{
            config::SessionConfig,
            runtime::AuthRuntime,
            session_crypto::{session_csrf_token, RawSessionToken, SessionHmacKey},
            session_service::AuthenticatedSession,
        },
        test_helpers::{create_named_test_pool, run_test_migrations},
    };

    use super::super::session_repository::SessionMaintenanceMode;

    #[derive(Clone)]
    struct DirectoryState(Arc<HashMap<String, (Uuid, String)>>);

    async fn directory_school(
        State(state): State<DirectoryState>,
        Path(subdomain): Path<String>,
    ) -> Response {
        match state.0.get(&subdomain) {
            Some((tenant_id, database_url)) => Json(json!({
                "id": tenant_id,
                "db_connection_string": database_url,
                "name": subdomain,
            }))
            .into_response(),
            None => StatusCode::NOT_FOUND.into_response(),
        }
    }

    struct ProtectedFixture {
        runtime: AuthRuntime,
        tenant_a_id: Uuid,
        tenant_a_pool: sqlx::PgPool,
        _tenant_b_pool: sqlx::PgPool,
        server: JoinHandle<()>,
    }

    impl ProtectedFixture {
        async fn new() -> Self {
            let tenant_a_pool = create_named_test_pool("session_router_tenant_a").await;
            run_test_migrations(&tenant_a_pool).await;
            let tenant_b_pool = create_named_test_pool("session_router_tenant_b").await;
            run_test_migrations(&tenant_b_pool).await;

            let tenant_a_id = Uuid::new_v4();
            let tenant_b_id = Uuid::new_v4();
            let tenant_a_url = "test-pool://session-router-tenant-a".to_string();
            let tenant_b_url = "test-pool://session-router-tenant-b".to_string();
            let directory = DirectoryState(Arc::new(HashMap::from([
                ("tenant-a".to_string(), (tenant_a_id, tenant_a_url.clone())),
                ("tenant-b".to_string(), (tenant_b_id, tenant_b_url.clone())),
            ])));
            let listener = TcpListener::bind("127.0.0.1:0")
                .await
                .expect("test directory must bind");
            let address = listener
                .local_addr()
                .expect("test directory address must resolve");
            let server = tokio::spawn(async move {
                let app = Router::new()
                    .route("/internal/schools/{subdomain}", get(directory_school))
                    .with_state(directory);
                axum::serve(listener, app)
                    .await
                    .expect("test directory must serve");
            });

            let pool_manager = Arc::new(PoolManager::new());
            pool_manager
                .insert_test_pool(&tenant_a_url, tenant_a_pool.clone())
                .await;
            pool_manager
                .insert_test_pool(&tenant_b_url, tenant_b_pool.clone())
                .await;
            let config = Arc::new(SessionConfig::for_tests_with_dev_origins(
                SessionHmacKey::for_tests([61; 32]),
                ["http://localhost:5173"],
            ));
            let (events, _) = broadcast::channel(32);
            let runtime = AuthRuntime {
                admin_client: Arc::new(AdminClient::new(
                    format!("http://{address}"),
                    "test-secret".to_string(),
                    AdminClientConfig::for_tests(
                        std::time::Duration::from_secs(1),
                        1,
                        std::time::Duration::from_millis(1),
                    ),
                )),
                pool_manager,
                permission_cache: Arc::new(PermissionCache::new()),
                config,
                session_events: events,
            };

            Self {
                runtime,
                tenant_a_id,
                tenant_a_pool,
                _tenant_b_pool: tenant_b_pool,
                server,
            }
        }

        async fn insert_user(&self, pool: &sqlx::PgPool, username: &str, status: &str) -> Uuid {
            sqlx::query_scalar(
                "INSERT INTO users (\
                     username, email, password_hash, first_name, last_name, user_type, status\
                 ) VALUES ($1, $2, 'unused-session-router-hash', 'Test', 'User', 'staff', $3)\
                 RETURNING id",
            )
            .bind(username)
            .bind(format!("{username}@example.test"))
            .bind(status)
            .fetch_one(pool)
            .await
            .expect("router test user must insert")
        }

        async fn insert_session(
            &self,
            pool: &sqlx::PgPool,
            user_id: Uuid,
            token_byte: u8,
            rotation_due: bool,
        ) -> (Uuid, RawSessionToken) {
            let session_id = Uuid::new_v4();
            let token = RawSessionToken::from_bytes([token_byte; 32]);
            let now = Utc::now();
            let rotated_at = if rotation_due {
                now - Duration::minutes(16)
            } else {
                now
            };
            sqlx::query(
                "INSERT INTO auth_sessions (\
                     id, user_id, current_token_hash, remember_me, device_label, created_at,\
                     last_seen_at, idle_expires_at, absolute_expires_at, rotated_at\
                 ) VALUES ($1, $2, $3, false, 'Router test', $4, $4, $5, $6, $7)",
            )
            .bind(session_id)
            .bind(user_id)
            .bind(token.token_hash().as_bytes().as_slice())
            .bind(now - Duration::minutes(20))
            .bind(now + Duration::hours(2))
            .bind(now + Duration::hours(12))
            .bind(rotated_at)
            .execute(pool)
            .await
            .expect("router test session must insert");
            (session_id, token)
        }

        fn csrf(&self, session_id: Uuid) -> String {
            session_csrf_token(self.runtime.config.hmac_key(), self.tenant_a_id, session_id)
                .expose_for_header()
        }
    }

    impl Drop for ProtectedFixture {
        fn drop(&mut self) {
            self.server.abort();
        }
    }

    async fn protected_identity(
        Extension(session): Extension<AuthenticatedSession>,
    ) -> Json<serde_json::Value> {
        Json(json!({
            "sessionId": session.session_id,
            "userId": session.user_id,
        }))
    }

    async fn feature_cookie(Extension(_session): Extension<AuthenticatedSession>) -> Response {
        let mut response = StatusCode::OK.into_response();
        response.headers_mut().append(
            header::SET_COOKIE,
            HeaderValue::from_static("feature=value; Secure; Path=/"),
        );
        response
    }

    fn protected_app(runtime: AuthRuntime) -> Router {
        Router::new()
            .route("/protected", get(protected_identity))
            .route("/unsafe", post(protected_identity))
            .route("/unsafe", put(protected_identity))
            .route("/unsafe", patch(protected_identity))
            .route("/unsafe", delete(protected_identity))
            .route("/api/auth/me", get(protected_identity))
            .route("/api/notifications/stream", get(protected_identity))
            .route("/feature-cookie", get(feature_cookie))
            .route_layer(from_fn_with_state(runtime, session_middleware))
    }

    fn request(
        method: Method,
        uri: &str,
        origin: Option<&str>,
        token: Option<&RawSessionToken>,
        csrf: Option<&str>,
    ) -> Request<Body> {
        let mut request = Request::builder().method(method).uri(uri);
        if let Some(origin) = origin {
            request = request.header("origin", origin);
        }
        if let Some(token) = token {
            request = request.header(
                header::COOKIE,
                format!(
                    "__Host-schoolorbit_session={}",
                    token.encode().expose_for_cookie()
                ),
            );
        }
        if let Some(csrf) = csrf {
            request = request.header("x-csrf-token", csrf);
        }
        request
            .body(Body::empty())
            .expect("protected router request must build")
    }

    fn set_cookie_count(response: &Response) -> usize {
        response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .count()
    }

    #[tokio::test]
    async fn auth_json_limit_rejects_before_handler_while_application_payload_remains_available() {
        let service_invocations = Arc::new(AtomicUsize::new(0));
        let handler_service_invocations = Arc::clone(&service_invocations);
        let app = Router::new()
            .route(
                "/login",
                post(
                    move |payload: Result<Json<serde_json::Value>, JsonRejection>| {
                        let service_invocations = Arc::clone(&handler_service_invocations);
                        async move {
                            match payload {
                                Ok(_) => {
                                    service_invocations.fetch_add(1, Ordering::SeqCst);
                                    StatusCode::OK.into_response()
                                }
                                Err(rejection) => rejection.into_response(),
                            }
                        }
                    },
                )
                .layer(DefaultBodyLimit::max(AUTH_JSON_BODY_LIMIT)),
            )
            .route(
                "/upload",
                post(|body: Bytes| async move { (StatusCode::OK, body.len().to_string()) }),
            )
            .layer(DefaultBodyLimit::max(APPLICATION_BODY_LIMIT));

        let oversized_auth = json!({
            "username": "limit-test",
            "password": "x".repeat(AUTH_JSON_BODY_LIMIT),
        })
        .to_string();
        let rejected = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/login")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(oversized_auth.clone()))
                    .expect("oversized auth request must build"),
            )
            .await
            .expect("oversized auth response must resolve");
        assert_eq!(rejected.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(service_invocations.load(Ordering::SeqCst), 0);

        let accepted = app
            .oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri("/upload")
                    .body(Body::from(oversized_auth))
                    .expect("application payload request must build"),
            )
            .await
            .expect("application payload response must resolve");
        assert_eq!(accepted.status(), StatusCode::OK);
    }

    #[test]
    fn rotation_is_deferred_only_for_the_closed_credential_mutation_set() {
        let session_id = Uuid::new_v4();
        for (method, path) in [
            (Method::GET, "/api/notifications/stream".to_string()),
            (Method::POST, "/api/auth/logout-all".to_string()),
            (Method::POST, "/api/auth/me/change-password".to_string()),
            (Method::DELETE, format!("/api/auth/sessions/{session_id}")),
        ] {
            assert_eq!(
                maintenance_mode(&method, &path),
                SessionMaintenanceMode::TouchOnly,
                "{method} {path}"
            );
        }

        for (method, path) in [
            (Method::GET, "/api/auth/me"),
            (Method::GET, "/api/notifications/stream/extra"),
            (Method::DELETE, "/api/auth/sessions/not-a-uuid"),
            (
                Method::DELETE,
                "/api/auth/sessions/00000000-0000-0000-0000-000000000000/extra",
            ),
        ] {
            assert_eq!(
                maintenance_mode(&method, path),
                SessionMaintenanceMode::RotateAndTouch,
                "{method} {path}"
            );
        }
    }

    #[tokio::test]
    async fn opaque_session_boundary_rejects_legacy_identity_and_rotates_safely() {
        let fixture = ProtectedFixture::new().await;
        let active_user = fixture
            .insert_user(&fixture.tenant_a_pool, "router-active", "active")
            .await;
        let inactive_user = fixture
            .insert_user(&fixture.tenant_a_pool, "router-inactive", "inactive")
            .await;
        let (active_session_id, active_token) = fixture
            .insert_session(&fixture.tenant_a_pool, active_user, 71, false)
            .await;
        let (_inactive_session_id, inactive_token) = fixture
            .insert_session(&fixture.tenant_a_pool, inactive_user, 72, false)
            .await;
        let (other_session_id, _other_token) = fixture
            .insert_session(&fixture.tenant_a_pool, active_user, 73, false)
            .await;
        let active_csrf = fixture.csrf(active_session_id);
        let other_csrf = fixture.csrf(other_session_id);
        let app = protected_app(fixture.runtime.clone());
        let tenant_a_origin = "https://tenant-a.schoolorbit.test";

        let no_cookie = app
            .clone()
            .oneshot(request(
                Method::GET,
                "/protected",
                Some(tenant_a_origin),
                None,
                None,
            ))
            .await
            .expect("no-cookie response must resolve");
        assert_eq!(no_cookie.status(), StatusCode::UNAUTHORIZED);

        let mut legacy_only = request(Method::GET, "/protected", Some(tenant_a_origin), None, None);
        legacy_only.headers_mut().insert(
            header::COOKIE,
            HeaderValue::from_static("auth_token=fake.jwt"),
        );
        let legacy_only = app
            .clone()
            .oneshot(legacy_only)
            .await
            .expect("legacy-cookie response must resolve");
        assert_eq!(legacy_only.status(), StatusCode::UNAUTHORIZED);

        let mut bearer_only = request(Method::GET, "/protected", Some(tenant_a_origin), None, None);
        bearer_only.headers_mut().insert(
            header::AUTHORIZATION,
            HeaderValue::from_static("Bearer fake.jwt"),
        );
        let bearer_only = app
            .clone()
            .oneshot(bearer_only)
            .await
            .expect("bearer response must resolve");
        assert_eq!(bearer_only.status(), StatusCode::UNAUTHORIZED);

        let valid = app
            .clone()
            .oneshot(request(
                Method::GET,
                "/protected",
                Some(tenant_a_origin),
                Some(&active_token),
                None,
            ))
            .await
            .expect("valid session response must resolve");
        assert_eq!(valid.status(), StatusCode::OK);

        let wrong_tenant = app
            .clone()
            .oneshot(request(
                Method::GET,
                "/protected",
                Some("https://tenant-b.schoolorbit.test"),
                Some(&active_token),
                None,
            ))
            .await
            .expect("cross-tenant response must resolve");
        assert_eq!(wrong_tenant.status(), StatusCode::UNAUTHORIZED);

        let inactive = app
            .clone()
            .oneshot(request(
                Method::GET,
                "/protected",
                Some(tenant_a_origin),
                Some(&inactive_token),
                None,
            ))
            .await
            .expect("inactive response must resolve");
        assert_eq!(inactive.status(), StatusCode::UNAUTHORIZED);

        for method in [Method::POST, Method::PUT, Method::PATCH, Method::DELETE] {
            for (origin, csrf) in [
                (None, Some(active_csrf.as_str())),
                (Some("https://evil.test"), Some(active_csrf.as_str())),
                (Some(tenant_a_origin), None),
                (Some(tenant_a_origin), Some("wrong-csrf")),
            ] {
                let rejected = app
                    .clone()
                    .oneshot(request(
                        method.clone(),
                        "/unsafe",
                        origin,
                        Some(&active_token),
                        csrf,
                    ))
                    .await
                    .expect("unsafe rejection must resolve");
                assert_eq!(
                    rejected.status(),
                    StatusCode::FORBIDDEN,
                    "{method} origin={origin:?} csrf={csrf:?}"
                );
                assert_eq!(set_cookie_count(&rejected), 0);
            }

            let accepted = app
                .clone()
                .oneshot(request(
                    method.clone(),
                    "/unsafe",
                    Some(tenant_a_origin),
                    Some(&active_token),
                    Some(&active_csrf),
                ))
                .await
                .expect("unsafe success must resolve");
            assert_eq!(accepted.status(), StatusCode::OK, "{method}");
        }

        let different_session_csrf = app
            .clone()
            .oneshot(request(
                Method::POST,
                "/unsafe",
                Some(tenant_a_origin),
                Some(&active_token),
                Some(&other_csrf),
            ))
            .await
            .expect("different-session CSRF response must resolve");
        assert_eq!(different_session_csrf.status(), StatusCode::FORBIDDEN);

        let (due_session_id, due_token) = fixture
            .insert_session(&fixture.tenant_a_pool, active_user, 74, true)
            .await;
        let due_csrf = fixture.csrf(due_session_id);
        let streaming = app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/notifications/stream?school_subdomain=tenant-a",
                Some("http://localhost:5173"),
                Some(&due_token),
                None,
            ))
            .await
            .expect("streaming response must resolve");
        assert_eq!(streaming.status(), StatusCode::OK);
        assert_eq!(set_cookie_count(&streaming), 0);
        assert!(streaming.headers().get("x-csrf-token").is_none());

        let duplicate_hint = app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/notifications/stream?school_subdomain=tenant-a&school_subdomain=tenant-b",
                Some("http://localhost:5173"),
                Some(&due_token),
                None,
            ))
            .await
            .expect("duplicate hint response must resolve");
        assert_eq!(duplicate_hint.status(), StatusCode::FORBIDDEN);

        let rotated = app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/auth/me",
                Some(tenant_a_origin),
                Some(&due_token),
                None,
            ))
            .await
            .expect("ordinary rotation response must resolve");
        assert_eq!(rotated.status(), StatusCode::OK);
        assert_eq!(set_cookie_count(&rotated), 1);
        assert_eq!(
            rotated
                .headers()
                .get("x-csrf-token")
                .expect("rotation must expose CSRF"),
            due_csrf.as_str()
        );
        let replacement_cookie = rotated
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .find_map(|value| {
                value
                    .to_str()
                    .ok()?
                    .strip_prefix("__Host-schoolorbit_session=")?
                    .split(';')
                    .next()
            })
            .expect("rotation must provide an opaque replacement");
        let replacement =
            RawSessionToken::parse(replacement_cookie).expect("replacement credential must parse");

        let previous = app
            .clone()
            .oneshot(request(
                Method::GET,
                "/api/auth/me",
                Some(tenant_a_origin),
                Some(&due_token),
                None,
            ))
            .await
            .expect("previous credential response must resolve");
        assert_eq!(previous.status(), StatusCode::OK);
        assert_eq!(set_cookie_count(&previous), 0);
        assert_eq!(
            previous
                .headers()
                .get("x-csrf-token")
                .expect("previous credential must expose stable CSRF"),
            due_csrf.as_str()
        );

        let replacement_accepted = app
            .clone()
            .oneshot(request(
                Method::POST,
                "/unsafe",
                Some(tenant_a_origin),
                Some(&replacement),
                Some(&due_csrf),
            ))
            .await
            .expect("replacement credential response must resolve");
        assert_eq!(replacement_accepted.status(), StatusCode::OK);

        let (concurrent_session_id, concurrent_token) = fixture
            .insert_session(&fixture.tenant_a_pool, active_user, 75, true)
            .await;
        let concurrent_csrf = fixture.csrf(concurrent_session_id);
        let first = app.clone().oneshot(request(
            Method::GET,
            "/api/auth/me",
            Some(tenant_a_origin),
            Some(&concurrent_token),
            None,
        ));
        let second = app.clone().oneshot(request(
            Method::GET,
            "/api/auth/me",
            Some(tenant_a_origin),
            Some(&concurrent_token),
            None,
        ));
        let (first, second) = tokio::join!(first, second);
        let first = first.expect("first concurrent response must resolve");
        let second = second.expect("second concurrent response must resolve");
        assert_eq!(first.status(), StatusCode::OK);
        assert_eq!(second.status(), StatusCode::OK);
        assert_eq!(set_cookie_count(&first) + set_cookie_count(&second), 1);
        for response in [&first, &second] {
            assert_eq!(
                response
                    .headers()
                    .get("x-csrf-token")
                    .expect("concurrent response must expose CSRF"),
                concurrent_csrf.as_str()
            );
        }

        let (_feature_session_id, feature_token) = fixture
            .insert_session(&fixture.tenant_a_pool, active_user, 76, true)
            .await;
        let feature = app
            .oneshot(request(
                Method::GET,
                "/feature-cookie",
                Some(tenant_a_origin),
                Some(&feature_token),
                None,
            ))
            .await
            .expect("feature-cookie response must resolve");
        assert_eq!(feature.status(), StatusCode::OK);
        assert_eq!(set_cookie_count(&feature), 2);
    }
}
