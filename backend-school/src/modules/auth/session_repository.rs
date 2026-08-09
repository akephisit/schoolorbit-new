use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::error::AppError;

use super::{
    session_crypto::{RawSessionToken, ThrottleBucketHash, TokenHash},
    session_policy::{
        SessionLifetime, SessionTimes, CLEANUP_BATCH_SIZE, PREVIOUS_TOKEN_GRACE, ROTATION_INTERVAL,
        SESSION_RETENTION, THROTTLE_RETENTION, TOUCH_INTERVAL,
    },
};

const TOKEN_LOCK_DOMAIN: i64 = 0x534f_544f_4b45_4e01;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentedTokenKind {
    Current,
    Previous,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionMaintenanceMode {
    RotateAndTouch,
    TouchOnly,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRevocationTarget {
    Session(Uuid),
    User { except_session_id: Option<Uuid> },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SessionRevocationReason {
    Logout,
    UserSelected,
    LogoutAll,
    PasswordChanged,
}

impl SessionRevocationReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Logout => "logout",
            Self::UserSelected => "user_selected",
            Self::LogoutAll => "logout_all",
            Self::PasswordChanged => "password_changed",
        }
    }
}

pub struct NewSession {
    pub id: Uuid,
    pub user_id: Uuid,
    pub current_token_hash: TokenHash,
    pub remember_me: bool,
    pub device_label: String,
    pub times: SessionTimes,
}

#[derive(Clone, Debug, Eq, PartialEq, FromRow)]
pub struct SessionRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub device_label: String,
    pub remember_me: bool,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub idle_expires_at: DateTime<Utc>,
    pub absolute_expires_at: DateTime<Utc>,
    pub rotated_at: DateTime<Utc>,
    pub revoked_at: Option<DateTime<Utc>>,
    pub revocation_reason: Option<String>,
}

