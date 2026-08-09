use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use chrono::{DateTime, Duration, Timelike, Utc};
use sqlx::PgPool;
use tokio::sync::Barrier;
use uuid::Uuid;

use crate::test_helpers::{
    create_named_test_pool, create_named_test_pool_with_max_connections, create_test_user,
    run_test_migrations,
};

use super::{
    session_crypto::{
        identifier_bucket, source_bucket, RawSessionToken, SessionHmacKey, ThrottleBucketHash,
        TokenHash,
    },
    session_policy::{BucketKind, SessionLifetime, SessionTimes},
    session_repository::{
        authenticate_and_maintain, cleanup_auth_state, create_login_session, list_user_sessions,
        revalidate_session, revoke_sessions, NewSession, PresentedTokenKind,
        SessionMaintenanceMode, SessionRevocationReason, SessionRevocationTarget,
    },
    throttle_repository::{check_login_throttles, record_login_failure},
};

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
        sqlx::query("UPDATE auth_sessions SET last_seen_at = $1 WHERE id = $2")
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

#[tokio::test]
async fn concurrent_rotation_keeps_one_current_hash_and_one_grace_hash() {
    let fixture = SessionFixture::concurrent("concurrent_rotation", 4).await;
    let old = fixture.raw_token.token_hash();
    fixture
        .set_rotated_at(Utc::now() - Duration::minutes(16))
        .await;
    let now = database_timestamp(Utc::now());
    let barrier = Arc::new(Barrier::new(2));

    let (left, right) = tokio::join!(
        async {
            barrier.wait().await;
            authenticate_and_maintain(
                &fixture.pool,
                old,
                now,
                SessionMaintenanceMode::RotateAndTouch,
                || RawSessionToken::from_bytes([1; 32]),
            )
            .await
        },
        async {
            barrier.wait().await;
            authenticate_and_maintain(
                &fixture.pool,
                old,
                now,
                SessionMaintenanceMode::RotateAndTouch,
                || RawSessionToken::from_bytes([2; 32]),
            )
            .await
        },
    );

    assert!(left.is_ok());
    assert!(right.is_ok());
    let left = left.unwrap().expect("left request must authenticate");
    let right = right.unwrap().expect("right request must authenticate");
    assert_eq!(
        left.replacement.is_some() as usize + right.replacement.is_some() as usize,
        1
    );
    let (current, previous, rotated_at) = fixture.hashes().await;
    assert_eq!(previous.as_deref(), Some(old.as_bytes().as_slice()));
    assert_ne!(current.as_slice(), old.as_bytes().as_slice());
    assert_eq!(rotated_at, left.rotated_at.max(right.rotated_at));
}

