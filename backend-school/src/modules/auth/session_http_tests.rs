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
