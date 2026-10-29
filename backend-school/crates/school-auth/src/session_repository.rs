use chrono::{DateTime, Utc};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use school_errors::AppError;

use super::{
    services::SessionLoginUser,
    session_crypto::{canonical_session_time, pending_session_token, RawSessionToken, SessionHmacKey, ThrottleBucketHash, TokenHash},
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
    Pending,
}

pub struct ActivityTokenDerivation<'a> {
    pub key: &'a SessionHmacKey,
    pub tenant_id: Uuid,
    pub raw: &'a RawSessionToken,
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
    pub user_type: String,
    pub presented_as: PresentedTokenKind,
    pub remember_me: bool,
    pub rotated_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub idle_expires_at: DateTime<Utc>,
    pub absolute_expires_at: DateTime<Utc>,
    pub replacement: Option<RawSessionToken>,
    pub has_pending: bool,
    pub credential_changed: bool,
}

#[derive(Clone, FromRow)]
pub struct SessionValidity {
    pub idle_expires_at: DateTime<Utc>,
    pub absolute_expires_at: DateTime<Utc>,
}

impl SessionValidity {
    pub fn is_valid_at(&self, now: DateTime<Utc>) -> bool {
        now < self.idle_expires_at && now < self.absolute_expires_at
    }
}

pub struct PasswordChangeSnapshot {
    pub password_hash: String,
}

pub struct LockedPasswordChange {
    pub password_hash: String,
    pub remember_me: bool,
    pub absolute_expires_at: DateTime<Utc>,
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
    pending_token_hash: Option<Vec<u8>>,
    pending_created_at: Option<DateTime<Utc>>,
    remember_me: bool,
    rotated_at: DateTime<Utc>,
    last_seen_at: DateTime<Utc>,
    idle_expires_at: DateTime<Utc>,
    absolute_expires_at: DateTime<Utc>,
    revoked_at: Option<DateTime<Utc>>,
    user_type: String,
    user_status: String,
}

const AUTHENTICATION_COLUMNS: &str = r#"
    SELECT s.id, s.user_id, s.current_token_hash, s.previous_token_hash,
           s.previous_token_valid_until, s.pending_token_hash, s.pending_created_at, s.remember_me, s.rotated_at, s.last_seen_at,
           s.idle_expires_at, s.absolute_expires_at, s.revoked_at,
           u.user_type, u.status AS user_status
    FROM auth_sessions s
    JOIN users u ON u.id = s.user_id
    WHERE s.current_token_hash = $1 OR s.previous_token_hash = $1 OR s.pending_token_hash = $1
    LIMIT 2
"#;

#[derive(Debug, Eq, PartialEq)]
pub enum LoginSessionCreation {
    Created,
    StaleIdentity,
}

pub async fn create_login_session(
    pool: &PgPool,
    session: &NewSession,
    identifier_hash: ThrottleBucketHash,
    verified_user: &SessionLoginUser,
) -> Result<LoginSessionCreation, AppError> {
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        &pool,
        "auth.session_repository.create_login_session",
    )
    .await
    .map_err(session_store_error)?;
    // Serialize final credential validation with password changes and account updates.
    // User locks precede session/token locks; bcrypt has already completed outside this tx.
    let locked: Option<(String, String, String, String)> =
        school_database::DbSession::new(&mut *transaction)
            .query_as(
                "auth.src.session_repository.create_login_session",
                "SELECT username,password_hash,status,user_type FROM users WHERE id=$1 FOR UPDATE",
            )
            .bind(verified_user.id)
            .fetch_optional()
            .await
            .map_err(session_store_error)?;
    let valid = locked.is_some_and(|(username, password_hash, status, user_type)| {
        session.user_id == verified_user.id
            && username == verified_user.username
            && status == "active"
            && verified_user.status == "active"
            && user_type == verified_user.user_type
            && bool::from(
                password_hash
                    .as_bytes()
                    .ct_eq(verified_user.password_hash.as_bytes()),
            )
    });
    if !valid {
        transaction.rollback().await.map_err(session_store_error)?;
        return Ok(LoginSessionCreation::StaleIdentity);
    }
    lock_token_hash(&mut transaction, session.current_token_hash).await?;
    ensure_token_hash_available(&mut transaction, session.current_token_hash).await?;

    school_database::DbSession::new(&mut *transaction)
        .query(
            "auth.src.session_repository.create_login_session",
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
        .execute()
        .await
        .map_err(session_store_error)?;

    school_database::DbSession::new(&mut *transaction).query("auth.src.session_repository.create_login_session",
        "DELETE FROM auth_login_throttles WHERE bucket_kind = 'identifier' AND bucket_hash = $1",
    )
    .bind(identifier_hash.as_bytes().as_slice())
    .execute()
    .await
    .map_err(session_store_error)?;

    transaction.commit().await.map_err(session_store_error)?;
    Ok(LoginSessionCreation::Created)
}

