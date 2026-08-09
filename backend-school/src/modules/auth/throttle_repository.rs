use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};

use crate::error::AppError;

use super::{
    session_crypto::ThrottleBucketHash,
    session_policy::{BucketKind, ThrottlePolicy, THROTTLE_WINDOW},
};

const IDENTIFIER_LOCK_DOMAIN: i64 = 0x534f_4944_4c4f_4301;
const SOURCE_LOCK_DOMAIN: i64 = 0x534f_5352_4c4f_4301;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ThrottleState {
    pub identifier_failure_count: i32,
    pub identifier_blocked_until: Option<DateTime<Utc>>,
    pub source_failure_count: i32,
    pub source_blocked_until: Option<DateTime<Utc>>,
}

impl ThrottleState {
    pub fn blocked_until(self) -> Option<DateTime<Utc>> {
        [self.identifier_blocked_until, self.source_blocked_until]
            .into_iter()
            .flatten()
            .max()
    }
}

#[derive(FromRow)]
struct BucketRow {
    bucket_kind: String,
    failure_count: i32,
    window_started_at: DateTime<Utc>,
    blocked_until: Option<DateTime<Utc>>,
}

pub async fn check_login_throttles(
    pool: &PgPool,
    identifier_hash: ThrottleBucketHash,
    source_hash: ThrottleBucketHash,
    now: DateTime<Utc>,
) -> Result<ThrottleState, AppError> {
    let rows = sqlx::query_as::<_, BucketRow>(
        r#"
        SELECT bucket_kind, failure_count, window_started_at, blocked_until
        FROM auth_login_throttles
        WHERE (bucket_kind = 'identifier' AND bucket_hash = $1)
           OR (bucket_kind = 'source' AND bucket_hash = $2)
        "#,
    )
    .bind(identifier_hash.as_bytes().as_slice())
    .bind(source_hash.as_bytes().as_slice())
    .fetch_all(pool)
    .await
    .map_err(session_store_error)?;

    Ok(state_from_rows(rows, now))
}

pub async fn record_login_failure(
    pool: &PgPool,
    identifier_hash: ThrottleBucketHash,
    source_hash: ThrottleBucketHash,
    now: DateTime<Utc>,
) -> Result<ThrottleState, AppError> {
    let mut transaction = pool.begin().await.map_err(session_store_error)?;
    lock_failure_buckets(&mut transaction, identifier_hash, source_hash).await?;

    let identifier = update_bucket(
        &mut transaction,
        BucketKind::Identifier,
        identifier_hash,
        now,
    )
    .await?;
    let source = update_bucket(&mut transaction, BucketKind::Source, source_hash, now).await?;
    transaction.commit().await.map_err(session_store_error)?;

    Ok(ThrottleState {
        identifier_failure_count: identifier.failure_count,
        identifier_blocked_until: active_block(identifier.blocked_until, now),
        source_failure_count: source.failure_count,
        source_blocked_until: active_block(source.blocked_until, now),
    })
}

async fn update_bucket(
    transaction: &mut Transaction<'_, Postgres>,
    kind: BucketKind,
    hash: ThrottleBucketHash,
    now: DateTime<Utc>,
) -> Result<BucketRow, AppError> {
    let kind_name = bucket_kind_name(kind);
    let existing = sqlx::query_as::<_, BucketRow>(
        r#"
        SELECT bucket_kind, failure_count, window_started_at, blocked_until
        FROM auth_login_throttles
        WHERE bucket_kind = $1 AND bucket_hash = $2
        FOR UPDATE
        "#,
    )
    .bind(kind_name)
    .bind(hash.as_bytes().as_slice())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(session_store_error)?;

    let (failure_count, window_started_at) = match existing {
        Some(row) if row.window_started_at > now - THROTTLE_WINDOW => {
            (row.failure_count.saturating_add(1), row.window_started_at)
        }
        _ => (1, now),
    };
    let blocked_until = ThrottlePolicy
        .delay(kind, failure_count.max(0) as u32)
        .map(|delay| now + delay);

    sqlx::query(
        r#"
        INSERT INTO auth_login_throttles (
            bucket_kind, bucket_hash, failure_count, window_started_at, blocked_until, updated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (bucket_kind, bucket_hash) DO UPDATE
        SET failure_count = EXCLUDED.failure_count,
            window_started_at = EXCLUDED.window_started_at,
            blocked_until = EXCLUDED.blocked_until,
            updated_at = EXCLUDED.updated_at
        "#,
    )
    .bind(kind_name)
    .bind(hash.as_bytes().as_slice())
    .bind(failure_count)
    .bind(window_started_at)
    .bind(blocked_until)
    .bind(now)
    .execute(&mut **transaction)
    .await
    .map_err(session_store_error)?;

    Ok(BucketRow {
        bucket_kind: kind_name.to_string(),
        failure_count,
        window_started_at,
        blocked_until,
    })
}

async fn lock_failure_buckets(
    transaction: &mut Transaction<'_, Postgres>,
    identifier_hash: ThrottleBucketHash,
    source_hash: ThrottleBucketHash,
) -> Result<(), AppError> {
    let mut lock_keys = [
        bucket_lock_key(BucketKind::Identifier, identifier_hash),
        bucket_lock_key(BucketKind::Source, source_hash),
    ];
    lock_keys.sort_unstable();
    for (index, lock_key) in lock_keys.into_iter().enumerate() {
        if index == 1 && lock_key == lock_keys[0] {
            continue;
        }
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(lock_key)
            .execute(&mut **transaction)
            .await
            .map_err(session_store_error)?;
    }
    Ok(())
}

fn bucket_lock_key(kind: BucketKind, hash: ThrottleBucketHash) -> i64 {
    let mut prefix = [0_u8; 8];
    prefix.copy_from_slice(&hash.as_bytes()[..8]);
    let domain = match kind {
        BucketKind::Identifier => IDENTIFIER_LOCK_DOMAIN,
        BucketKind::Source => SOURCE_LOCK_DOMAIN,
    };
    i64::from_be_bytes(prefix) ^ domain
}

fn state_from_rows(rows: Vec<BucketRow>, now: DateTime<Utc>) -> ThrottleState {
    let mut state = ThrottleState {
        identifier_failure_count: 0,
        identifier_blocked_until: None,
        source_failure_count: 0,
        source_blocked_until: None,
    };

    for row in rows {
        let active = row.window_started_at > now - THROTTLE_WINDOW;
        let failure_count = if active { row.failure_count } else { 0 };
        let blocked_until = if active {
            active_block(row.blocked_until, now)
        } else {
            None
        };
        match row.bucket_kind.as_str() {
            "identifier" => {
                state.identifier_failure_count = failure_count;
                state.identifier_blocked_until = blocked_until;
            }
            "source" => {
                state.source_failure_count = failure_count;
                state.source_blocked_until = blocked_until;
            }
            _ => {}
        }
    }
    state
}

fn active_block(blocked_until: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Option<DateTime<Utc>> {
    blocked_until.filter(|blocked_until| *blocked_until > now)
}

fn bucket_kind_name(kind: BucketKind) -> &'static str {
    match kind {
        BucketKind::Identifier => "identifier",
        BucketKind::Source => "source",
    }
}

fn session_store_error(_error: sqlx::Error) -> AppError {
    AppError::ServiceUnavailable("session_store".to_string())
}
