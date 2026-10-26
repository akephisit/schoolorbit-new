//! Tenant/user/session-owned Web Push bindings. Registration secrets never leave this owner.
use crate::models::{
    ListPushDevicesResponse, PushConfigResponse, PushDevice, RegisterPushDeviceRequest,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use school_errors::AppError;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub const MAX_DEVICES: usize = 20;
pub const DEVICE_EXPIRY_DAYS: i64 = 90;
pub(crate) struct VapidConfig {
    pub public_key: String,
    pub private_key: String,
    pub subject: String,
    pub key_id: String,
}
fn decode(value: &str, length: usize) -> bool {
    value.len() <= 128
        && URL_SAFE_NO_PAD
            .decode(value)
            .is_ok_and(|bytes| bytes.len() == length && (length != 65 || bytes[0] == 4))
}
pub(crate) fn vapid_config() -> Option<VapidConfig> {
    let public_key = std::env::var("VAPID_PUBLIC_KEY").ok()?;
    let private_key = std::env::var("VAPID_PRIVATE_KEY").ok()?;
    let subject = std::env::var("VAPID_SUBJECT").ok()?;
    validate_vapid(public_key, private_key, subject)
}
fn validate_vapid(public_key: String, private_key: String, subject: String) -> Option<VapidConfig> {
    if !decode(&public_key, 65) || !decode(&private_key, 32) || subject.len() > 512 {
        return None;
    }
    let contact = url::Url::parse(&subject).ok()?;
    if !(contact.scheme() == "https" && contact.host_str().is_some()
        || contact.scheme() == "mailto" && !contact.path().is_empty())
    {
        return None;
    }
    let public_bytes = URL_SAFE_NO_PAD.decode(&public_key).ok()?;
    let signer = web_push::VapidSignatureBuilder::from_base64_no_sub(&private_key).ok()?;
    if signer.get_public_key() != public_bytes {
        return None;
    }
    let key_id = URL_SAFE_NO_PAD.encode(Sha256::digest(public_bytes));
    Some(VapidConfig {
        public_key,
        private_key,
        subject,
        key_id,
    })
}
pub fn push_config() -> PushConfigResponse {
    let config = vapid_config();
    PushConfigResponse {
        available: config.is_some(),
        public_key: config.as_ref().map(|c| c.public_key.clone()),
        key_id: config.map(|c| c.key_id),
        max_devices: MAX_DEVICES,
        device_expiry_days: DEVICE_EXPIRY_DAYS,
    }
}
pub(crate) fn valid_endpoint(value: &str) -> bool {
    if value.len() > 2048 {
        return false;
    }
    let Ok(endpoint) = url::Url::parse(value) else {
        return false;
    };
    if endpoint.scheme() != "https"
        || !endpoint.username().is_empty()
        || endpoint.password().is_some()
        || endpoint.fragment().is_some()
        || endpoint.port_or_known_default() != Some(443)
    {
        return false;
    }
    let Some(host) = endpoint.host_str() else {
        return false;
    };
    // Exact provider-controlled suffixes prevent arbitrary network destinations/SSRF.
    host == "fcm.googleapis.com"
        || host == "updates.push.services.mozilla.com"
        || host.ends_with(".push.services.mozilla.com")
        || host == "web.push.apple.com"
        || host.ends_with(".push.apple.com")
}
fn validate(request: &RegisterPushDeviceRequest, key_id: &str) -> Result<(), AppError> {
    if request.installation_id.is_nil()
        || request.label.trim().is_empty()
        || request.label.chars().count() > 80
        || request.label.chars().any(char::is_control)
        || !valid_endpoint(&request.endpoint)
        || !decode(&request.p256dh, 65)
        || !decode(&request.auth, 16)
    {
        return Err(AppError::BadRequest("push_device_invalid".into()));
    }
    if request.key_id != key_id {
        return Err(AppError::Conflict("push_key_changed".into()));
    }
    Ok(())
}
pub async fn list(pool: &PgPool, user: Uuid) -> Result<ListPushDevicesResponse, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "notifications.devices.list")
            .await?;
    let items=school_database::DbSession::new(&mut *connection).query_as("notifications.devices.list",r#"SELECT p.id,p.installation_id,p.label,p.version,p.binding_token,p.key_id,p.last_confirmed_at,p.expires_at
        FROM push_subscriptions p JOIN auth_sessions s ON s.id=p.session_id AND s.user_id=p.user_id JOIN users u ON u.id=s.user_id
        WHERE p.user_id=$1 AND p.expires_at>now() AND s.revoked_at IS NULL AND s.idle_expires_at>now() AND s.absolute_expires_at>now() AND u.status='active'
        ORDER BY p.last_confirmed_at DESC,p.id LIMIT 20"#).bind(user).fetch_all().await?;
    Ok(ListPushDevicesResponse { items })
}
pub async fn register(
    pool: &PgPool,
    user: Uuid,
    session: Uuid,
    request: RegisterPushDeviceRequest,
) -> Result<PushDevice, AppError> {
    let config =
        vapid_config().ok_or_else(|| AppError::ServiceUnavailable("push_unavailable".into()))?;
    register_with_key(pool, user, session, request, &config.key_id).await
}
async fn register_with_key(
    pool: &PgPool,
    user: Uuid,
    session: Uuid,
    request: RegisterPushDeviceRequest,
    key_id: &str,
) -> Result<PushDevice, AppError> {
    validate(&request, key_id)?;
    let mut tx =
        school_tenancy::TenantPool::begin_observed(pool, "notifications.devices.register").await?;
    // Serialize this installation across account switches, then all of a user's device admission.
    school_database::DbSession::new(&mut *tx)
        .query(
            "notifications.devices.installation_lock",
            "SELECT pg_advisory_xact_lock(hashtextextended($1::text,139))",
        )
        .bind(request.installation_id.to_string())
        .execute()
        .await?;
    school_database::DbSession::new(&mut *tx)
        .query(
            "notifications.devices.user_lock",
            "SELECT id FROM users WHERE id=$1 FOR UPDATE",
        )
        .bind(user)
        .execute()
        .await?;
    let session_valid:bool=school_database::DbSession::new(&mut *tx).scalar("notifications.devices.session","SELECT EXISTS(SELECT 1 FROM auth_sessions s JOIN users u ON u.id=s.user_id WHERE s.id=$1 AND s.user_id=$2 AND s.revoked_at IS NULL AND s.idle_expires_at>now() AND s.absolute_expires_at>now() AND u.status='active' FOR SHARE OF s)").bind(session).bind(user).fetch_one().await?;
    if !session_valid {
        return Err(AppError::AuthError("session_invalid".into()));
    }
    if let Some(expected) = request.expected_version {
        let binding:Option<(Uuid,Option<Uuid>,Uuid)>=school_database::DbSession::new(&mut *tx).query_as("notifications.devices.conditional_refresh","SELECT user_id,session_id,version FROM push_subscriptions WHERE installation_id=$1 FOR UPDATE").bind(request.installation_id).fetch_optional().await?;
        if binding != Some((user, Some(session), expected)) {
            return Err(AppError::Conflict("push_binding_changed".into()));
        }
    }
    let count:i64=school_database::DbSession::new(&mut *tx).scalar("notifications.devices.admission",r#"SELECT count(*) FROM push_subscriptions p JOIN auth_sessions s ON s.id=p.session_id AND s.user_id=p.user_id JOIN users u ON u.id=s.user_id
        WHERE p.user_id=$1 AND p.installation_id IS DISTINCT FROM $2 AND p.expires_at>now()
        AND s.revoked_at IS NULL AND s.idle_expires_at>now() AND s.absolute_expires_at>now() AND u.status='active'"#).bind(user).bind(request.installation_id).fetch_one().await?;
    if count >= MAX_DEVICES as i64 {
        return Err(AppError::Conflict("push_device_limit".into()));
    }
    // A browser subscription has one account binding, including legacy endpoint-only registrations.
    school_database::DbSession::new(&mut *tx).query("notifications.devices.retire_old_endpoint","DELETE FROM push_subscriptions WHERE endpoint=$1 AND installation_id IS DISTINCT FROM $2").bind(&request.endpoint).bind(request.installation_id).execute().await?;
    let device = register_in_transaction(&mut tx, user, session, &request).await?;
    tx.commit().await?;
    Ok(device)
}
async fn register_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    user: Uuid,
    session: Uuid,
    request: &RegisterPushDeviceRequest,
) -> Result<PushDevice, AppError> {
    Ok(school_database::DbSession::new(&mut **tx).query_as("notifications.devices.upsert",r#"INSERT INTO push_subscriptions(user_id,session_id,installation_id,label,endpoint,p256dh_key,auth_key,key_id,last_confirmed_at,expires_at)
        VALUES($1,$2,$3,$4,$5,$6,$7,$8,now(),now()+interval '90 days')
        ON CONFLICT(installation_id) WHERE installation_id IS NOT NULL DO UPDATE SET
        version=CASE WHEN push_subscriptions.user_id=EXCLUDED.user_id AND push_subscriptions.session_id=EXCLUDED.session_id AND push_subscriptions.endpoint=EXCLUDED.endpoint AND push_subscriptions.p256dh_key=EXCLUDED.p256dh_key AND push_subscriptions.auth_key=EXCLUDED.auth_key AND push_subscriptions.key_id=EXCLUDED.key_id AND push_subscriptions.expires_at>now() THEN push_subscriptions.version ELSE gen_random_uuid() END,
        binding_token=CASE WHEN push_subscriptions.user_id=EXCLUDED.user_id AND push_subscriptions.session_id=EXCLUDED.session_id AND push_subscriptions.endpoint=EXCLUDED.endpoint AND push_subscriptions.p256dh_key=EXCLUDED.p256dh_key AND push_subscriptions.auth_key=EXCLUDED.auth_key AND push_subscriptions.key_id=EXCLUDED.key_id AND push_subscriptions.expires_at>now() THEN push_subscriptions.binding_token ELSE gen_random_uuid() END,
        user_id=EXCLUDED.user_id,session_id=EXCLUDED.session_id,label=EXCLUDED.label,endpoint=EXCLUDED.endpoint,p256dh_key=EXCLUDED.p256dh_key,auth_key=EXCLUDED.auth_key,key_id=EXCLUDED.key_id,last_confirmed_at=now(),updated_at=now(),expires_at=EXCLUDED.expires_at
        RETURNING id,installation_id,label,version,binding_token,key_id,last_confirmed_at,expires_at"#)
        .bind(user).bind(session).bind(request.installation_id).bind(request.label.trim()).bind(&request.endpoint).bind(&request.p256dh).bind(&request.auth).bind(&request.key_id).fetch_one().await?)
}
pub async fn unregister(
    pool: &PgPool,
    user: Uuid,
    id: Uuid,
    version: Uuid,
) -> Result<(), AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "notifications.devices.unregister",
    )
    .await?;
    school_database::DbSession::new(&mut *connection)
        .query(
            "notifications.devices.unregister",
            "DELETE FROM push_subscriptions WHERE id=$1 AND user_id=$2 AND version=$3",
        )
        .bind(id)
        .bind(user)
        .bind(version)
        .execute()
        .await?;
    Ok(())
}
pub(crate) async fn cleanup(pool: &PgPool) -> Result<(), AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "notifications.devices.cleanup")
            .await?;
    school_database::DbSession::new(&mut *connection).query("notifications.devices.cleanup",r#"WITH stale AS (SELECT p.id FROM push_subscriptions p WHERE p.expires_at<=now() OR NOT EXISTS(SELECT 1 FROM auth_sessions s JOIN users u ON u.id=s.user_id WHERE s.id=p.session_id AND s.user_id=p.user_id AND s.revoked_at IS NULL AND s.idle_expires_at>now() AND s.absolute_expires_at>now() AND u.status='active') ORDER BY p.expires_at,p.id LIMIT 100 FOR UPDATE OF p SKIP LOCKED) DELETE FROM push_subscriptions p USING stale WHERE p.id=stale.id"#).execute().await?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn vapid_pair_and_subject_validation_fail_closed_without_touching_devices() {
        let private = URL_SAFE_NO_PAD.encode([1u8; 32]);
        let signer = web_push::VapidSignatureBuilder::from_base64_no_sub(&private).unwrap();
        let public = URL_SAFE_NO_PAD.encode(signer.get_public_key());
        assert!(validate_vapid(
            public.clone(),
            private.clone(),
            "mailto:ops@example.invalid".into()
        )
        .is_some());
        assert!(validate_vapid(
            public.clone(),
            URL_SAFE_NO_PAD.encode([2u8; 32]),
            "mailto:ops@example.invalid".into()
        )
        .is_none());
        assert!(validate_vapid(public.clone(), private.clone(), "not-a-contact".into()).is_none());
        assert!(
            validate_vapid(public, private, "https://example.invalid/contact".into()).is_some()
        );
    }
    #[test]
    fn endpoints_reject_arbitrary_hosts_userinfo_and_non_https() {
        for s in [
            "http://fcm.googleapis.com/x",
            "https://localhost/x",
            "https://127.0.0.1/x",
            "https://fcm.googleapis.com.evil.test/x",
            "https://user@fcm.googleapis.com/x",
            "https://fcm.googleapis.com:444/x",
            "https://fcm.googleapis.com/x#f",
        ] {
            assert!(!valid_endpoint(s), "{s}")
        }
        for s in [
            "https://fcm.googleapis.com/fcm/send/x",
            "https://updates.push.services.mozilla.com/wpush/v2/x",
            "https://web.push.apple.com/x",
        ] {
            assert!(valid_endpoint(s), "{s}")
        }
    }
    #[test]
    fn crypto_encodings_are_bounded_canonical_and_length_checked() {
        assert!(decode(&URL_SAFE_NO_PAD.encode([4; 65]), 65));
        assert!(decode(&URL_SAFE_NO_PAD.encode([0; 16]), 16));
        assert!(!decode(&URL_SAFE_NO_PAD.encode([3; 65]), 65));
        assert!(!decode("bad", 16));
        assert!(!decode(&"a".repeat(4096), 32));
    }
}

#[cfg(feature = "test-support")]
pub async fn register_for_tests(
    pool: &PgPool,
    user: Uuid,
    session: Uuid,
    request: RegisterPushDeviceRequest,
    key_id: &str,
) -> Result<PushDevice, AppError> {
    register_with_key(pool, user, session, request, key_id).await
}
#[cfg(feature = "test-support")]
pub async fn cleanup_for_tests(pool: &PgPool) -> Result<(), AppError> {
    cleanup(pool).await
}