/// Validation has no persistent side effects, including expired-credential rejection.
pub async fn validate_session(
    pool: &PgPool,
    presented_hash: TokenHash,
    now: DateTime<Utc>,
) -> Result<Option<MaintainedSession>, AppError> {
    let Some(row) = load_authentication_row(pool, presented_hash).await? else {
        super::audit::session_rejected("unknown_credential");
        return Ok(None);
    };
    let kind = presented_kind(&row, presented_hash).ok_or_else(unavailable)?;
    if !accepted(&row, kind, now) {
        return Ok(None);
    }
    Ok(Some(maintained(row, kind, None, false)))
}

/// Called only after origin, CSRF and the expected session fence pass. None acknowledges
/// pending credentials without touching idle; Some is the sole human activity boundary.
pub async fn maintain_session(
    pool: &PgPool,
    user_id: Uuid,
    expected_session_id: Uuid,
    presented_hash: TokenHash,
    now: DateTime<Utc>,
    activity: Option<ActivityTokenDerivation<'_>>,
) -> Result<Option<MaintainedSession>, AppError> {
    let now = canonical_session_time(now)?;
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        pool, "auth.session_repository.maintain_session",
    ).await.map_err(session_store_error)?;
    // Account/password changes take this lock first too; session then token allocation follow.
    school_database::DbSession::new(&mut *transaction)
        .query("auth.src.session_repository.maintain_session.user_lock",
            "SELECT id FROM users WHERE id = $1 FOR UPDATE")
        .bind(user_id).execute().await.map_err(session_store_error)?;
    let Some(mut row) = load_authentication_row_for_update(&mut transaction, presented_hash).await? else {
        transaction.rollback().await.map_err(session_store_error)?;
        return Ok(None);
    };
    if row.id != expected_session_id || row.user_id != user_id {
        return Err(AppError::Conflict("session_changed".into()));
    }
    let kind = presented_kind(&row, presented_hash).ok_or_else(unavailable)?;
    if !accepted(&row, kind, now) {
        transaction.rollback().await.map_err(session_store_error)?;
        return Ok(None);
    }
    let promoted = kind == PresentedTokenKind::Pending;
    if promoted {
        row.previous_token_hash = Some(row.current_token_hash.clone());
        row.previous_token_valid_until = Some(now + PREVIOUS_TOKEN_GRACE);
        row.current_token_hash = row.pending_token_hash.take().ok_or_else(unavailable)?;
        row.pending_created_at = None;
        row.rotated_at = now;
    }
    let touch = activity.is_some() && row.last_seen_at <= now - TOUCH_INTERVAL;
    let mut replacement = None;
    let mut staged = false;
    if let Some(proof) = activity {
        // Previous credentials may report activity during the grace window, but cannot
        // derive the current generation or allocate another pending credential.
        if kind == PresentedTokenKind::Current && row.rotated_at <= now - ROTATION_INTERVAL {
            let created = row.pending_created_at.unwrap_or(now);
            let token = pending_session_token(proof.key, proof.tenant_id, row.id, proof.raw, created);
            let hash = token.token_hash();
            if let Some(pending) = row.pending_token_hash.as_deref() {
                if !bool::from(pending.ct_eq(hash.as_bytes().as_slice())) {
                    return Err(unavailable());
                }
            } else {
                lock_token_hash(&mut transaction, hash).await?;
                ensure_token_hash_available(&mut transaction, hash).await?;
                row.pending_token_hash = Some(hash.as_bytes().to_vec());
                row.pending_created_at = Some(created);
                staged = true;
            }
            replacement = Some(token);
        }
    }
    if touch {
        let lifetime = if row.remember_me { SessionLifetime::remembered() } else { SessionLifetime::normal() };
        row.last_seen_at = now;
        row.idle_expires_at = (now + lifetime.idle).min(row.absolute_expires_at);
    }
    if promoted || staged || touch {
        school_database::DbSession::new(&mut *transaction)
            .query("auth.src.session_repository.maintain_session", r#"
                UPDATE auth_sessions SET current_token_hash=$1, previous_token_hash=$2,
                    previous_token_valid_until=$3, pending_token_hash=$4, pending_created_at=$5,
                    rotated_at=$6, last_seen_at=$7, idle_expires_at=$8 WHERE id=$9
            "#)
            .bind(&row.current_token_hash).bind(&row.previous_token_hash)
            .bind(row.previous_token_valid_until).bind(&row.pending_token_hash)
            .bind(row.pending_created_at).bind(row.rotated_at).bind(row.last_seen_at)
            .bind(row.idle_expires_at).bind(row.id)
            .execute().await.map_err(session_store_error)?;
    }
    transaction.commit().await.map_err(session_store_error)?;
    if promoted { super::audit::session_transition(row.id, "credential_promoted"); }
    if staged { super::audit::session_transition(row.id, "credential_staged"); }
    Ok(Some(maintained(row, if promoted { PresentedTokenKind::Current } else { kind }, replacement, promoted || staged || touch)))
}

