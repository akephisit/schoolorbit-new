//! Revocable browser credentials are separate from finite authenticated sessions.
use chrono::{DateTime, Utc};
use school_errors::AppError;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::session_crypto::{
    canonical_session_time, remembered_device_token, remembered_session_token, RawSessionToken,
    SessionHmacKey, TokenHash,
};
use crate::session_repository::SessionRevocationReason;

#[derive(Clone, Copy)]
pub(crate) enum DeviceRevocationTarget {
    Session(Uuid),
    All,
}

pub struct NewRememberedDevice {
    pub id: Uuid,
    pub token_hash: TokenHash,
}

#[derive(FromRow)]
struct DeviceRow {
    id: Uuid,
    user_id: Uuid,
    current_token_hash: Vec<u8>,
    pending_token_hash: Option<Vec<u8>>,
    pending_created_at: Option<DateTime<Utc>>,
    pending_session_id: Option<Uuid>,
    device_label: String,
    revoked_at: Option<DateTime<Utc>>,
}

pub struct RestoredSession {
    pub session_token: RawSessionToken,
    pub device_token: RawSessionToken,
    pub user_id: Uuid,
}

#[derive(Debug)]
pub struct RevokedDeviceSessions {
    pub found: bool,
    pub sessions: Vec<(Uuid, Uuid)>,
}

#[derive(FromRow)]
pub struct ManagedSessionRow {
    pub user_id: Uuid,
    pub id: Uuid,
    pub device_label: String,
    pub remember_me: bool,
    pub created_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub idle_expires_at: Option<DateTime<Utc>>,
    pub absolute_expires_at: Option<DateTime<Utc>>,
    pub is_current: bool,
}

pub(crate) async fn lock_login_users(
    tx: &mut Transaction<'_, Postgres>,
    user_id: Option<Uuid>,
    presented: Option<TokenHash>,
) -> Result<Vec<Uuid>, AppError> {
    // Capture the recognized grant before waiting: acknowledgment may retire its old hash.
    // A successful explicit login/logout still retires that exact prior browser grant.
    let prior: Vec<(Uuid,Uuid)> = school_database::DbSession::new(&mut **tx).query_as(
        "auth.remembered_devices.capture_presented",
        "SELECT id,user_id FROM auth_remembered_devices WHERE current_token_hash=$1 OR pending_token_hash=$1",
    ).bind(presented.map(|hash| hash.as_bytes().to_vec())).fetch_all().await.map_err(store_error)?;
    let users: Vec<Uuid> = prior.iter().map(|(_, user)| *user).collect();
    // Account switches may involve two users; lock all users before any grant/session.
    school_database::DbSession::new(&mut **tx)
        .query(
            "auth.remembered_devices.lock_login_users",
            "SELECT id FROM users WHERE id=$1 OR id=ANY($2) ORDER BY id FOR UPDATE",
        )
        .bind(user_id)
        .bind(&users)
        .execute()
        .await
        .map_err(store_error)?;
    Ok(prior.into_iter().map(|(id, _)| id).collect())
}

pub(crate) async fn enroll(
    tx: &mut Transaction<'_, Postgres>,
    device: &NewRememberedDevice,
    user_id: Uuid,
    label: &str,
    now: DateTime<Utc>,
) -> Result<(), AppError> {
    crate::session_repository::lock_token_hash(tx, device.token_hash).await?;
    crate::session_repository::ensure_token_hash_available(tx, device.token_hash).await?;
    school_database::DbSession::new(&mut **tx).query("auth.remembered_devices.enroll",
        "INSERT INTO auth_remembered_devices(id,user_id,current_token_hash,device_label,created_at,last_used_at) VALUES($1,$2,$3,$4,$5,$5)")
        .bind(device.id).bind(user_id).bind(device.token_hash.as_bytes().as_slice()).bind(label).bind(now)
        .execute().await.map_err(store_error)?;
    Ok(())
}

pub(crate) async fn retire_devices(
    tx: &mut Transaction<'_, Postgres>,
    captured: &[Uuid],
    reason: &str,
    now: DateTime<Utc>,
) -> Result<Vec<(Uuid, Uuid)>, AppError> {
    let devices: Vec<Uuid> = school_database::DbSession::new(&mut **tx).scalar(
        "auth.remembered_devices.retire_devices",
        "SELECT id FROM auth_remembered_devices WHERE id=ANY($1) AND revoked_at IS NULL ORDER BY id FOR UPDATE",
    ).bind(captured).fetch_all().await.map_err(store_error)?;
    revoke_grants(tx, &devices, reason, now).await
}