pub struct MaintainedSession {
    pub session_id: Uuid,
    pub user_id: Uuid,
    pub username: String,
    pub user_type: String,
    pub presented_as: PresentedTokenKind,
    pub remember_me: bool,
    pub rotated_at: DateTime<Utc>,
    pub absolute_expires_at: DateTime<Utc>,
    pub replacement: Option<RawSessionToken>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CleanupResult {
    pub previous_tokens_cleared: u64,
    pub sessions_deleted: u64,
    pub throttles_deleted: u64,
}

#[derive(FromRow)]
struct AuthenticationRow {
    id: Uuid,
    user_id: Uuid,
    current_token_hash: Vec<u8>,
    previous_token_hash: Option<Vec<u8>>,
    previous_token_valid_until: Option<DateTime<Utc>>,
    remember_me: bool,
    rotated_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    idle_expires_at: DateTime<Utc>,
    absolute_expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
    username: String,
    user_type: String,
    user_status: String,
}

const AUTHENTICATION_COLUMNS: &str = r#"
    SELECT s.id, s.user_id, s.current_token_hash, s.previous_token_hash,
           s.previous_token_valid_until, s.remember_me, s.rotated_at, s.last_seen_at,
           s.idle_expires_at, s.absolute_expires_at, s.revoked_at,
           u.username, u.user_type, u.status AS user_status
    FROM auth_sessions s
    JOIN users u ON u.id = s.user_id
    WHERE s.current_token_hash = $1 OR s.previous_token_hash = $1
    LIMIT 2
"#;

pub async fn create_login_session(
    pool: &PgPool,
    session: &NewSession,
    identifier_hash: ThrottleBucketHash,
) -> Result<(), AppError> {
    let mut transaction = pool.begin().await.map_err(session_store_error)?;
    lock_token_hash(&mut transaction, session.current_token_hash).await?;
    ensure_token_hash_available(&mut transaction, session.current_token_hash).await?;

    sqlx::query(
        r#"
        INSERT INTO auth_sessions (
            id, user_id, current_token_hash, remember_me, device_label,
            created_at, last_seen_at, idle_expires_at, absolute_expires_at, rotated_at
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
        "#,
    )
    .bind(session.id)
    .bind(session.user_id)
    .bind(session.current_token_hash.as_bytes().as_slice())
    .bind(session.remember_me)
    .bind(&session.device_label)
    .bind(session.times.created_at)
    .bind(session.times.last_seen_at)
    .bind(session.times.idle_expires_at)
    .bind(session.times.absolute_expires_at)
    .bind(session.times.rotated_at)
    .execute(&mut *transaction)
    .await
    .map_err(session_store_error)?;

    sqlx::query(
        "DELETE FROM auth_login_throttles WHERE bucket_kind = 'identifier' AND bucket_hash = $1",
    )
    .bind(identifier_hash.as_bytes().as_slice())
    .execute(&mut *transaction)
    .await
    .map_err(session_store_error)?;

    transaction.commit().await.map_err(session_store_error)
}

pub async fn authenticate_and_maintain<F>(
    pool: &PgPool,
    presented_hash: TokenHash,
    now: DateTime<Utc>,
    maintenance: SessionMaintenanceMode,
    generate: F,
) -> Result<Option<MaintainedSession>, AppError>
where
    F: FnOnce() -> RawSessionToken,
{
    let Some(initial) = load_authentication_row(pool, presented_hash).await? else {
        return Ok(None);
    };
    let Some(initial_kind) = presented_kind(&initial, presented_hash) else {
        return Err(unavailable());
    };

    if !base_session_is_valid(&initial, now) {
        return Ok(None);
    }

    let previous_expired = previous_token_is_expired(&initial, now);
    if initial_kind == PresentedTokenKind::Previous && !previous_token_is_valid(&initial, now) {
        clear_expired_previous_token(pool, initial.id, now).await?;
        return Ok(None);
    }

    let rotation_due = initial_kind == PresentedTokenKind::Current
        && maintenance == SessionMaintenanceMode::RotateAndTouch
        && initial.rotated_at <= now - ROTATION_INTERVAL;
    let touch_due = initial.last_seen_at <= now - TOUCH_INTERVAL;
    if !rotation_due && !touch_due && !previous_expired {
        return Ok(Some(maintained(initial, initial_kind, None)));
    }

    let mut transaction = pool.begin().await.map_err(session_store_error)?;
    let Some(mut locked) =
        load_authentication_row_for_update(&mut transaction, presented_hash).await?
    else {
        transaction.rollback().await.map_err(session_store_error)?;
        return Ok(None);
    };
    let Some(locked_kind) = presented_kind(&locked, presented_hash) else {
        return Err(unavailable());
    };

    if !base_session_is_valid(&locked, now) {
        transaction.rollback().await.map_err(session_store_error)?;
        return Ok(None);
    }

    if locked_kind == PresentedTokenKind::Previous && !previous_token_is_valid(&locked, now) {
        clear_expired_previous_token_in_transaction(&mut transaction, locked.id, now).await?;
        transaction.commit().await.map_err(session_store_error)?;
        return Ok(None);
    }

    let rotation_due = locked_kind == PresentedTokenKind::Current
        && maintenance == SessionMaintenanceMode::RotateAndTouch
        && locked.rotated_at <= now - ROTATION_INTERVAL;
    let touch_due = locked.last_seen_at <= now - TOUCH_INTERVAL;
    let previous_expired = previous_token_is_expired(&locked, now);
    let mut replacement = None;

    let current_token_hash = if rotation_due {
        let token = generate();
        let token_hash = token.token_hash();
        lock_token_hash(&mut transaction, token_hash).await?;
        ensure_token_hash_available(&mut transaction, token_hash).await?;
        replacement = Some(token);
        token_hash.as_bytes().to_vec()
    } else {
        locked.current_token_hash.clone()
    };

    let (previous_token_hash, previous_token_valid_until, rotated_at) = if rotation_due {
        (
            Some(locked.current_token_hash.clone()),
            Some(now + PREVIOUS_TOKEN_GRACE),
            now,
        )
    } else if previous_expired {
        (None, None, locked.rotated_at)
    } else {
        (
            locked.previous_token_hash.clone(),
            locked.previous_token_valid_until,
            locked.rotated_at,
        )
    };

    let lifetime = if locked.remember_me {
        SessionLifetime::remembered()
    } else {
        SessionLifetime::normal()
    };
    let last_seen_at = if touch_due { now } else { locked.last_seen_at };
    let idle_expires_at = if touch_due {
        (now + lifetime.idle).min(locked.absolute_expires_at)
    } else {
        locked.idle_expires_at
    };

    sqlx::query(
        r#"
        UPDATE auth_sessions
        SET current_token_hash = $1,
            previous_token_hash = $2,
            previous_token_valid_until = $3,
            rotated_at = $4,
            last_seen_at = $5,
            idle_expires_at = $6
        WHERE id = $7
        "#,
    )
    .bind(current_token_hash)
    .bind(previous_token_hash)
    .bind(previous_token_valid_until)
    .bind(rotated_at)
    .bind(last_seen_at)
    .bind(idle_expires_at)
    .bind(locked.id)
    .execute(&mut *transaction)
    .await
    .map_err(session_store_error)?;

    locked.current_token_hash.clear();
    locked.previous_token_hash = None;
    locked.previous_token_valid_until = None;
    locked.rotated_at = rotated_at;
    locked.last_seen_at = last_seen_at;
    locked.idle_expires_at = idle_expires_at;
    transaction.commit().await.map_err(session_store_error)?;

    Ok(Some(maintained(locked, locked_kind, replacement)))
}

pub async fn revalidate_session(
    pool: &PgPool,
    session_id: Uuid,
    user_id: Uuid,
    now: DateTime<Utc>,
) -> Result<bool, AppError> {
    sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM auth_sessions s
            JOIN users u ON u.id = s.user_id
            WHERE s.id = $1
              AND s.user_id = $2
              AND s.revoked_at IS NULL
              AND s.idle_expires_at > $3
              AND s.absolute_expires_at > $3
              AND u.status = 'active'
        )
        "#,
    )
    .bind(session_id)
    .bind(user_id)
    .bind(now)
    .fetch_one(pool)
    .await
    .map_err(session_store_error)
}