fn accepted(row: &AuthenticationRow, kind: PresentedTokenKind, now: DateTime<Utc>) -> bool {
    let reason = if row.revoked_at.is_some() { Some("revoked") }
        else if row.user_status != "active" { Some("inactive_user") }
        else if row.absolute_expires_at <= now { Some("absolute_expired") }
        else if row.idle_expires_at <= now { Some("idle_expired") }
        else if kind == PresentedTokenKind::Previous && !previous_token_is_valid(row, now) { Some("previous_expired") }
        else { None };
    if let Some(reason) = reason {
        super::audit::session_rejected(reason);
        false
    } else { true }
}

#[cfg(test)]
pub async fn revalidate_session(
    pool: &PgPool,
    session_id: Uuid,
    user_id: Uuid,
    now: DateTime<Utc>,
) -> Result<bool, AppError> {
    Ok(load_session_validity(pool, session_id, user_id, now)
        .await?
        .is_some())
}

pub async fn load_session_validity(
    pool: &PgPool,
    session_id: Uuid,
    user_id: Uuid,
    now: DateTime<Utc>,
) -> Result<Option<SessionValidity>, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "auth.load_session_validity")
            .await
            .map_err(session_store_error)?;
    load_session_validity_on_connection(&mut connection, session_id, user_id, now).await
}

pub async fn load_session_validity_on_connection(
    connection: &mut sqlx::PgConnection,
    session_id: Uuid,
    user_id: Uuid,
    now: DateTime<Utc>,
) -> Result<Option<SessionValidity>, AppError> {
    school_database::DbSession::new(&mut *connection)
        .query_as(
            "auth.src.session_repository.load_session_validity_on_connection",
            r#"
            SELECT s.idle_expires_at, s.absolute_expires_at
            FROM auth_sessions s
            JOIN users u ON u.id = s.user_id
            WHERE s.id = $1
              AND s.user_id = $2
              AND s.revoked_at IS NULL
              AND s.idle_expires_at > $3
              AND s.absolute_expires_at > $3
              AND u.status = 'active'
        "#,
        )
        .bind(session_id)
        .bind(user_id)
        .bind(now)
        .fetch_optional()
        .await
        .map_err(session_store_error)
}

