use std::sync::Arc;

use chrono::{DateTime, Duration, Timelike, Utc};
use sqlx::PgPool;
use tokio::sync::Barrier;
use uuid::Uuid;

use school_http::AppErrorHttpExt;
use school_test_db::{
    create_named_test_pool, create_named_test_pool_with_max_connections, create_test_user,
    run_test_migrations,
};

use super::{
    session_crypto::{
        identifier_bucket, pending_session_token, source_bucket, RawSessionToken, SessionHmacKey,
        ThrottleBucketHash, TokenHash,
    },
    session_policy::{BucketKind, SessionLifetime, SessionTimes},
    session_repository::{
        cleanup_auth_state, list_user_sessions, maintain_session, revalidate_session,
        revoke_sessions, validate_session, ActivityTokenDerivation, MaintainedSession, NewSession,
        PresentedTokenKind, SessionRevocationReason, SessionRevocationTarget,
    },
    throttle_repository::{check_login_throttles, record_login_failure},
};

#[tokio::test]
async fn closed_login_throttle_store_preserves_service_unavailable() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    pool.close().await;
    let key = SessionHmacKey::for_tests([72; 32]);
    let tenant = Uuid::new_v4();
    let error = check_login_throttles(
        &pool,
        identifier_bucket(&key, tenant, "synthetic-user"),
        source_bucket(&key, tenant, std::net::Ipv4Addr::LOCALHOST.into()),
        Utc::now(),
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.status_code(),
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    );
    assert!(
        matches!(error, school_errors::AppError::ServiceUnavailable(ref reason) if reason == "session_store")
    );
}

#[tokio::test]
async fn closed_session_store_entrypoints_preserve_service_unavailable() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/unused")
        .unwrap();
    pool.close().await;
    let user = Uuid::new_v4();
    let session = Uuid::new_v4();
    let now = Utc::now();
    let results = [
        super::session_repository::load_session_validity(&pool, session, user, now)
            .await
            .map(|_| ()),
        super::session_repository::load_password_change_snapshot(&pool, user, session, now)
            .await
            .map(|_| ()),
        list_user_sessions(&pool, user, now).await.map(|_| ()),
        revoke_sessions(
            &pool,
            user,
            SessionRevocationTarget::Session(session),
            SessionRevocationReason::UserSelected,
            now,
        )
        .await
        .map(|_| ()),
        validate_session(
            &pool,
            RawSessionToken::from_bytes([71; 32]).token_hash(),
            now,
        )
        .await
        .map(|_| ()),
    ];
    for result in results {
        let error = result.unwrap_err();
        assert_eq!(
            error.status_code(),
            axum::http::StatusCode::SERVICE_UNAVAILABLE
        );
        assert!(
            matches!(error, school_errors::AppError::ServiceUnavailable(ref reason) if reason == "session_store")
        );
    }
}

// Fixture setup still uses the production guarded insertion contract.
async fn create_login_session(
    pool: &PgPool,
    session: &NewSession,
    identifier_hash: ThrottleBucketHash,
) -> Result<(), school_errors::AppError> {
    let username: String = sqlx::query_scalar("SELECT username FROM users WHERE id=$1")
        .bind(session.user_id)
        .fetch_one(pool)
        .await?;
    let user = super::services::find_session_login_user_by_username(pool, &username)
        .await?
        .unwrap();
    let result =
        super::session_repository::create_login_session(pool, session, identifier_hash, &user)
            .await?;
    assert_eq!(
        result,
        super::session_repository::LoginSessionCreation::Created
    );
    Ok(())
}

struct SessionFixture {
    pool: PgPool,
    user_id: Uuid,
    session_id: Uuid,
    raw_token: RawSessionToken,
    identifier_hash: ThrottleBucketHash,
    source_hash: ThrottleBucketHash,
}

impl SessionFixture {
    async fn active(test_name: &str) -> Self {
        let pool = create_named_test_pool(test_name).await;
        Self::in_pool(pool, test_name).await
    }

    async fn concurrent(test_name: &str, max_connections: u32) -> Self {
        let pool = create_named_test_pool_with_max_connections(test_name, max_connections).await;
        Self::in_pool(pool, test_name).await
    }