pub async fn list_user_sessions(
    pool: &PgPool,
    user_id: Uuid,
    now: DateTime<Utc>,
) -> Result<Vec<SessionRow>, AppError> {
    sqlx::query_as(
        r#"
        SELECT s.id, s.user_id, s.device_label, s.remember_me, s.created_at,
               s.last_seen_at, s.idle_expires_at, s.absolute_expires_at, s.rotated_at,
               s.revoked_at, s.revocation_reason
        FROM auth_sessions s
        JOIN users u ON u.id = s.user_id
        WHERE s.user_id = $1
          AND s.revoked_at IS NULL
          AND s.idle_expires_at > $2
          AND s.absolute_expires_at > $2
          AND u.status = 'active'
        ORDER BY s.last_seen_at DESC, s.id
        "#,
    )
    .bind(user_id)
    .bind(now)
    .fetch_all(pool)
    .await
    .map_err(session_store_error)
}

pub async fn revoke_sessions(
    pool: &PgPool,
    user_id: Uuid,
    target: SessionRevocationTarget,
    reason: SessionRevocationReason,
    now: DateTime<Utc>,
) -> Result<Vec<Uuid>, AppError> {
    let result = match target {
        SessionRevocationTarget::Session(session_id) => {
            sqlx::query_scalar(
                r#"
                UPDATE auth_sessions
                SET revoked_at = $1, revocation_reason = $2
                WHERE id = $3
                  AND user_id = $4
                  AND revoked_at IS NULL
                  AND idle_expires_at > $1
                  AND absolute_expires_at > $1
                RETURNING id
                "#,
            )
            .bind(now)
            .bind(reason.as_str())
            .bind(session_id)
            .bind(user_id)
            .fetch_all(pool)
            .await
        }
        SessionRevocationTarget::User { except_session_id } => {
            sqlx::query_scalar(
                r#"
                UPDATE auth_sessions
                SET revoked_at = $1, revocation_reason = $2
                WHERE user_id = $3
                  AND revoked_at IS NULL
                  AND idle_expires_at > $1
                  AND absolute_expires_at > $1
                  AND ($4::uuid IS NULL OR id <> $4)
                RETURNING id
                "#,
            )
            .bind(now)
            .bind(reason.as_str())
            .bind(user_id)
            .bind(except_session_id)
            .fetch_all(pool)
            .await
        }
    };

    result.map_err(session_store_error)
}