pub async fn load_password_change_snapshot(
    pool: &PgPool,
    user_id: Uuid,
    session_id: Uuid,
    now: DateTime<Utc>,
) -> Result<PasswordChangeSnapshot, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "auth.load_password_change_snapshot",
    )
    .await
    .map_err(session_store_error)?;
    load_password_change_snapshot_on_connection(&mut connection, user_id, session_id, now).await
}

pub async fn load_password_change_snapshot_on_connection(
    connection: &mut sqlx::PgConnection,
    user_id: Uuid,
    session_id: Uuid,
    now: DateTime<Utc>,
) -> Result<PasswordChangeSnapshot, AppError> {
    school_database::DbSession::new(&mut *connection)
        .query_as::<(String,)>(
            "auth.src.session_repository.load_password_change_snapshot_on_connection",
            r#"
        SELECT u.password_hash
        FROM users u
        JOIN auth_sessions s ON s.user_id = u.id
        WHERE u.id = $1
          AND s.id = $2
          AND u.status = 'active'
          AND s.revoked_at IS NULL
          AND s.idle_expires_at > $3
          AND s.absolute_expires_at > $3
        "#,
        )
        .bind(user_id)
        .bind(session_id)
        .bind(now)
        .fetch_optional()
        .await
        .map_err(session_store_error)?
        .map(|(password_hash,)| PasswordChangeSnapshot { password_hash })
        .ok_or_else(authentication_required)
}

pub async fn lock_password_change<'a>(
    transaction: &mut Transaction<'a, Postgres>,
    user_id: Uuid,
    session_id: Uuid,
    now: DateTime<Utc>,
) -> Result<LockedPasswordChange, AppError> {
    school_database::DbSession::new(&mut **transaction)
        .query_as::<(String, bool, DateTime<Utc>)>(
            "auth.src.session_repository.lock_password_change",
            r#"
        SELECT u.password_hash, s.remember_me, s.absolute_expires_at
        FROM users u
        JOIN auth_sessions s ON s.user_id = u.id
        WHERE u.id = $1
          AND s.id = $2
          AND u.status = 'active'
          AND s.revoked_at IS NULL
          AND s.idle_expires_at > $3
          AND s.absolute_expires_at > $3
        FOR UPDATE OF u, s
        "#,
        )
        .bind(user_id)
        .bind(session_id)
        .bind(now)
        .fetch_optional()
        .await
        .map_err(session_store_error)?
        .map(
            |(password_hash, remember_me, absolute_expires_at)| LockedPasswordChange {
                password_hash,
                remember_me,
                absolute_expires_at,
            },
        )
        .ok_or_else(authentication_required)
}

pub async fn apply_password_change<'a>(
    transaction: &mut Transaction<'a, Postgres>,
    user_id: Uuid,
    session_id: Uuid,
    new_password_hash: &str,
    new_token_hash: TokenHash,
    now: DateTime<Utc>,
) -> Result<Vec<Uuid>, AppError> {
    lock_token_hash(transaction, new_token_hash).await?;
    ensure_token_hash_available(transaction, new_token_hash).await?;

    let updated_user = school_database::DbSession::new(&mut **transaction)
        .query(
            "auth.src.session_repository.apply_password_change",
            "UPDATE users SET password_hash = $1, updated_at = $2 WHERE id = $3",
        )
        .bind(new_password_hash)
        .bind(now)
        .bind(user_id)
        .execute()
        .await
        .map_err(session_store_error)?
        .rows_affected();
    if updated_user != 1 {
        return Err(unavailable());
    }

    let mut revoked_session_ids: Vec<Uuid> = school_database::DbSession::new(&mut **transaction)
        .scalar(
            "auth.src.session_repository.apply_password_change",
            r#"
        UPDATE auth_sessions
        SET revoked_at = $1, revocation_reason = $2, pending_token_hash = NULL, pending_created_at = NULL
        WHERE user_id = $3
          AND id <> $4
          AND revoked_at IS NULL
          AND idle_expires_at > $1
          AND absolute_expires_at > $1
        RETURNING id
        "#,
        )
        .bind(now)
        .bind(SessionRevocationReason::PasswordChanged.as_str())
        .bind(user_id)
        .bind(session_id)
        .fetch_all()
        .await
        .map_err(session_store_error)?;

    let updated_session = school_database::DbSession::new(&mut **transaction)
        .query(
            "auth.src.session_repository.apply_password_change",
            r#"
        UPDATE auth_sessions
        SET previous_token_hash = current_token_hash,
            previous_token_valid_until = $1 + interval '60 seconds',
            current_token_hash = $2,
            rotated_at = $1,
            pending_token_hash = NULL, pending_created_at = NULL,
            last_seen_at = $1,
            idle_expires_at = LEAST(
                $1 + CASE WHEN remember_me THEN interval '7 days' ELSE interval '2 hours' END,
                absolute_expires_at
            )
        WHERE id = $3
          AND user_id = $4
          AND revoked_at IS NULL
          AND idle_expires_at > $1
          AND absolute_expires_at > $1
        "#,
        )
        .bind(now)
        .bind(new_token_hash.as_bytes().as_slice())
        .bind(session_id)
        .bind(user_id)
        .execute()
        .await
        .map_err(session_store_error)?
        .rows_affected();
    if updated_session != 1 {
        return Err(authentication_required());
    }

    revoked_session_ids.sort_unstable();
    Ok(revoked_session_ids)
}