#[tokio::test]
async fn current_previous_and_touch_only_maintenance_follow_the_contract() {
    let fixture = SessionFixture::active("current_previous_touch").await;
    let old = fixture.raw_token.token_hash();
    let now = database_timestamp(Utc::now());
    fixture.set_rotated_at(now - Duration::minutes(16)).await;

    let current = authenticate_and_maintain(
        &fixture.pool,
        old,
        now,
        SessionMaintenanceMode::RotateAndTouch,
        || RawSessionToken::from_bytes([21; 32]),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(current.presented_as, PresentedTokenKind::Current);
    let replacement = current.replacement.unwrap();

    let previous = authenticate_and_maintain(
        &fixture.pool,
        old,
        now + Duration::seconds(1),
        SessionMaintenanceMode::RotateAndTouch,
        || panic!("previous credentials must never rotate"),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(previous.presented_as, PresentedTokenKind::Previous);
    assert!(previous.replacement.is_none());

    let after_grace = now + Duration::seconds(61);
    assert!(authenticate_and_maintain(
        &fixture.pool,
        old,
        after_grace,
        SessionMaintenanceMode::RotateAndTouch,
        || panic!("expired previous credentials must not rotate"),
    )
    .await
    .unwrap()
    .is_none());

    let replacement_hash = replacement.token_hash();
    fixture
        .set_rotated_at(after_grace - Duration::minutes(16))
        .await;
    fixture
        .set_last_seen_at(after_grace - Duration::minutes(6))
        .await;
    let generated = AtomicBool::new(false);
    let before = fixture.hashes().await;
    let maintained = authenticate_and_maintain(
        &fixture.pool,
        replacement_hash,
        after_grace,
        SessionMaintenanceMode::TouchOnly,
        || {
            generated.store(true, Ordering::SeqCst);
            RawSessionToken::from_bytes([22; 32])
        },
    )
    .await
    .unwrap()
    .unwrap();
    let after = fixture.hashes().await;
    assert!(!generated.load(Ordering::SeqCst));
    assert!(maintained.replacement.is_none());
    assert_eq!(before.0, after.0);
    assert_eq!(before.1, after.1);
    let touched_at: DateTime<Utc> =
        sqlx::query_scalar("SELECT last_seen_at FROM auth_sessions WHERE id = $1")
            .bind(fixture.session_id)
            .fetch_one(&fixture.pool)
            .await
            .unwrap();
    assert_eq!(touched_at, after_grace);
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
        assert!(authenticate_and_maintain(
            &fixture.pool,
            fixture.raw_token.token_hash(),
            now,
            SessionMaintenanceMode::RotateAndTouch,
            || panic!("invalid session must not rotate"),
        )
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
    assert!(authenticate_and_maintain(
        &fixture.pool,
        fixture.raw_token.token_hash(),
        Utc::now(),
        SessionMaintenanceMode::RotateAndTouch,
        || panic!("inactive user session must not rotate"),
    )
    .await
    .unwrap()
    .is_none());
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
async fn token_collisions_roll_back_insert_and_rotation() {
    let fixture = SessionFixture::active("token_collision").await;
    let duplicate = new_session(
        Uuid::new_v4(),
        fixture.user_id,
        fixture.raw_token.token_hash(),
        false,
        Utc::now(),
    );
    let insert_error = create_login_session(&fixture.pool, &duplicate, fixture.identifier_hash)
        .await
        .unwrap_err();
    assert_eq!(
        insert_error.status_code(),
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    );
    let session_count: i64 = sqlx::query_scalar("SELECT count(*) FROM auth_sessions")
        .fetch_one(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(session_count, 1);

    fixture
        .set_rotated_at(Utc::now() - Duration::minutes(16))
        .await;
    let before = fixture.hashes().await;
    let rotation_result = authenticate_and_maintain(
        &fixture.pool,
        fixture.raw_token.token_hash(),
        Utc::now(),
        SessionMaintenanceMode::RotateAndTouch,
        || RawSessionToken::from_bytes(*fixture.raw_token.as_bytes()),
    )
    .await;
    let rotation_error = match rotation_result {
        Ok(_) => panic!("same-token rotation must be rejected"),
        Err(error) => error,
    };
    assert_eq!(
        rotation_error.status_code(),
        axum::http::StatusCode::SERVICE_UNAVAILABLE
    );
    let after = fixture.hashes().await;
    assert_eq!(before, after);
}

#[tokio::test]
async fn cross_column_and_concurrent_same_digest_collisions_are_rejected() {
    let fixture = SessionFixture::concurrent("cross_column_collision", 4).await;
    let old = fixture.raw_token.token_hash();
    let now = database_timestamp(Utc::now());
    fixture.set_rotated_at(now - Duration::minutes(16)).await;
    let rotated = authenticate_and_maintain(
        &fixture.pool,
        old,
        now,
        SessionMaintenanceMode::RotateAndTouch,
        || RawSessionToken::from_bytes([71; 32]),
    )
    .await
    .unwrap()
    .unwrap();
    let current = rotated.replacement.unwrap();

    let colliding_insert = new_session(Uuid::new_v4(), fixture.user_id, old, false, now);
    assert!(
        create_login_session(&fixture.pool, &colliding_insert, fixture.identifier_hash)
            .await
            .is_err()
    );

    let second = RawSessionToken::from_bytes([72; 32]);
    let second_id = fixture.create_same_user_session(&second).await;
    fixture
        .set_rotated_at_for(second_id, now - Duration::minutes(16))
        .await;
    assert!(authenticate_and_maintain(
        &fixture.pool,
        second.token_hash(),
        now,
        SessionMaintenanceMode::RotateAndTouch,
        || RawSessionToken::from_bytes(*fixture.raw_token.as_bytes()),
    )
    .await
    .is_err());

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
    assert_eq!(
        left_result.is_err() as usize + right_result.is_err() as usize,
        1
    );
    assert!(authenticate_and_maintain(
        &fixture.pool,
        current.token_hash(),
        now,
        SessionMaintenanceMode::TouchOnly,
        || panic!("touch-only authentication must not generate"),
    )
    .await
    .unwrap()
    .is_some());
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