async fn revoke_grants(
    tx: &mut Transaction<'_, Postgres>,
    devices: &[Uuid],
    reason: &str,
    now: DateTime<Utc>,
) -> Result<Vec<(Uuid, Uuid)>, AppError> {
    school_database::DbSession::new(&mut **tx).query("auth.remembered_devices.revoke_grants",
        "UPDATE auth_remembered_devices SET revoked_at=$1,revocation_reason=$2,pending_token_hash=NULL,pending_created_at=NULL,pending_session_id=NULL WHERE id=ANY($3) AND revoked_at IS NULL")
        .bind(now).bind(reason).bind(devices).execute().await.map_err(store_error)?;
    school_database::DbSession::new(&mut **tx).query_as("auth.remembered_devices.revoke_grants",
        "UPDATE auth_sessions SET revoked_at=$1,revocation_reason=$2,pending_token_hash=NULL,pending_created_at=NULL WHERE remembered_device_id=ANY($3) AND revoked_at IS NULL RETURNING user_id,id")
        .bind(now).bind(reason).bind(devices).fetch_all().await.map_err(store_error)
}

/// No incoming live session is ever replaced, even on an idempotent retry.
pub async fn restore(
    pool: &PgPool,
    key: &SessionHmacKey,
    tenant_id: Uuid,
    device: &RawSessionToken,
    incoming_session: Option<TokenHash>,
    now: DateTime<Utc>,
) -> Result<RestoredSession, AppError> {
    let now = canonical_session_time(now)?;
    let hash = device.token_hash();
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "auth.remembered_devices.restore")
            .await
            .map_err(store_error)?;
    // Read identity only to acquire the stable lock order; recheck the credential under lock.
    let users: Vec<Uuid> = school_database::DbSession::new(&mut *tx).scalar(
        "auth.remembered_devices.restore_user",
        "SELECT DISTINCT user_id FROM auth_remembered_devices WHERE current_token_hash=$1 OR pending_token_hash=$1",
    ).bind(hash.as_bytes().as_slice()).fetch_all().await.map_err(store_error)?;
    if users.len() != 1 {
        return Err(authentication_required());
    }
    let user_id = users[0];
    let status: Option<String> = school_database::DbSession::new(&mut *tx)
        .scalar(
            "auth.remembered_devices.restore_user",
            "SELECT status FROM users WHERE id=$1 FOR UPDATE",
        )
        .bind(user_id)
        .fetch_optional()
        .await
        .map_err(store_error)?;
    if status.as_deref() != Some("active") {
        return Err(authentication_required());
    }
    // A direct DB check bypasses cache and performs no cookie/idle/rotation maintenance.
    if let Some(incoming) = incoming_session {
        let live: bool = school_database::DbSession::new(&mut *tx).scalar(
            "auth.remembered_devices.incoming_session",
            "SELECT EXISTS(SELECT 1 FROM auth_sessions s JOIN users u ON u.id=s.user_id WHERE u.status='active' AND s.revoked_at IS NULL AND s.idle_expires_at>$2 AND s.absolute_expires_at>$2 AND (s.current_token_hash=$1 OR s.pending_token_hash=$1 OR (s.previous_token_hash=$1 AND s.previous_token_valid_until>$2)))",
        ).bind(incoming.as_bytes().as_slice()).bind(now).fetch_one().await.map_err(store_error)?;
        if live {
            return Err(AppError::Conflict("session_changed".into()));
        }
    }
    let mut rows: Vec<DeviceRow> = school_database::DbSession::new(&mut *tx).query_as(
        "auth.remembered_devices.restore_grant",
        "SELECT id,user_id,current_token_hash,pending_token_hash,pending_created_at,pending_session_id,device_label,revoked_at FROM auth_remembered_devices WHERE user_id=$2 AND (current_token_hash=$1 OR pending_token_hash=$1) LIMIT 2 FOR UPDATE",
    ).bind(hash.as_bytes().as_slice()).bind(user_id).fetch_all().await.map_err(store_error)?;
    if rows.len() != 1 {
        return Err(authentication_required());
    }
    let mut row = rows.pop().ok_or_else(authentication_required)?;
    if row.revoked_at.is_some() {
        return Err(authentication_required());
    }
    if row
        .pending_token_hash
        .as_deref()
        .is_some_and(|value| bool::from(value.ct_eq(hash.as_bytes())))
    {
        // Acknowledgment retires the old credential. Stale current requests cannot clear cookies.
        school_database::DbSession::new(&mut *tx).query("auth.remembered_devices.acknowledge",
            "UPDATE auth_remembered_devices SET current_token_hash=$1,pending_token_hash=NULL,pending_created_at=NULL,pending_session_id=NULL WHERE id=$2")
            .bind(hash.as_bytes().as_slice()).bind(row.id).execute().await.map_err(store_error)?;
        row.current_token_hash = hash.as_bytes().to_vec();
        row.pending_token_hash = None;
        row.pending_created_at = None;
        row.pending_session_id = None;
    } else if !bool::from(row.current_token_hash.as_slice().ct_eq(hash.as_bytes())) {
        return Err(authentication_required());
    }
    let mut replay = None;
    if let (Some(staged_hash), Some(at), Some(session_id)) = (
        row.pending_token_hash.as_deref(),
        row.pending_created_at,
        row.pending_session_id,
    ) {
        let next = remembered_device_token(key, tenant_id, row.id, device, at);
        if !bool::from(staged_hash.ct_eq(next.token_hash().as_bytes())) {
            return Err(unavailable());
        }
        let raw = remembered_session_token(key, tenant_id, row.id, session_id, device, at);
        // Retry only while the reproducible credential is still accepted by this finite child.
        let valid: bool=school_database::DbSession::new(&mut *tx).scalar("auth.remembered_devices.replay",
            "SELECT EXISTS(SELECT 1 FROM auth_sessions WHERE id=$1 AND user_id=$2 AND remembered_device_id=$3 AND revoked_at IS NULL AND idle_expires_at>$4 AND absolute_expires_at>$4 AND (current_token_hash=$5 OR pending_token_hash=$5 OR (previous_token_hash=$5 AND previous_token_valid_until>$4)))")
            .bind(session_id).bind(user_id).bind(row.id).bind(now).bind(raw.token_hash().as_bytes().as_slice())
            .fetch_one().await.map_err(store_error)?;
        if valid {
            replay = Some((raw, next));
        }
    }
    let (session_token, device_token) = if let Some(pair) = replay {
        pair
    } else {
        let session_id = Uuid::new_v4();
        let next = remembered_device_token(key, tenant_id, row.id, device, now);
        let raw = remembered_session_token(key, tenant_id, row.id, session_id, device, now);
        // Discard an obsolete pending generation before collision checks.
        school_database::DbSession::new(&mut *tx).query("auth.remembered_devices.replace_generation",
            "UPDATE auth_remembered_devices SET pending_token_hash=NULL,pending_created_at=NULL,pending_session_id=NULL WHERE id=$1")
            .bind(row.id).execute().await.map_err(store_error)?;
        let mut hashes = [next.token_hash(), raw.token_hash()];
        hashes.sort_by_key(|value| *value.as_bytes());
        for allocated in hashes {
            crate::session_repository::lock_token_hash(&mut tx, allocated).await?;
            crate::session_repository::ensure_token_hash_available(&mut tx, allocated).await?;
        }
        let lifetime = crate::session_policy::SessionLifetime::remembered();
        school_database::DbSession::new(&mut *tx).query("auth.remembered_devices.create_session",
            "INSERT INTO auth_sessions(id,user_id,current_token_hash,remember_me,device_label,created_at,last_seen_at,idle_expires_at,absolute_expires_at,rotated_at,remembered_device_id) VALUES($1,$2,$3,true,$4,$5,$5,$7,$8,$5,$6)")
            .bind(session_id).bind(user_id).bind(raw.token_hash().as_bytes().as_slice()).bind(&row.device_label).bind(now).bind(row.id)
            .bind(now+lifetime.idle).bind(now+lifetime.absolute)
            .execute().await.map_err(store_error)?;
        school_database::DbSession::new(&mut *tx).query("auth.remembered_devices.stage_generation",
            "UPDATE auth_remembered_devices SET pending_token_hash=$1,pending_created_at=$2,pending_session_id=$3 WHERE id=$4")
            .bind(next.token_hash().as_bytes().as_slice()).bind(now).bind(session_id).bind(row.id)
            .execute().await.map_err(store_error)?;
        (raw, next)
    };
    school_database::DbSession::new(&mut *tx)
        .query(
            "auth.remembered_devices.restore_used",
            "UPDATE auth_remembered_devices SET last_used_at=GREATEST(last_used_at,$1) WHERE id=$2",
        )
        .bind(now)
        .bind(row.id)
        .execute()
        .await
        .map_err(store_error)?;
    tx.commit().await.map_err(store_error)?;
    Ok(RestoredSession {
        session_token,
        device_token,
        user_id: row.user_id,
    })
}