pub async fn list_user_sessions(
    pool: &PgPool,
    user_id: Uuid,
    now: DateTime<Utc>,
) -> Result<Vec<SessionRow>, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "auth.list_user_sessions")
            .await
            .map_err(session_store_error)?;
    list_user_sessions_on_connection(&mut connection, user_id, now).await
}

pub async fn list_user_sessions_on_connection(
    connection: &mut sqlx::PgConnection,
    user_id: Uuid,
    now: DateTime<Utc>,
) -> Result<Vec<SessionRow>, AppError> {
    school_database::DbSession::new(&mut *connection)
        .query_as(
            "auth.src.session_repository.list_user_sessions_on_connection",
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
        .fetch_all()
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
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "auth.revoke_sessions")
            .await
            .map_err(session_store_error)?;
    revoke_sessions_on_connection(&mut connection, user_id, target, reason, now).await
}

pub async fn revoke_sessions_on_connection(
    connection: &mut sqlx::PgConnection,
    user_id: Uuid,
    target: SessionRevocationTarget,
    reason: SessionRevocationReason,
    now: DateTime<Utc>,
) -> Result<Vec<Uuid>, AppError> {
    let result = match target {
        SessionRevocationTarget::Session(session_id) => {
            school_database::DbSession::new(&mut *connection)
                .scalar(
                    "auth.src.session_repository.revoke_sessions_on_connection",
                    r#"
                UPDATE auth_sessions
                SET revoked_at = $1, revocation_reason = $2, pending_token_hash = NULL, pending_created_at = NULL
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
                .fetch_all()
                .await
        }
        SessionRevocationTarget::User { except_session_id } => {
            school_database::DbSession::new(&mut *connection)
                .scalar(
                    "auth.src.session_repository.revoke_sessions_on_connection",
                    r#"
                UPDATE auth_sessions
                SET revoked_at = $1, revocation_reason = $2, pending_token_hash = NULL, pending_created_at = NULL
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
                .fetch_all()
                .await
        }
    };

    result.map_err(session_store_error)
}

pub async fn cleanup_auth_state(
    pool: &PgPool,
    now: DateTime<Utc>,
) -> Result<CleanupResult, AppError> {
    let mut transaction = school_tenancy::TenantPool::begin_observed(
        &pool,
        "auth.session_repository.cleanup_auth_state",
    )
    .await
    .map_err(session_store_error)?;
    let previous_tokens_cleared = school_database::DbSession::new(&mut *transaction)
        .query(
            "auth.src.session_repository.cleanup_auth_state",
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
        .execute()
        .await
        .map_err(session_store_error)?
        .rows_affected();

    let sessions_deleted = school_database::DbSession::new(&mut *transaction)
        .query(
            "auth.src.session_repository.cleanup_auth_state",
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
        .execute()
        .await
        .map_err(session_store_error)?
        .rows_affected();

    let throttles_deleted = school_database::DbSession::new(&mut *transaction)
        .query(
            "auth.src.session_repository.cleanup_auth_state",
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
        .execute()
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
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "auth.load_authentication_row")
            .await
            .map_err(session_store_error)?;
    load_authentication_row_on_connection(&mut connection, presented_hash).await
}

async fn load_authentication_row_on_connection(
    connection: &mut sqlx::PgConnection,
    presented_hash: TokenHash,
) -> Result<Option<AuthenticationRow>, AppError> {
    let rows = school_database::DbSession::new(&mut *connection)
        .query_as::<AuthenticationRow>(
            "auth.src.session_repository.load_authentication_row_on_connection",
            AUTHENTICATION_COLUMNS,
        )
        .bind(presented_hash.as_bytes().as_slice())
        .fetch_all()
        .await
        .map_err(session_store_error)?;
    exactly_one_or_none(rows)
}

async fn load_authentication_row_for_update(
    transaction: &mut Transaction<'_, Postgres>,
    presented_hash: TokenHash,
) -> Result<Option<AuthenticationRow>, AppError> {
    let query = format!("{AUTHENTICATION_COLUMNS} FOR UPDATE OF s");
    let rows = school_database::DbSession::new(&mut **transaction)
        .query_as::<AuthenticationRow>(
            "auth.src.session_repository.load_authentication_row_for_update",
            sqlx::AssertSqlSafe(query),
        )
        .bind(presented_hash.as_bytes().as_slice())
        .fetch_all()
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
    } else if row.pending_token_hash.as_deref()
        .is_some_and(|hash| bool::from(hash.ct_eq(presented_hash.as_bytes().as_slice()))) {
        Some(PresentedTokenKind::Pending)
    } else {
        None
    }
}

fn previous_token_is_valid(row: &AuthenticationRow, now: DateTime<Utc>) -> bool {
    row.previous_token_hash.is_some()
        && row
            .previous_token_valid_until
            .is_some_and(|valid_until| valid_until > now)
}

fn maintained(
    row: AuthenticationRow,
    presented_as: PresentedTokenKind,
    replacement: Option<RawSessionToken>,
    credential_changed: bool,
) -> MaintainedSession {
    MaintainedSession {
        session_id: row.id,
        user_id: row.user_id,
        user_type: row.user_type,
        presented_as,
        remember_me: row.remember_me,
        rotated_at: row.rotated_at,
        last_seen_at: row.last_seen_at,
        idle_expires_at: row.idle_expires_at,
        absolute_expires_at: row.absolute_expires_at,
        replacement,
        has_pending: row.pending_token_hash.is_some(),
        credential_changed,
    }
}

async fn lock_token_hash(
    transaction: &mut Transaction<'_, Postgres>,
    token_hash: TokenHash,
) -> Result<(), AppError> {
    let mut prefix = [0_u8; 8];
    prefix.copy_from_slice(&token_hash.as_bytes()[..8]);
    let lock_key = i64::from_be_bytes(prefix) ^ TOKEN_LOCK_DOMAIN;
    school_database::DbSession::new(&mut **transaction)
        .query(
            "auth.src.session_repository.lock_token_hash",
            "SELECT pg_advisory_xact_lock($1)",
        )
        .bind(lock_key)
        .execute()
        .await
        .map_err(session_store_error)?;
    Ok(())
}

async fn ensure_token_hash_available(
    transaction: &mut Transaction<'_, Postgres>,
    token_hash: TokenHash,
) -> Result<(), AppError> {
    let exists: bool = school_database::DbSession::new(&mut **transaction)
        .scalar(
            "auth.src.session_repository.ensure_token_hash_available",
            "SELECT EXISTS (SELECT 1 FROM auth_sessions \
         WHERE current_token_hash = $1 OR previous_token_hash = $1 OR pending_token_hash = $1)",
        )
        .bind(token_hash.as_bytes().as_slice())
        .fetch_one()
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

fn authentication_required() -> AppError {
    AppError::AuthError("กรุณาเข้าสู่ระบบอีกครั้ง".to_string())
}