    async fn in_pool(pool: PgPool, label: &str) -> Self {
        run_test_migrations(&pool).await;
        let user_id = create_test_user(
            &pool,
            &format!("{label}-owner@example.test"),
            "test-password",
        )
        .await
        .unwrap();
        let session_id = Uuid::new_v4();
        let raw_token = RawSessionToken::generate().unwrap();
        let key = SessionHmacKey::for_tests([11_u8; 32]);
        let identifier_hash = identifier_bucket(&key, Uuid::nil(), label);
        let source_hash = source_bucket(&key, Uuid::nil(), "203.0.113.9".parse().unwrap());
        let now = Utc::now();
        let session = new_session(session_id, user_id, raw_token.token_hash(), false, now);
        create_login_session(&pool, &session, identifier_hash)
            .await
            .unwrap();

        Self {
            pool,
            user_id,
            session_id,
            raw_token,
            identifier_hash,
            source_hash,
        }
    }

    async fn set_rotated_at(&self, value: DateTime<Utc>) {
        self.set_rotated_at_for(self.session_id, value).await;
    }

    async fn set_rotated_at_for(&self, session_id: Uuid, value: DateTime<Utc>) {
        sqlx::query(
            "UPDATE auth_sessions SET created_at = LEAST(created_at, $1), rotated_at = $1 \
             WHERE id = $2",
        )
        .bind(value)
        .bind(session_id)
        .execute(&self.pool)
        .await
        .unwrap();
    }

    async fn set_last_seen_at(&self, value: DateTime<Utc>) {
        sqlx::query("UPDATE auth_sessions SET created_at=LEAST(created_at,$1), last_seen_at = $1 WHERE id = $2")
            .bind(value)
            .bind(self.session_id)
            .execute(&self.pool)
            .await
            .unwrap();
    }

    async fn hashes(&self) -> (Vec<u8>, Option<Vec<u8>>, DateTime<Utc>) {
        sqlx::query_as(
            "SELECT current_token_hash, previous_token_hash, rotated_at \
             FROM auth_sessions WHERE id = $1",
        )
        .bind(self.session_id)
        .fetch_one(&self.pool)
        .await
        .unwrap()
    }

    async fn create_other_user_session(&self) -> (Uuid, Uuid) {
        let other_user = create_test_user(
            &self.pool,
            &format!("other-{}@example.test", Uuid::new_v4()),
            "test-password",
        )
        .await
        .unwrap();
        let session_id = Uuid::new_v4();
        let token = RawSessionToken::generate().unwrap();
        let session = new_session(
            session_id,
            other_user,
            token.token_hash(),
            false,
            Utc::now(),
        );
        create_login_session(&self.pool, &session, self.identifier_hash)
            .await
            .unwrap();
        (other_user, session_id)
    }

    async fn create_same_user_session(&self, token: &RawSessionToken) -> Uuid {
        let session_id = Uuid::new_v4();
        let session = new_session(
            session_id,
            self.user_id,
            token.token_hash(),
            true,
            Utc::now(),
        );
        create_login_session(&self.pool, &session, self.identifier_hash)
            .await
            .unwrap();
        session_id
    }

    async fn failure_count(&self, kind: BucketKind) -> i32 {
        sqlx::query_scalar(
            "SELECT failure_count FROM auth_login_throttles \
             WHERE bucket_kind = $1 AND bucket_hash = $2",
        )
        .bind(match kind {
            BucketKind::Identifier => "identifier",
            BucketKind::Source => "source",
        })
        .bind(match kind {
            BucketKind::Identifier => self.identifier_hash.as_bytes().as_slice(),
            BucketKind::Source => self.source_hash.as_bytes().as_slice(),
        })
        .fetch_optional(&self.pool)
        .await
        .unwrap()
        .unwrap_or(0)
    }
}

fn database_timestamp(value: DateTime<Utc>) -> DateTime<Utc> {
    value
        .with_nanosecond((value.nanosecond() / 1_000) * 1_000)
        .unwrap()
}

fn new_session(
    id: Uuid,
    user_id: Uuid,
    current_token_hash: TokenHash,
    remember_me: bool,
    now: DateTime<Utc>,
) -> NewSession {
    let lifetime = if remember_me {
        SessionLifetime::remembered()
    } else {
        SessionLifetime::normal()
    };
    NewSession {
        id,
        user_id,
        current_token_hash,
        remember_me,
        device_label: "Test browser on Test OS".to_string(),
        times: SessionTimes {
            created_at: now,
            last_seen_at: now,
            idle_expires_at: now + lifetime.idle,
            absolute_expires_at: now + lifetime.absolute,
            rotated_at: now,
        },
    }
}