pub async fn cleanup_auth_state(
    pool: &PgPool,
    now: DateTime<Utc>,
) -> Result<CleanupResult, AppError> {
    let mut transaction = pool.begin().await.map_err(session_store_error)?;
    let previous_tokens_cleared = sqlx::query(
        r#"
        WITH candidates AS (
            SELECT id
            FROM auth_sessions
            WHERE previous_token_valid_until <= $1
            ORDER BY previous_token_valid_until, id
            LIMIT $2
            FOR UPDATE SKIP LOCKED
        )
        UPDATE auth_sessions s
        SET previous_token_hash = NULL, previous_token_valid_until = NULL
        FROM candidates c
        WHERE s.id = c.id
        "#,
    )
    .bind(now)
    .bind(CLEANUP_BATCH_SIZE)
    .execute(&mut *transaction)
    .await
    .map_err(session_store_error)?
    .rows_affected();

    let sessions_deleted = sqlx::query(
        r#"
        WITH candidates AS (
            SELECT id
            FROM auth_sessions
            WHERE (revoked_at IS NOT NULL AND revoked_at <= $1)
               OR (revoked_at IS NULL AND LEAST(idle_expires_at, absolute_expires_at) <= $1)
            ORDER BY COALESCE(revoked_at, LEAST(idle_expires_at, absolute_expires_at)), id
            LIMIT $2
            FOR UPDATE SKIP LOCKED
        )
        DELETE FROM auth_sessions s
        USING candidates c
        WHERE s.id = c.id
        "#,
    )
    .bind(now - SESSION_RETENTION)
    .bind(CLEANUP_BATCH_SIZE)
    .execute(&mut *transaction)
    .await
    .map_err(session_store_error)?
    .rows_affected();

    let throttles_deleted = sqlx::query(
        r#"
        WITH candidates AS (
            SELECT bucket_kind, bucket_hash
            FROM auth_login_throttles
            WHERE updated_at <= $1
            ORDER BY updated_at, bucket_kind, bucket_hash
            LIMIT $2
            FOR UPDATE SKIP LOCKED
        )
        DELETE FROM auth_login_throttles t
        USING candidates c
        WHERE t.bucket_kind = c.bucket_kind AND t.bucket_hash = c.bucket_hash
        "#,
    )
    .bind(now - THROTTLE_RETENTION)
    .bind(CLEANUP_BATCH_SIZE)
    .execute(&mut *transaction)
    .await
    .map_err(session_store_error)?
    .rows_affected();

    transaction.commit().await.map_err(session_store_error)?;
    Ok(CleanupResult {
        previous_tokens_cleared,
        sessions_deleted,
        throttles_deleted,
    })
}

async fn load_authentication_row(
    pool: &PgPool,
    presented_hash: TokenHash,
) -> Result<Option<AuthenticationRow>, AppError> {
    let rows = sqlx::query_as::<_, AuthenticationRow>(AUTHENTICATION_COLUMNS)
        .bind(presented_hash.as_bytes().as_slice())
        .fetch_all(pool)
        .await
        .map_err(session_store_error)?;
    exactly_one_or_none(rows)
}

async fn load_authentication_row_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    presented_hash: TokenHash,
) -> Result<Option<AuthenticationRow>, AppError> {
    let query = format!("{AUTHENTICATION_COLUMNS} FOR UPDATE OF s");
    let rows = sqlx::query_as::<_, AuthenticationRow>(&query)
        .bind(presented_hash.as_bytes().as_slice())
        .fetch_all(&mut **transaction)
        .await
        .map_err(session_store_error)?;
    exactly_one_or_none(rows)
}

