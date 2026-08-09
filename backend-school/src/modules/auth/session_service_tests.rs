use std::{
    io::Write,
    net::IpAddr,
    sync::{
        atomic::{AtomicU8, Ordering},
        mpsc, Arc, Mutex,
    },
};

use axum::http::StatusCode;
use chrono::{DateTime, Duration, Timelike, Utc};
use sqlx::PgPool;
use tokio::sync::broadcast;
use tracing::instrument::WithSubscriber;
use tracing_subscriber::fmt::MakeWriter;
use uuid::Uuid;

use crate::{
    db::permission_cache::PermissionCache,
    error::AppError,
    test_helpers::{create_named_test_pool, run_test_migrations},
    utils::tenant::TenantContext,
};

use super::{
    audit::{self, LoginRejectionReason},
    config::SessionConfig,
    events::SessionRevocationEvent,
    session_crypto::{identifier_bucket, RawSessionToken, SessionHmacKey},
    session_policy::normalize_login_identifier,
    session_repository::{SessionMaintenanceMode, SessionRevocationReason},
    session_service::{
        authenticate, change_password, list_sessions, load_current_user, login, logout, logout_all,
        revalidate, revoke_selected, LoginCommand, LoginResult, SessionServiceContext,
    },
};

struct AuthServiceFixture {
    context: SessionServiceContext,
    pool: PgPool,
    now: DateTime<Utc>,
    next_token_byte: AtomicU8,
}

impl AuthServiceFixture {
    async fn new(test_name: &str) -> Self {
        let pool = create_named_test_pool(test_name).await;
        run_test_migrations(&pool).await;
        let config = Arc::new(SessionConfig::for_tests(SessionHmacKey::for_tests(
            [19; 32],
        )));
        let tenant = TenantContext {
            tenant_id: Uuid::new_v4(),
            subdomain: format!("{test_name}-school"),
            pool: pool.clone(),
        };
        let (events, _) = broadcast::channel(32);
        let context =
            SessionServiceContext::new(tenant, Arc::new(PermissionCache::new()), config, events);

        Self {
            context,
            pool,
            now: database_timestamp(Utc::now()),
            next_token_byte: AtomicU8::new(20),
        }
    }