async fn pulse(
    fixture: &SessionFixture,
    raw: &RawSessionToken,
    now: DateTime<Utc>,
) -> Result<Option<MaintainedSession>, school_errors::AppError> {
    maintain_session(
        &fixture.pool,
        fixture.user_id,
        fixture.session_id,
        raw.token_hash(),
        now,
        Some(ActivityTokenDerivation {
            key: &SessionHmacKey::for_tests([11; 32]),
            tenant_id: Uuid::nil(),
            raw,
        }),
    )
    .await
}

#[tokio::test]
async fn concurrent_activity_stages_exactly_one_reproducible_pending_credential() {
    let fixture = SessionFixture::concurrent("concurrent_rotation", 4).await;
    let now = database_timestamp(Utc::now());
    fixture.set_rotated_at(now - Duration::minutes(16)).await;
    let (left, right) = tokio::join!(
        pulse(&fixture, &fixture.raw_token, now),
        pulse(&fixture, &fixture.raw_token, now)
    );
    let left = left.unwrap().unwrap();
    let right = right.unwrap().unwrap();
    assert_eq!(
        left.replacement.as_ref().unwrap().token_hash(),
        right.replacement.as_ref().unwrap().token_hash()
    );
    let (current, previous, _) = fixture.hashes().await;
    assert_eq!(
        current.as_slice(),
        fixture.raw_token.token_hash().as_bytes()
    );
    assert!(previous.is_none());
    let pending: (Vec<u8>, DateTime<Utc>) = sqlx::query_as(
        "SELECT pending_token_hash,pending_created_at FROM auth_sessions WHERE id=$1",
    )
    .bind(fixture.session_id)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(
        pending.0.as_slice(),
        left.replacement.unwrap().token_hash().as_bytes()
    );
    assert_eq!(pending.1, now);
}

#[tokio::test]
async fn lost_stage_reply_quiet_pending_and_exact_previous_grace_follow_acknowledgment() {
    let fixture = SessionFixture::active("current_previous_touch").await;
    let now = database_timestamp(Utc::now());
    fixture.set_rotated_at(now - Duration::minutes(16)).await;
    let old = fixture.raw_token.token_hash();
    let staged = pulse(&fixture, &fixture.raw_token, now)
        .await
        .unwrap()
        .unwrap();
    let b = staged.replacement.unwrap();
    // Lost reply: A stays current indefinitely within the session and repeats the same B.
    let quiet = now + Duration::minutes(30);
    assert_eq!(
        validate_session(&fixture.pool, old, quiet)
            .await
            .unwrap()
            .unwrap()
            .presented_as,
        PresentedTokenKind::Current
    );
    assert_eq!(
        validate_session(&fixture.pool, b.token_hash(), quiet)
            .await
            .unwrap()
            .unwrap()
            .presented_as,
        PresentedTokenKind::Pending
    );
    let replay = pulse(&fixture, &fixture.raw_token, quiet)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(replay.replacement.unwrap().token_hash(), b.token_hash());
    let (first, duplicate) = tokio::join!(
        maintain_session(
            &fixture.pool,
            fixture.user_id,
            fixture.session_id,
            b.token_hash(),
            quiet,
            None
        ),
        maintain_session(
            &fixture.pool,
            fixture.user_id,
            fixture.session_id,
            b.token_hash(),
            quiet,
            None
        )
    );
    assert!(first.unwrap().unwrap().replacement.is_none());
    assert!(duplicate.unwrap().unwrap().replacement.is_none());
    let (current, previous, rotated) = fixture.hashes().await;
    assert_eq!(current.as_slice(), b.token_hash().as_bytes());
    assert_eq!(previous.as_deref(), Some(old.as_bytes().as_slice()));
    assert_eq!(rotated, quiet);
    let (pending, grace): (Option<Vec<u8>>, DateTime<Utc>) = sqlx::query_as(
        "SELECT pending_token_hash,previous_token_valid_until FROM auth_sessions WHERE id=$1",
    )
    .bind(fixture.session_id)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert!(pending.is_none());
    assert_eq!(grace, quiet + Duration::seconds(60));
    assert!(
        validate_session(&fixture.pool, old, grace - Duration::microseconds(1))
            .await
            .unwrap()
            .is_some()
    );
    assert!(validate_session(&fixture.pool, old, grace)
        .await
        .unwrap()
        .is_none());
    // Validation rejects the expired A without deleting or touching anything.
    assert_eq!(
        fixture.hashes().await.1.as_deref(),
        Some(old.as_bytes().as_slice())
    );
}