pub async fn list(
    pool: &PgPool,
    user_id: Uuid,
    current_session_id: Uuid,
    now: DateTime<Utc>,
) -> Result<Vec<ManagedSessionRow>, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "auth.remembered_devices.list")
            .await
            .map_err(store_error)?;
    school_database::DbSession::new(&mut *connection).query_as("auth.remembered_devices.list",
        "SELECT s.id,s.user_id,s.device_label,s.remember_me,s.created_at,s.last_seen_at,s.idle_expires_at,s.absolute_expires_at,(s.id=$2) AS is_current FROM auth_sessions s JOIN users u ON u.id=s.user_id WHERE s.user_id=$1 AND s.remembered_device_id IS NULL AND s.revoked_at IS NULL AND s.idle_expires_at>$3 AND s.absolute_expires_at>$3 AND u.status='active' UNION ALL SELECT g.id,g.user_id,g.device_label,true AS remember_me,g.created_at,g.last_used_at AS last_seen_at,NULL AS idle_expires_at,NULL AS absolute_expires_at,EXISTS(SELECT 1 FROM auth_sessions c WHERE c.id=$2 AND c.user_id=g.user_id AND c.remembered_device_id=g.id AND c.revoked_at IS NULL AND c.idle_expires_at>$3 AND c.absolute_expires_at>$3) AS is_current FROM auth_remembered_devices g JOIN users u ON u.id=g.user_id WHERE g.user_id=$1 AND g.revoked_at IS NULL AND u.status='active' ORDER BY last_seen_at DESC,id")
        .bind(user_id).bind(current_session_id).bind(now).fetch_all().await.map_err(store_error)
}