fn exactly_one_or_none(
    mut rows: Vec<AuthenticationRow>,
) -> Result<Option<AuthenticationRow>, AppError> {
    match rows.len() {
        0 => Ok(None),
        1 => Ok(rows.pop()),
        _ => Err(unavailable()),
    }
}

fn presented_kind(
    row: &AuthenticationRow,
    presented_hash: TokenHash,
) -> Option<PresentedTokenKind> {
    if bool::from(
        row.current_token_hash
            .as_slice()
            .ct_eq(presented_hash.as_bytes().as_slice()),
    ) {
        Some(PresentedTokenKind::Current)
    } else if row
        .previous_token_hash
        .as_deref()
        .is_some_and(|hash| bool::from(hash.ct_eq(presented_hash.as_bytes().as_slice())))
    {
        Some(PresentedTokenKind::Previous)
    } else {
        None
    }
}

fn base_session_is_valid(row: &AuthenticationRow, now: DateTime<Utc>) -> bool {
    row.revoked_at.is_none()
        && row.user_status == "active"
        && row.idle_expires_at > now
        && row.absolute_expires_at > now
}

fn previous_token_is_valid(row: &AuthenticationRow, now: DateTime<Utc>) -> bool {
    row.previous_token_hash.is_some()
        && row
            .previous_token_valid_until
            .is_some_and(|valid_until| valid_until > now)
}

fn previous_token_is_expired(row: &AuthenticationRow, now: DateTime<Utc>) -> bool {
    row.previous_token_hash.is_some()
        && row
            .previous_token_valid_until
            .is_some_and(|valid_until| valid_until <= now)
}

fn maintained(
    row: AuthenticationRow,
    presented_as: PresentedTokenKind,
    replacement: Option<RawSessionToken>,
) -> MaintainedSession {
    MaintainedSession {
        session_id: row.id,
        user_id: row.user_id,
        username: row.username,
        user_type: row.user_type,
        presented_as,
        remember_me: row.remember_me,
        rotated_at: row.rotated_at,
        absolute_expires_at: row.absolute_expires_at,
        replacement,
    }
}

async fn clear_expired_previous_token(
    pool: &PgPool,
    session_id: Uuid,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE auth_sessions SET previous_token_hash = NULL, previous_token_valid_until = NULL \
         WHERE id = $1 AND previous_token_valid_until <= $2",
    )
    .bind(session_id)
    .bind(now)
    .execute(pool)
    .await
    .map_err(session_store_error)?;
    Ok(())
}

async fn clear_expired_previous_token_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    session_id: Uuid,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE auth_sessions SET previous_token_hash = NULL, previous_token_valid_until = NULL \
         WHERE id = $1 AND previous_token_valid_until <= $2",
    )
    .bind(session_id)
    .bind(now)
    .execute(&mut **transaction)
    .await
    .map_err(session_store_error)?;
    Ok(())
}

async fn lock_token_hash(
    transaction: &mut Transaction<'_, Postgres>,
    token_hash: TokenHash,
) -> Result<(), AppError> {
    let mut prefix = [0_u8; 8];
    prefix.copy_from_slice(&token_hash.as_bytes()[..8]);
    let lock_key = i64::from_be_bytes(prefix) ^ TOKEN_LOCK_DOMAIN;
    sqlx::query("SELECT pg_advisory_xact_lock($1)")
        .bind(lock_key)
        .execute(&mut **transaction)
        .await
        .map_err(session_store_error)?;
    Ok(())
}

async fn ensure_token_hash_available(
    transaction: &mut Transaction<'_, Postgres>,
    token_hash: TokenHash,
) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM auth_sessions \
         WHERE current_token_hash = $1 OR previous_token_hash = $1)",
    )
    .bind(token_hash.as_bytes().as_slice())
    .fetch_one(&mut **transaction)
    .await
    .map_err(session_store_error)?;
    if exists {
        Err(unavailable())
    } else {
        Ok(())
    }
}

fn session_store_error(_error: sqlx::Error) -> AppError {
    unavailable()
}

fn unavailable() -> AppError {
    AppError::ServiceUnavailable("session_store".to_string())
}