#[tokio::test]
async fn ordinary_validation_does_not_touch_and_activity_writes_at_most_once_per_five_minutes() {
    let fixture = SessionFixture::active("activity_write_coalescing").await;
    let now = database_timestamp(Utc::now());
    fixture.set_last_seen_at(now - Duration::minutes(6)).await;
    let before: (DateTime<Utc>, DateTime<Utc>, String) = sqlx::query_as(
        "SELECT last_seen_at,idle_expires_at,xmin::text FROM auth_sessions WHERE id=$1",
    )
    .bind(fixture.session_id)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    for _ in 0..5 {
        validate_session(&fixture.pool, fixture.raw_token.token_hash(), now)
            .await
            .unwrap()
            .unwrap();
    }
    let after: (DateTime<Utc>, DateTime<Utc>, String) = sqlx::query_as(
        "SELECT last_seen_at,idle_expires_at,xmin::text FROM auth_sessions WHERE id=$1",
    )
    .bind(fixture.session_id)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(before, after);
    let renewed = pulse(&fixture, &fixture.raw_token, now)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(renewed.last_seen_at, now);
    let version: String = sqlx::query_scalar("SELECT xmin::text FROM auth_sessions WHERE id=$1")
        .bind(fixture.session_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_ne!(version, before.2);
    for _ in 0..5 {
        pulse(&fixture, &fixture.raw_token, now + Duration::minutes(4))
            .await
            .unwrap()
            .unwrap();
    }
    let duplicate_version: String =
        sqlx::query_scalar("SELECT xmin::text FROM auth_sessions WHERE id=$1")
            .bind(fixture.session_id)
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(version, duplicate_version);
    let renewed_again = pulse(&fixture, &fixture.raw_token, now + Duration::minutes(5))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(renewed_again.last_seen_at, now + Duration::minutes(5));
}

#[tokio::test]
async fn selected_revocation_cannot_cross_user_ownership() {
    let fixture = SessionFixture::active("owner_session").await;
    let (_other_user, other_session) = fixture.create_other_user_session().await;
    let revoked = revoke_sessions(
        &fixture.pool,
        fixture.user_id,
        SessionRevocationTarget::Session(other_session),
        SessionRevocationReason::UserSelected,
        Utc::now(),
    )
    .await
    .unwrap();

    assert!(revoked.is_empty());
    let revoked_at: Option<DateTime<Utc>> =
        sqlx::query_scalar("SELECT revoked_at FROM auth_sessions WHERE id = $1")
            .bind(other_session)
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert!(revoked_at.is_none());

    let own_revoked = revoke_sessions(
        &fixture.pool,
        fixture.user_id,
        SessionRevocationTarget::Session(fixture.session_id),
        SessionRevocationReason::UserSelected,
        Utc::now(),
    )
    .await
    .unwrap();
    assert_eq!(own_revoked, vec![fixture.session_id]);
    let own_reason: Option<String> =
        sqlx::query_scalar("SELECT revocation_reason FROM auth_sessions WHERE id = $1")
            .bind(fixture.session_id)
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(own_reason.as_deref(), Some("user_selected"));
}

#[tokio::test]
async fn failure_buckets_update_atomically_and_success_clears_only_identifier() {
    let fixture = SessionFixture::active("throttle_atomicity").await;
    record_login_failure(
        &fixture.pool,
        fixture.identifier_hash,
        fixture.source_hash,
        Utc::now(),
    )
    .await
    .unwrap();
    assert_eq!(fixture.failure_count(BucketKind::Identifier).await, 1);
    assert_eq!(fixture.failure_count(BucketKind::Source).await, 1);

    let new_token = RawSessionToken::generate().unwrap();
    fixture.create_same_user_session(&new_token).await;
    assert_eq!(fixture.failure_count(BucketKind::Identifier).await, 0);
    assert_eq!(fixture.failure_count(BucketKind::Source).await, 1);
}

#[tokio::test]
async fn throttle_window_resets_and_reports_only_active_blocks() {
    let fixture = SessionFixture::active("throttle_window").await;
    let now = database_timestamp(Utc::now());
    for _ in 0..5 {
        record_login_failure(
            &fixture.pool,
            fixture.identifier_hash,
            fixture.source_hash,
            now,
        )
        .await
        .unwrap();
    }
    let state = check_login_throttles(
        &fixture.pool,
        fixture.identifier_hash,
        fixture.source_hash,
        now,
    )
    .await
    .unwrap();
    assert_eq!(state.identifier_failure_count, 5);
    assert_eq!(
        state.identifier_blocked_until,
        Some(now + Duration::seconds(1))
    );
    assert!(state.source_blocked_until.is_none());

    sqlx::query(
        "UPDATE auth_login_throttles SET window_started_at = $1, updated_at = $1 \
         WHERE bucket_kind = 'identifier' AND bucket_hash = $2",
    )
    .bind(now - Duration::minutes(16))
    .bind(fixture.identifier_hash.as_bytes().as_slice())
    .execute(&fixture.pool)
    .await
    .unwrap();
    record_login_failure(
        &fixture.pool,
        fixture.identifier_hash,
        fixture.source_hash,
        now,
    )
    .await
    .unwrap();
    assert_eq!(fixture.failure_count(BucketKind::Identifier).await, 1);
}

#[tokio::test]
async fn inactive_revoked_and_expired_sessions_fail_authentication_and_revalidation() {
    for (label, mutation) in [
        ("revoked", "UPDATE auth_sessions SET revoked_at = now(), revocation_reason = 'logout' WHERE id = $1"),
        ("idle", "UPDATE auth_sessions SET idle_expires_at = created_at + interval '1 second' WHERE id = $1"),
        ("absolute", "UPDATE auth_sessions SET absolute_expires_at = created_at + interval '1 second', idle_expires_at = created_at + interval '1 second' WHERE id = $1"),
    ] {
        let fixture = SessionFixture::active(&format!("invalid_{label}")).await;
        sqlx::query(mutation)
            .bind(fixture.session_id)
            .execute(&fixture.pool)
            .await
            .unwrap();
        let now = Utc::now() + Duration::seconds(2);
        assert!(validate_session(
            &fixture.pool,
            fixture.raw_token.token_hash(),
            now,)
        .await
        .unwrap()
        .is_none());
        assert!(!revalidate_session(&fixture.pool, fixture.session_id, fixture.user_id, now)
            .await
            .unwrap());
    }

    let fixture = SessionFixture::active("invalid_inactive_user").await;
    sqlx::query("UPDATE users SET status = 'inactive' WHERE id = $1")
        .bind(fixture.user_id)
        .execute(&fixture.pool)
        .await
        .unwrap();
    assert!(
        validate_session(&fixture.pool, fixture.raw_token.token_hash(), Utc::now(),)
            .await
            .unwrap()
            .is_none()
    );
}

#[tokio::test]
async fn listing_and_user_revocation_include_only_owned_active_sessions() {
    let fixture = SessionFixture::active("listing_revocation").await;
    let token_two = RawSessionToken::from_bytes([41; 32]);
    let token_three = RawSessionToken::from_bytes([42; 32]);
    let session_two = fixture.create_same_user_session(&token_two).await;
    let session_three = fixture.create_same_user_session(&token_three).await;
    let (_other_user, _other_session) = fixture.create_other_user_session().await;

    let listed = list_user_sessions(&fixture.pool, fixture.user_id, Utc::now())
        .await
        .unwrap();
    assert_eq!(listed.len(), 3);
    assert!(listed
        .iter()
        .all(|session| session.user_id == fixture.user_id));

    let revoked = revoke_sessions(
        &fixture.pool,
        fixture.user_id,
        SessionRevocationTarget::User {
            except_session_id: Some(fixture.session_id),
        },
        SessionRevocationReason::LogoutAll,
        Utc::now(),
    )
    .await
    .unwrap();
    assert_eq!(revoked.len(), 2);
    assert!(revoked.contains(&session_two));
    assert!(revoked.contains(&session_three));
    let reasons: Vec<String> = sqlx::query_scalar(
        "SELECT COALESCE(revocation_reason, '') FROM auth_sessions \
         WHERE id = ANY($1) ORDER BY id",
    )
    .bind(&revoked)
    .fetch_all(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(reasons, vec!["logout_all", "logout_all"]);
    assert!(revalidate_session(
        &fixture.pool,
        fixture.session_id,
        fixture.user_id,
        Utc::now()
    )
    .await
    .unwrap());

    let final_revoked = revoke_sessions(
        &fixture.pool,
        fixture.user_id,
        SessionRevocationTarget::User {
            except_session_id: None,
        },
        SessionRevocationReason::LogoutAll,
        Utc::now(),
    )
    .await
    .unwrap();
    assert_eq!(final_revoked, vec![fixture.session_id]);
}

#[tokio::test]
async fn token_collisions_roll_back_insert_and_pending_staging_across_all_columns() {
    for column in [
        "current_token_hash",
        "previous_token_hash",
        "pending_token_hash",
    ] {
        let fixture = SessionFixture::active(&format!("token_collision_{column}")).await;
        let now = database_timestamp(Utc::now());
        fixture.set_rotated_at(now - Duration::minutes(16)).await;
        let candidate = pending_session_token(
            &SessionHmacKey::for_tests([11; 32]),
            Uuid::nil(),
            fixture.session_id,
            &fixture.raw_token,
            now,
        );
        let second = RawSessionToken::from_bytes([72; 32]);
        let second_id = fixture.create_same_user_session(&second).await;
        let query = match column {
            "current_token_hash" => "UPDATE auth_sessions SET current_token_hash=$1 WHERE id=$2",
            "previous_token_hash" => "UPDATE auth_sessions SET previous_token_hash=$1,previous_token_valid_until=rotated_at+interval '60 seconds' WHERE id=$2",
            _ => "UPDATE auth_sessions SET pending_token_hash=$1,pending_created_at=rotated_at WHERE id=$2",
        };
        sqlx::query(query)
            .bind(candidate.token_hash().as_bytes().as_slice())
            .bind(second_id)
            .execute(&fixture.pool)
            .await
            .unwrap();
        let before = fixture.hashes().await;
        assert!(
            pulse(&fixture, &fixture.raw_token, now).await.is_err(),
            "{column}"
        );
        assert_eq!(before, fixture.hashes().await);
        let pending: bool = sqlx::query_scalar(
            "SELECT pending_token_hash IS NOT NULL FROM auth_sessions WHERE id=$1",
        )
        .bind(fixture.session_id)
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
        assert!(!pending);
        let colliding_insert = new_session(
            Uuid::new_v4(),
            fixture.user_id,
            candidate.token_hash(),
            false,
            now,
        );
        assert!(
            create_login_session(&fixture.pool, &colliding_insert, fixture.identifier_hash)
                .await
                .is_err(),
            "{column}"
        );
    }
}

#[tokio::test]
async fn concurrent_same_digest_insert_is_serialized_and_pending_promotion_races_are_safe() {
    let fixture = SessionFixture::concurrent("cross_column_collision", 4).await;
    let now = database_timestamp(Utc::now());
    let shared = RawSessionToken::from_bytes([73; 32]);
    let left = new_session(
        Uuid::new_v4(),
        fixture.user_id,
        shared.token_hash(),
        false,
        now,
    );
    let right = new_session(
        Uuid::new_v4(),
        fixture.user_id,
        shared.token_hash(),
        false,
        now,
    );
    let barrier = Arc::new(Barrier::new(2));
    let (left_result, right_result) = tokio::join!(
        async {
            barrier.wait().await;
            create_login_session(&fixture.pool, &left, fixture.identifier_hash).await
        },
        async {
            barrier.wait().await;
            create_login_session(&fixture.pool, &right, fixture.identifier_hash).await
        }
    );
    assert_eq!(
        left_result.is_ok() as usize + right_result.is_ok() as usize,
        1
    );
    fixture.set_rotated_at(now - Duration::minutes(16)).await;
    let b = pulse(&fixture, &fixture.raw_token, now)
        .await
        .unwrap()
        .unwrap()
        .replacement
        .unwrap();
    let (a, b_result) = tokio::join!(
        pulse(&fixture, &fixture.raw_token, now + Duration::seconds(1)),
        maintain_session(
            &fixture.pool,
            fixture.user_id,
            fixture.session_id,
            b.token_hash(),
            now + Duration::seconds(1),
            None
        )
    );
    let a = a.unwrap().unwrap();
    let promoted = b_result.unwrap().unwrap();
    if let Some(replayed) = a.replacement {
        assert_eq!(replayed.token_hash(), b.token_hash());
    }
    assert_eq!(promoted.presented_as, PresentedTokenKind::Current);
    assert_eq!(
        fixture.hashes().await.0.as_slice(),
        b.token_hash().as_bytes()
    );
}

#[tokio::test]
async fn cleanup_cancellation_rolls_back_prior_steps_and_retries() {
    let fixture = SessionFixture::concurrent("cleanup_cancel_rollback", 5).await;
    let now = database_timestamp(Utc::now());
    fixture.set_rotated_at(now - Duration::minutes(2)).await;
    sqlx::query("UPDATE auth_sessions SET previous_token_hash=decode(repeat('fe',32),'hex'), previous_token_valid_until=$1 - interval '1 minute' WHERE id=$2")
        .bind(now).bind(fixture.session_id).execute(&fixture.pool).await.unwrap();
    let mut blocker = fixture.pool.begin().await.unwrap();
    sqlx::query("LOCK TABLE auth_login_throttles IN ACCESS EXCLUSIVE MODE")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let worker_pool = fixture.pool.clone();
    let task = tokio::spawn(async move { cleanup_auth_state(&worker_pool, now).await });
    // The cleanup reached its third statement after changing previous-token
    // state inside its still-uncommitted transaction.
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let waiting: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE relation='auth_login_throttles'::regclass AND NOT granted)")
                .fetch_one(&fixture.pool).await.unwrap();
            if waiting { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    blocker.rollback().await.unwrap();
    let retry = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        cleanup_auth_state(&fixture.pool, now),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        retry.previous_tokens_cleared, 1,
        "cancelled transaction must preserve its earlier token cleanup for retry"
    );
    assert!(fixture.hashes().await.1.is_none());
}

#[tokio::test]
async fn cleanup_skips_locked_rows_and_resumes_after_release() {
    let fixture = SessionFixture::concurrent("cleanup_skip_locked", 5).await;
    let now = database_timestamp(Utc::now());
    fixture.set_rotated_at(now - Duration::minutes(2)).await;
    sqlx::query("UPDATE auth_sessions SET previous_token_hash=decode(repeat('fe',32),'hex'), previous_token_valid_until=$1 - interval '1 minute' WHERE id=$2")
        .bind(now).bind(fixture.session_id).execute(&fixture.pool).await.unwrap();
    let mut blocker = fixture.pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM auth_sessions WHERE id=$1 FOR UPDATE")
        .bind(fixture.session_id)
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    let skipped = tokio::time::timeout(
        std::time::Duration::from_secs(5),
        cleanup_auth_state(&fixture.pool, now),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(skipped.previous_tokens_cleared, 0);
    blocker.rollback().await.unwrap();
    let retry = cleanup_auth_state(&fixture.pool, now).await.unwrap();
    assert_eq!(retry.previous_tokens_cleared, 1);
}

#[tokio::test]
async fn cleanup_retains_recent_rows_and_deletes_exactly_one_batch() {
    let fixture = SessionFixture::active("cleanup_batch").await;
    let now = database_timestamp(Utc::now());
    fixture.set_rotated_at(now - Duration::minutes(2)).await;
    sqlx::query(
        "UPDATE auth_sessions SET previous_token_hash = decode(repeat('fe', 32), 'hex'), \
         previous_token_valid_until = $1 - interval '1 minute' WHERE id = $2",
    )
    .bind(now)
    .bind(fixture.session_id)
    .execute(&fixture.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO auth_sessions \
         (id, user_id, current_token_hash, remember_me, device_label, created_at, last_seen_at, \
          idle_expires_at, absolute_expires_at, rotated_at) \
         SELECT gen_random_uuid(), $1, decode(lpad(to_hex(value), 64, '0'), 'hex'), false, \
                'Cleanup fixture', $2 - interval '61 days', $2 - interval '61 days', \
                $2 - interval '60 days', $2 - interval '60 days', $2 - interval '61 days' \
         FROM generate_series(1, 105) AS value",
    )
    .bind(fixture.user_id)
    .bind(now)
    .execute(&fixture.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO auth_login_throttles \
         (bucket_kind, bucket_hash, failure_count, window_started_at, updated_at) \
         SELECT 'identifier', decode(lpad(to_hex(value + 1000), 64, '0'), 'hex'), 1, \
                $1 - interval '2 days', $1 - interval '2 days' \
         FROM generate_series(1, 105) AS value",
    )
    .bind(now)
    .execute(&fixture.pool)
    .await
    .unwrap();

    let result = cleanup_auth_state(&fixture.pool, now).await.unwrap();
    assert_eq!(result.previous_tokens_cleared, 1);
    assert_eq!(result.sessions_deleted, 100);
    assert_eq!(result.throttles_deleted, 100);
    let expired_sessions: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM auth_sessions WHERE absolute_expires_at < $1 - interval '30 days'",
    )
    .bind(now)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    let stale_throttles: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM auth_login_throttles WHERE updated_at < $1 - interval '1 day'",
    )
    .bind(now)
    .fetch_one(&fixture.pool)
    .await
    .unwrap();
    assert_eq!(expired_sessions, 5);
    assert_eq!(stale_throttles, 5);
}