pub(crate) async fn revoke(
    pool: &PgPool,
    user_id: Uuid,
    target: DeviceRevocationTarget,
    reason: SessionRevocationReason,
    now: DateTime<Utc>,
) -> Result<RevokedDeviceSessions, AppError> {
    let mut tx = school_tenancy::TenantPool::begin_observed(pool, "auth.remembered_devices.revoke")
        .await
        .map_err(store_error)?;
    school_database::DbSession::new(&mut *tx)
        .query(
            "auth.remembered_devices.revoke_user",
            "SELECT id FROM users WHERE id=$1 FOR UPDATE",
        )
        .bind(user_id)
        .execute()
        .await
        .map_err(store_error)?;
    let selected = match target {
        DeviceRevocationTarget::Session(id) => Some(id),
        DeviceRevocationTarget::All => None,
    };
    let devices:Vec<Uuid>=school_database::DbSession::new(&mut *tx).scalar("auth.remembered_devices.revoke_devices",
        "SELECT id FROM auth_remembered_devices WHERE user_id=$1 AND revoked_at IS NULL AND ($2::uuid IS NULL OR id=$2 OR id=(SELECT remembered_device_id FROM auth_sessions WHERE id=$2 AND user_id=$1)) ORDER BY id FOR UPDATE")
        .bind(user_id).bind(selected).fetch_all().await.map_err(store_error)?;
    let mut sessions = revoke_grants(&mut tx, &devices, reason.as_str(), now).await?;
    let ordinary:Vec<(Uuid,Uuid)>=school_database::DbSession::new(&mut *tx).query_as("auth.remembered_devices.revoke_sessions",
        "UPDATE auth_sessions SET revoked_at=$1,revocation_reason=$2,pending_token_hash=NULL,pending_created_at=NULL WHERE user_id=$3 AND revoked_at IS NULL AND ($4::uuid IS NULL OR id=$4) RETURNING user_id,id")
        .bind(now).bind(reason.as_str()).bind(user_id).bind(selected).fetch_all().await.map_err(store_error)?;
    sessions.extend(ordinary);
    let found = !devices.is_empty() || !sessions.is_empty();
    tx.commit().await.map_err(store_error)?;
    Ok(RevokedDeviceSessions { found, sessions })
}

pub async fn revoke_presented(
    pool: &PgPool,
    hash: TokenHash,
    now: DateTime<Utc>,
) -> Result<RevokedDeviceSessions, AppError> {
    let mut tx = school_tenancy::TenantPool::begin_observed(
        pool,
        "auth.remembered_devices.logout_presented",
    )
    .await
    .map_err(store_error)?;
    let captured = lock_login_users(&mut tx, None, Some(hash)).await?;
    let sessions = retire_devices(&mut tx, &captured, "logout", now).await?;
    tx.commit().await.map_err(store_error)?;
    Ok(RevokedDeviceSessions {
        found: !captured.is_empty(),
        sessions,
    })
}

fn store_error(_: sqlx::Error) -> AppError {
    unavailable()
}
fn unavailable() -> AppError {
    AppError::ServiceUnavailable("session_store".into())
}
fn authentication_required() -> AppError {
    AppError::AuthError("กรุณาเข้าสู่ระบบอีกครั้ง".into())
}