    async fn insert_user(&self, username: &str, password: &str, status: &str) -> Uuid {
        let password = password.to_string();
        let password_hash = tokio::task::spawn_blocking(move || bcrypt::hash(password, 4))
            .await
            .unwrap()
            .unwrap();
        sqlx::query_scalar(
            "INSERT INTO users \
             (username, email, password_hash, first_name, last_name, user_type, status) \
             VALUES ($1, $2, $3, 'Test', 'Teacher', 'staff', $4) RETURNING id",
        )
        .bind(username)
        .bind(format!("{username}@example.test"))
        .bind(password_hash)
        .bind(status)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    async fn assign_teacher_role(&self, user_id: Uuid) {
        sqlx::query(
            "INSERT INTO user_roles (user_id, role_id, is_primary) \
             SELECT $1, id, true FROM roles WHERE code = 'TEACHER'",
        )
        .bind(user_id)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    fn credentials(bytes: [u8; 32]) -> impl FnOnce() -> Result<RawSessionToken, AppError> {
        move || Ok(RawSessionToken::from_bytes(bytes))
    }

    async fn login(&self, username: &str, password: &str) -> Result<LoginResult, AppError> {
        self.login_from_at(
            username,
            password,
            "203.0.113.9",
            false,
            self.now,
            self.next_credentials(),
        )
        .await
    }

    async fn login_from(
        &self,
        username: &str,
        password: &str,
        source: &str,
    ) -> Result<LoginResult, AppError> {
        self.login_from_at(
            username,
            password,
            source,
            false,
            self.now,
            self.next_credentials(),
        )
        .await
    }

    async fn login_with_token(
        &self,
        username: &str,
        password: &str,
        remember_me: bool,
        now: DateTime<Utc>,
        token: [u8; 32],
    ) -> Result<LoginResult, AppError> {
        self.login_from_at(username, password, "203.0.113.9", remember_me, now, token)
            .await
    }

    async fn login_from_at(
        &self,
        username: &str,
        password: &str,
        source: &str,
        remember_me: bool,
        now: DateTime<Utc>,
        token: [u8; 32],
    ) -> Result<LoginResult, AppError> {
        login(
            &self.context,
            LoginCommand {
                username,
                password,
                remember_me,
                source: source.parse::<IpAddr>().unwrap(),
                user_agent: Some("Mozilla/5.0 Firefox/120 Linux"),
                now,
            },
            Self::credentials(token),
        )
        .await
    }

    fn next_credentials(&self) -> [u8; 32] {
        [self.next_token_byte.fetch_add(1, Ordering::Relaxed); 32]
    }

    async fn clear_identifier_bucket(&self, username: &str) {
        let bucket = identifier_bucket(
            self.context.config().hmac_key(),
            self.context.tenant().tenant_id,
            &normalize_login_identifier(username),
        );
        sqlx::query(
            "DELETE FROM auth_login_throttles \
             WHERE bucket_kind = 'identifier' AND bucket_hash = $1",
        )
        .bind(bucket.as_bytes().as_slice())
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn identifier_failure_count(&self, username: &str) -> i32 {
        let bucket = identifier_bucket(
            self.context.config().hmac_key(),
            self.context.tenant().tenant_id,
            &normalize_login_identifier(username),
        );
        sqlx::query_scalar(
            "SELECT failure_count FROM auth_login_throttles \
             WHERE bucket_kind = 'identifier' AND bucket_hash = $1",
        )
        .bind(bucket.as_bytes().as_slice())
        .fetch_optional(&self.pool)
        .await
        .unwrap()
        .unwrap_or_default()
    }

    async fn session_count(&self) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM auth_sessions")
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn session_is_active(&self, session_id: Uuid) -> bool {
        sqlx::query_scalar("SELECT revoked_at IS NULL FROM auth_sessions WHERE id = $1")
            .bind(session_id)
            .fetch_one(&self.pool)
            .await
            .unwrap()
    }

    async fn password_verifies(&self, username: &str, password: &str) -> bool {
        let hash: String =
            sqlx::query_scalar("SELECT password_hash FROM users WHERE username = $1")
                .bind(username)
                .fetch_one(&self.pool)
                .await
                .unwrap();
        let password = password.to_string();
        tokio::task::spawn_blocking(move || bcrypt::verify(password, &hash))
            .await
            .unwrap()
            .unwrap()
    }

    async fn make_rotation_due(&self, session_id: Uuid, at: DateTime<Utc>) {
        sqlx::query(
            "UPDATE auth_sessions \
             SET created_at = LEAST(created_at, $1), rotated_at = $1 \
             WHERE id = $2",
        )
        .bind(at - Duration::minutes(16))
        .bind(session_id)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn session_token_hashes(&self, session_id: Uuid) -> (Vec<u8>, Option<Vec<u8>>) {
        sqlx::query_as(
            "SELECT current_token_hash, previous_token_hash FROM auth_sessions WHERE id = $1",
        )
        .bind(session_id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }
}

fn database_timestamp(value: DateTime<Utc>) -> DateTime<Utc> {
    value
        .with_nanosecond((value.nanosecond() / 1_000) * 1_000)
        .unwrap()
}

#[derive(Clone, Default)]
struct CapturedLogs(Arc<Mutex<Vec<u8>>>);

impl CapturedLogs {
    fn text(&self) -> String {
        String::from_utf8(self.0.lock().unwrap().clone()).unwrap()
    }

    fn clear(&self) {
        self.0.lock().unwrap().clear();
    }
}

struct CapturedLogWriter(Arc<Mutex<Vec<u8>>>);

impl Write for CapturedLogWriter {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'writer> MakeWriter<'writer> for CapturedLogs {
    type Writer = CapturedLogWriter;

    fn make_writer(&'writer self) -> Self::Writer {
        CapturedLogWriter(self.0.clone())
    }
}

#[tokio::test]
async fn unknown_wrong_and_inactive_logins_share_one_public_error() {
    let fixture = AuthServiceFixture::new("service_generic_login_error").await;
    fixture
        .insert_user("inactive.user", "correct-password", "inactive")
        .await;
    fixture
        .insert_user("active.user", "correct-password", "active")
        .await;

    let errors = [
        fixture.login("missing.user", "wrong").await.unwrap_err(),
        fixture
            .login("inactive.user", "correct-password")
            .await
            .unwrap_err(),
        fixture.login("active.user", "wrong").await.unwrap_err(),
    ];
    for error in errors {
        assert_eq!(error.status_code(), StatusCode::UNAUTHORIZED);
        assert_eq!(error.public_message(), "ชื่อผู้ใช้หรือรหัสผ่านไม่ถูกต้อง");
    }
    assert_eq!(fixture.session_count().await, 0);
}

#[tokio::test]
async fn fifth_identifier_failure_and_twentieth_source_failure_return_retry_after() {
    let fixture = AuthServiceFixture::new("service_login_throttle_thresholds").await;
    fixture
        .insert_user("teacher.one", "correct-password", "active")
        .await;

    for expected in 1..=5 {
        let error = fixture
            .login_from("teacher.one", "wrong", "203.0.113.9")
            .await
            .unwrap_err();
        if expected < 5 {
            assert_eq!(error.status_code(), StatusCode::UNAUTHORIZED);
        } else {
            assert_eq!(error.retry_after_seconds(), Some(1));
        }
    }

    let blocked_correct = fixture
        .login_from("teacher.one", "correct-password", "203.0.113.10")
        .await
        .unwrap_err();
    assert_eq!(blocked_correct.retry_after_seconds(), Some(1));

    fixture.clear_identifier_bucket("teacher.one").await;
    for expected in 1..=20 {
        let error = fixture
            .login_from(&format!("unknown-{expected}"), "wrong", "198.51.100.4")
            .await
            .unwrap_err();
        if expected == 20 {
            assert_eq!(error.retry_after_seconds(), Some(1));
        }
    }
}

#[tokio::test]
async fn successful_login_resets_identifier_and_loads_role_permissions_before_insert() {
    let fixture = AuthServiceFixture::new("service_login_snapshot").await;
    let user_id = fixture
        .insert_user("teacher.one", "correct-password", "active")
        .await;
    fixture.assign_teacher_role(user_id).await;

    fixture.login("teacher.one", "wrong").await.unwrap_err();
    assert_eq!(fixture.identifier_failure_count("teacher.one").await, 1);

    let result = fixture
        .login("teacher.one", "correct-password")
        .await
        .unwrap();
    assert_eq!(result.user.id, user_id);
    assert_eq!(result.user.primary_role_name.as_deref(), Some("ครูผู้สอน"));
    assert!(result
        .user
        .permissions
        .iter()
        .any(|permission| permission == "student.read.assigned"));
    assert_eq!(fixture.identifier_failure_count("teacher.one").await, 0);
    assert_eq!(fixture.session_count().await, 1);

    let failing = AuthServiceFixture::new("service_permission_before_insert").await;
    failing
        .insert_user("teacher.two", "correct-password", "active")
        .await;
    sqlx::query("DROP TABLE permissions CASCADE")
        .execute(&failing.pool)
        .await
        .unwrap();
    sqlx::query("CREATE TABLE permissions (id uuid PRIMARY KEY)")
        .execute(&failing.pool)
        .await
        .unwrap();
    let error = failing
        .login("teacher.two", "correct-password")
        .await
        .unwrap_err();
    assert_eq!(error.status_code(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(failing.session_count().await, 0);
}

#[tokio::test]
async fn csrf_is_stable_across_rotation_and_password_change_and_cookie_lifetime_shrinks() {
    let fixture = AuthServiceFixture::new("service_csrf_rotation").await;
    fixture
        .insert_user("teacher.one", "old-password", "active")
        .await;
    let login = fixture
        .login_with_token("teacher.one", "old-password", true, fixture.now, [41; 32])
        .await
        .unwrap();
    assert_eq!(login.credential.cookie_max_age_seconds, Some(2_592_000));
    fixture
        .make_rotation_due(login.authenticated.session_id, fixture.now)
        .await;

    let rotated = authenticate(
        &fixture.context,
        login.credential.token_hash(),
        fixture.now,
        SessionMaintenanceMode::RotateAndTouch,
        AuthServiceFixture::credentials([42; 32]),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(rotated.csrf_token, login.csrf_token);
    let replacement = rotated.replacement.as_ref().unwrap();

    let previous = authenticate(
        &fixture.context,
        login.credential.token_hash(),
        fixture.now + Duration::seconds(1),
        SessionMaintenanceMode::RotateAndTouch,
        || panic!("previous credentials must not rotate"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(previous.csrf_token, login.csrf_token);

    let password = change_password(
        &fixture.context,
        &login.authenticated,
        "old-password",
        "new-password-123",
        fixture.now + Duration::seconds(2),
        AuthServiceFixture::credentials([43; 32]),
    )
    .await
    .unwrap();
    assert_eq!(password.csrf_token, login.csrf_token);
    assert_eq!(password.credential.cookie_max_age_seconds, Some(2_591_998));
    assert_ne!(password.credential.token_hash(), replacement.token_hash());

    let other = fixture
        .login_with_token(
            "teacher.one",
            "new-password-123",
            true,
            fixture.now + Duration::seconds(3),
            [44; 32],
        )
        .await
        .unwrap();
    assert_ne!(other.csrf_token, login.csrf_token);

    let day_29 = fixture.now + Duration::days(29);
    sqlx::query(
        "UPDATE auth_sessions \
         SET last_seen_at = $1 - interval '6 minutes', \
             idle_expires_at = LEAST($1 + interval '7 days', absolute_expires_at), \
             rotated_at = $1 - interval '16 minutes' \
         WHERE id = $2",
    )
    .bind(day_29)
    .bind(other.authenticated.session_id)
    .execute(&fixture.pool)
    .await
    .unwrap();
    let late = authenticate(
        &fixture.context,
        other.credential.token_hash(),
        day_29,
        SessionMaintenanceMode::RotateAndTouch,
        AuthServiceFixture::credentials([45; 32]),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        late.replacement.unwrap().cookie_max_age_seconds,
        Some(86_403)
    );
}

#[tokio::test]
async fn websocket_touch_only_handshake_defers_rotation_to_the_next_ordinary_request() {
    let fixture = AuthServiceFixture::new("service_websocket_touch_only").await;
    fixture
        .insert_user("teacher.one", "correct-password", "active")
        .await;
    let login = fixture
        .login_with_token(
            "teacher.one",
            "correct-password",
            true,
            fixture.now,
            [61; 32],
        )
        .await
        .unwrap();
    let session_id = login.authenticated.session_id;
    fixture.make_rotation_due(session_id, fixture.now).await;
    let before_handshake = fixture.session_token_hashes(session_id).await;

    let websocket = authenticate(
        &fixture.context,
        login.credential.token_hash(),
        fixture.now + Duration::seconds(1),
        SessionMaintenanceMode::TouchOnly,
        || panic!("WebSocket touch-only authentication must not generate a credential"),
    )
    .await
    .unwrap()
    .unwrap();

    assert!(websocket.replacement.is_none());
    assert_eq!(websocket.csrf_token, login.csrf_token);
    let after_handshake = fixture.session_token_hashes(session_id).await;
    assert_eq!(after_handshake, before_handshake);

    let ordinary = authenticate(
        &fixture.context,
        login.credential.token_hash(),
        fixture.now + Duration::seconds(2),
        SessionMaintenanceMode::RotateAndTouch,
        AuthServiceFixture::credentials([62; 32]),
    )
    .await
    .unwrap()
    .unwrap();

    assert!(ordinary.replacement.is_some());
    assert_eq!(ordinary.csrf_token, login.csrf_token);
    let after_ordinary_request = fixture.session_token_hashes(session_id).await;
    assert_ne!(after_ordinary_request.0, after_handshake.0);
    assert_eq!(after_ordinary_request.1, Some(after_handshake.0));
}

#[tokio::test]
async fn password_change_commits_hash_rotation_and_other_revocations_together() {
    let fixture = AuthServiceFixture::new("service_password_change_atomicity").await;
    fixture
        .insert_user("teacher.one", "old-password", "active")
        .await;
    let current = fixture
        .login_with_token("teacher.one", "old-password", false, fixture.now, [51; 32])
        .await
        .unwrap();
    let other = fixture
        .login_with_token("teacher.one", "old-password", false, fixture.now, [52; 32])
        .await
        .unwrap();
    let mut events = fixture.context.session_events().subscribe();

    let result = change_password(
        &fixture.context,
        &current.authenticated,
        "old-password",
        "new-password-123",
        fixture.now,
        AuthServiceFixture::credentials([53; 32]),
    )
    .await
    .unwrap();

    assert_eq!(result.credential.cookie_max_age_seconds, None);
    assert_eq!(
        result.revoked_session_ids,
        vec![other.authenticated.session_id]
    );
    assert!(
        fixture
            .password_verifies("teacher.one", "new-password-123")
            .await
    );
    assert!(
        fixture
            .session_is_active(current.authenticated.session_id)
            .await
    );
    assert!(
        !fixture
            .session_is_active(other.authenticated.session_id)
            .await
    );
    assert_ne!(
        result.credential.token_hash(),
        current.credential.token_hash()
    );
    assert!(events.try_recv().unwrap().applies_to(
        &fixture.context.tenant().subdomain,
        current.authenticated.user_id,
        other.authenticated.session_id,
    ));
}

#[tokio::test]
async fn password_change_collision_rolls_back_password_and_both_sessions() {
    let fixture = AuthServiceFixture::new("service_password_collision").await;
    fixture
        .insert_user("teacher.one", "old-password", "active")
        .await;
    let current = fixture
        .login_with_token("teacher.one", "old-password", false, fixture.now, [61; 32])
        .await
        .unwrap();
    let other = fixture
        .login_with_token("teacher.one", "old-password", false, fixture.now, [62; 32])
        .await
        .unwrap();
    let mut events = fixture.context.session_events().subscribe();

    let error = change_password(
        &fixture.context,
        &current.authenticated,
        "old-password",
        "new-password-123",
        fixture.now,
        AuthServiceFixture::credentials([62; 32]),
    )
    .await
    .unwrap_err();

    assert_eq!(error.status_code(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(
        fixture
            .password_verifies("teacher.one", "old-password")
            .await
    );
    assert!(
        !fixture
            .password_verifies("teacher.one", "new-password-123")
            .await
    );
    assert!(
        fixture
            .session_is_active(current.authenticated.session_id)
            .await
    );
    assert!(
        fixture
            .session_is_active(other.authenticated.session_id)
            .await
    );
    assert!(matches!(
        events.try_recv(),
        Err(broadcast::error::TryRecvError::Empty)
    ));
}

#[tokio::test]
async fn password_form_errors_are_400_and_inactive_or_revoked_state_is_401() {
    let fixture = AuthServiceFixture::new("service_password_error_semantics").await;
    let user_id = fixture
        .insert_user("teacher.one", "old-password", "active")
        .await;
    let current = fixture.login("teacher.one", "old-password").await.unwrap();

    let wrong = change_password(
        &fixture.context,
        &current.authenticated,
        "wrong-password",
        "new-password-123",
        fixture.now,
        || panic!("form errors must not generate credentials"),
    )
    .await
    .unwrap_err();
    assert_eq!(wrong.status_code(), StatusCode::BAD_REQUEST);

    let invalid = change_password(
        &fixture.context,
        &current.authenticated,
        "old-password",
        "short",
        fixture.now,
        || panic!("form errors must not generate credentials"),
    )
    .await
    .unwrap_err();
    assert_eq!(invalid.status_code(), StatusCode::BAD_REQUEST);
    assert!(
        fixture
            .password_verifies("teacher.one", "old-password")
            .await
    );
    assert!(
        fixture
            .session_is_active(current.authenticated.session_id)
            .await
    );

    sqlx::query("UPDATE users SET status = 'inactive' WHERE id = $1")
        .bind(user_id)
        .execute(&fixture.pool)
        .await
        .unwrap();
    let inactive = change_password(
        &fixture.context,
        &current.authenticated,
        "old-password",
        "new-password-123",
        fixture.now,
        || panic!("inactive users must not generate credentials"),
    )
    .await
    .unwrap_err();
    assert_eq!(inactive.status_code(), StatusCode::UNAUTHORIZED);

    sqlx::query("UPDATE users SET status = 'active' WHERE id = $1")
        .bind(user_id)
        .execute(&fixture.pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE auth_sessions SET revoked_at = $1, revocation_reason = 'logout' WHERE id = $2",
    )
    .bind(fixture.now)
    .bind(current.authenticated.session_id)
    .execute(&fixture.pool)
    .await
    .unwrap();
    let revoked = change_password(
        &fixture.context,
        &current.authenticated,
        "old-password",
        "new-password-123",
        fixture.now,
        || panic!("revoked sessions must not generate credentials"),
    )
    .await
    .unwrap_err();
    assert_eq!(revoked.status_code(), StatusCode::UNAUTHORIZED);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_password_hash_change_returns_409_without_overwriting_it() {
    let fixture = AuthServiceFixture::new("service_password_conflict").await;
    fixture
        .insert_user("teacher.one", "old-password", "active")
        .await;
    let current = fixture.login("teacher.one", "old-password").await.unwrap();
    let replacement_hash = bcrypt::hash("concurrent-password", 4).unwrap();
    let pool = fixture.pool.clone();
    let user_id = current.authenticated.user_id;
    let (start_tx, start_rx) = mpsc::channel();
    let (done_tx, done_rx) = mpsc::channel();
    let updater = tokio::spawn(async move {
        tokio::task::spawn_blocking(move || start_rx.recv().unwrap())
            .await
            .unwrap();
        sqlx::query("UPDATE users SET password_hash = $1 WHERE id = $2")
            .bind(replacement_hash)
            .bind(user_id)
            .execute(&pool)
            .await
            .unwrap();
        done_tx.send(()).unwrap();
    });

    let error = change_password(
        &fixture.context,
        &current.authenticated,
        "old-password",
        "new-password-123",
        fixture.now,
        move || {
            start_tx.send(()).unwrap();
            done_rx.recv().unwrap();
            Ok(RawSessionToken::from_bytes([71; 32]))
        },
    )
    .await
    .unwrap_err();
    updater.await.unwrap();

    assert_eq!(error.status_code(), StatusCode::CONFLICT);
    assert!(
        fixture
            .password_verifies("teacher.one", "concurrent-password")
            .await
    );
    assert!(
        fixture
            .session_is_active(current.authenticated.session_id)
            .await
    );
}

#[tokio::test]
async fn session_listing_and_revocation_are_owned_and_emit_committed_events() {
    let fixture = AuthServiceFixture::new("service_session_ownership").await;
    fixture
        .insert_user("teacher.one", "password-one", "active")
        .await;
    fixture
        .insert_user("teacher.two", "password-two", "active")
        .await;
    let current = fixture.login("teacher.one", "password-one").await.unwrap();
    let other_owned = fixture.login("teacher.one", "password-one").await.unwrap();
    let foreign = fixture.login("teacher.two", "password-two").await.unwrap();

    let listed = list_sessions(&current.authenticated, fixture.now)
        .await
        .unwrap();
    assert_eq!(listed.len(), 2);
    assert!(listed
        .iter()
        .all(|session| session.user_id == current.authenticated.user_id));

    let not_owned = revoke_selected(
        &fixture.context,
        &current.authenticated,
        foreign.authenticated.session_id,
        fixture.now,
    )
    .await
    .unwrap_err();
    assert_eq!(not_owned.status_code(), StatusCode::NOT_FOUND);

    let mut events = fixture.context.session_events().subscribe();
    let selected = revoke_selected(
        &fixture.context,
        &current.authenticated,
        other_owned.authenticated.session_id,
        fixture.now,
    )
    .await
    .unwrap();
    assert_eq!(
        selected.revoked_session_ids,
        vec![other_owned.authenticated.session_id]
    );
    assert!(!selected.current_revoked);
    assert_eq!(
        events.try_recv().unwrap(),
        SessionRevocationEvent::session(
            &fixture.context.tenant().subdomain,
            current.authenticated.user_id,
            other_owned.authenticated.session_id,
        )
    );

    let logged_out = logout(&fixture.context, &current.authenticated, fixture.now)
        .await
        .unwrap();
    assert!(logged_out.current_revoked);
    assert!(!revalidate(&current.authenticated, fixture.now)
        .await
        .unwrap());
}

#[tokio::test]
async fn logout_all_revokes_only_the_user_and_load_current_user_rechecks_status() {
    let fixture = AuthServiceFixture::new("service_logout_all").await;
    let user_id = fixture
        .insert_user("teacher.one", "password-one", "active")
        .await;
    fixture
        .insert_user("teacher.two", "password-two", "active")
        .await;
    let current = fixture.login("teacher.one", "password-one").await.unwrap();
    let other = fixture.login("teacher.one", "password-one").await.unwrap();
    let foreign = fixture.login("teacher.two", "password-two").await.unwrap();

    let shell = load_current_user(&fixture.context, &current.authenticated)
        .await
        .unwrap();
    assert_eq!(shell.id, user_id);

    let result = logout_all(&fixture.context, &current.authenticated, fixture.now)
        .await
        .unwrap();
    assert!(result.current_revoked);
    assert_eq!(result.revoked_session_ids.len(), 2);
    assert!(
        !fixture
            .session_is_active(current.authenticated.session_id)
            .await
    );
    assert!(
        !fixture
            .session_is_active(other.authenticated.session_id)
            .await
    );
    assert!(
        fixture
            .session_is_active(foreign.authenticated.session_id)
            .await
    );

    sqlx::query("UPDATE users SET status = 'inactive' WHERE id = $1")
        .bind(user_id)
        .execute(&fixture.pool)
        .await
        .unwrap();
    let error = load_current_user(&fixture.context, &current.authenticated)
        .await
        .unwrap_err();
    assert_eq!(error.status_code(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn audit_and_cleanup_logs_contain_only_fixed_allowlisted_fields() {
    let captured = CapturedLogs::default();
    let subscriber = tracing::Dispatch::new(
        tracing_subscriber::fmt()
            .json()
            .without_time()
            .with_writer(captured.clone())
            .finish(),
    );
    // Prime the audit callsites under this dispatcher before concurrent tests
    // can cache them under the no-op default. Clear the probe output so every
    // assertion below still proves that the service flow emitted the marker.
    tracing::callsite::rebuild_interest_cache();
    tracing::dispatcher::with_default(&subscriber, || {
        audit::login_rejected(Uuid::nil(), LoginRejectionReason::InvalidCredentials);
        audit::login_succeeded(Uuid::nil(), Uuid::nil(), Uuid::nil());
        audit::session_created(Uuid::nil(), Uuid::nil(), Uuid::nil());
        audit::session_revoked(
            Uuid::nil(),
            Uuid::nil(),
            Uuid::nil(),
            SessionRevocationReason::UserSelected,
        );
        audit::cleanup_failed();
    });
    captured.clear();

    async {
        let fixture = AuthServiceFixture::new("service_redacted_audit").await;
        let username = "audit-secret-user";
        let password = "audit-secret-password";
        let source = "198.51.100.77";
        let user_agent = "AuditSecretBrowser/77";
        fixture.insert_user(username, password, "active").await;

        login(
            &fixture.context,
            LoginCommand {
                username,
                // Exercise the typed rejection audit before database I/O;
                // credential verification behavior is covered separately.
                password: "",
                remember_me: false,
                source: source.parse().unwrap(),
                user_agent: Some(user_agent),
                now: fixture.now,
            },
            AuthServiceFixture::credentials([81; 32]),
        )
        .await
        .unwrap_err();
        let current = fixture
            .login_with_token(username, password, false, fixture.now, [82; 32])
            .await
            .unwrap();
        let other = fixture
            .login_with_token(username, password, false, fixture.now, [83; 32])
            .await
            .unwrap();
        revoke_selected(
            &fixture.context,
            &current.authenticated,
            other.authenticated.session_id,
            fixture.now,
        )
        .await
        .unwrap();

        sqlx::query(
            "INSERT INTO auth_login_throttles \
                     (bucket_kind, bucket_hash, failure_count, window_started_at, updated_at) \
                     VALUES ('source', decode(repeat('ab', 32), 'hex'), 1, \
                             $1 - interval '2 days', $1 - interval '2 days')",
        )
        .bind(fixture.now)
        .execute(&fixture.pool)
        .await
        .unwrap();
        sqlx::query(
            r#"
                    CREATE FUNCTION reject_test_auth_cleanup() RETURNS trigger AS $$
                    BEGIN
                        RAISE EXCEPTION 'forced-cleanup-secret-error';
                    END;
                    $$ LANGUAGE plpgsql
                    "#,
        )
        .execute(&fixture.pool)
        .await
        .unwrap();
        sqlx::query(
            "CREATE TRIGGER reject_test_auth_cleanup \
                     BEFORE DELETE ON auth_login_throttles FOR EACH ROW \
                     WHEN (OLD.bucket_hash = decode(repeat('ab', 32), 'hex')) \
                     EXECUTE FUNCTION reject_test_auth_cleanup()",
        )
        .execute(&fixture.pool)
        .await
        .unwrap();

        let listed = list_sessions(&current.authenticated, fixture.now)
            .await
            .unwrap();
        assert_eq!(listed.len(), 1);

        let identifier_hash = identifier_bucket(
            fixture.context.config().hmac_key(),
            fixture.context.tenant().tenant_id,
            &normalize_login_identifier(username),
        );
        let logs = captured.text();
        for secret in [
            username,
            password,
            source,
            user_agent,
            "forced-cleanup-secret-error",
            &hex::encode(identifier_hash.as_bytes()),
        ] {
            assert!(!logs.contains(secret), "audit leaked secret marker");
        }
        for required in [
            "login_rejected",
            "login_succeeded",
            "session_created",
            "session_revoked",
            "auth_cleanup_failed",
        ] {
            assert!(logs.contains(required), "missing audit marker {required}");
        }

        for line in logs.lines() {
            let value: serde_json::Value = serde_json::from_str(line).unwrap();
            let Some(fields) = value.get("fields").and_then(|fields| fields.as_object()) else {
                continue;
            };
            if fields.get("event").is_some() {
                assert!(fields.keys().all(|field| matches!(
                    field.as_str(),
                    "event" | "tenant_id" | "user_id" | "session_id" | "reason"
                )));
            }
        }
    }
    .with_subscriber(subscriber)
    .await;
}