#[tokio::test]
async fn pending_credentials_cannot_revive_idle_absolute_revoked_or_inactive_sessions() {
    for reason in ["idle", "absolute", "revoked", "inactive"] {
        let fixture = SessionFixture::active(&format!("pending_denial_{reason}")).await;
        let now = database_timestamp(Utc::now());
        fixture.set_rotated_at(now - Duration::minutes(16)).await;
        let b = pulse(&fixture, &fixture.raw_token, now)
            .await
            .unwrap()
            .unwrap()
            .replacement
            .unwrap();
        match reason {
            "revoked" => {
                revoke_sessions(
                    &fixture.pool,
                    fixture.user_id,
                    SessionRevocationTarget::Session(fixture.session_id),
                    SessionRevocationReason::Logout,
                    now,
                )
                .await
                .unwrap();
                let pending: bool = sqlx::query_scalar(
                    "SELECT pending_token_hash IS NOT NULL FROM auth_sessions WHERE id=$1",
                )
                .bind(fixture.session_id)
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
                assert!(!pending);
            }
            "inactive" => {
                sqlx::query("UPDATE users SET status='inactive' WHERE id=$1")
                    .bind(fixture.user_id)
                    .execute(&fixture.pool)
                    .await
                    .unwrap();
            }
            _ => {
                let query = if reason == "absolute" {
                    "UPDATE auth_sessions SET idle_expires_at=$1,absolute_expires_at=$1 WHERE id=$2"
                } else {
                    "UPDATE auth_sessions SET idle_expires_at=$1 WHERE id=$2"
                };
                sqlx::query(query)
                    .bind(now + Duration::seconds(1))
                    .bind(fixture.session_id)
                    .execute(&fixture.pool)
                    .await
                    .unwrap();
            }
        }
        let version: String =
            sqlx::query_scalar("SELECT xmin::text FROM auth_sessions WHERE id=$1")
                .bind(fixture.session_id)
                .fetch_one(&fixture.pool)
                .await
                .unwrap();
        for raw in [&fixture.raw_token, &b] {
            assert!(
                validate_session(&fixture.pool, raw.token_hash(), now + Duration::seconds(2))
                    .await
                    .unwrap()
                    .is_none(),
                "{reason}"
            );
            assert!(
                pulse(&fixture, raw, now + Duration::seconds(2))
                    .await
                    .unwrap()
                    .is_none(),
                "{reason}"
            );
            assert!(maintain_session(
                &fixture.pool,
                fixture.user_id,
                fixture.session_id,
                raw.token_hash(),
                now + Duration::seconds(2),
                None
            )
            .await
            .unwrap()
            .is_none());
        }
        let after: String = sqlx::query_scalar("SELECT xmin::text FROM auth_sessions WHERE id=$1")
            .bind(fixture.session_id)
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
        assert_eq!(version, after, "{reason}");
    }
}

#[tokio::test]
async fn human_activity_clamps_both_lifetimes_to_the_original_absolute_expiry() {
    for remembered in [false, true] {
        let fixture =
            SessionFixture::active(&format!("activity_absolute_clamp_{remembered}")).await;
        let now = database_timestamp(Utc::now());
        let absolute = now + Duration::minutes(1);
        sqlx::query("UPDATE auth_sessions SET created_at=$1-interval '10 minutes',last_seen_at=$1-interval '6 minutes',idle_expires_at=$1+interval '30 seconds',absolute_expires_at=$2,remember_me=$3 WHERE id=$4")
            .bind(now).bind(absolute).bind(remembered).bind(fixture.session_id).execute(&fixture.pool).await.unwrap();
        let result = pulse(&fixture, &fixture.raw_token, now)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(result.idle_expires_at, absolute);
        assert_eq!(result.absolute_expires_at, absolute);
        assert!(pulse(&fixture, &fixture.raw_token, absolute)
            .await
            .unwrap()
            .is_none());
    }
}
