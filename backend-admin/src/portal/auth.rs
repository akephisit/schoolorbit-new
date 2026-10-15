use super::{models::*, PortalError};
use crate::auth::{hash_password, verify_password};
use axum::http::{HeaderMap, StatusCode};
use rand::TryRng;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use subtle::ConstantTimeEq;
use uuid::Uuid;

pub const COOKIE: &str = "__Host-schoolorbit_admin";
pub fn digest(value: &str) -> String {
    hex::encode(Sha256::digest(value.as_bytes()))
}
pub fn csrf(token: &str) -> String {
    digest(&format!("admin-csrf:v1:{token}"))
}
pub fn random_token() -> Result<String, PortalError> {
    let mut bytes = [0u8; 32];
    rand::rngs::SysRng
        .try_fill_bytes(&mut bytes)
        .map_err(|_| PortalError::unavailable())?;
    Ok(hex::encode(bytes))
}
pub fn valid_token(token: &str) -> bool {
    token.len() == 64 && token.bytes().all(|b| b.is_ascii_hexdigit())
}
pub fn normalize_email(input: &str) -> Result<String, PortalError> {
    let email = input.trim().to_ascii_lowercase();
    let parts: Vec<_> = email.split('@').collect();
    if email.len() > 254 || parts.len() != 2 {
        return Err(PortalError::invalid());
    }
    let local = parts[0];
    let domain = parts[1];
    if local.is_empty()
        || local.len() > 64
        || local.starts_with('.')
        || local.ends_with('.')
        || local.contains("..")
        || !local
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&b))
        || !domain.contains('.')
        || !domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
    {
        return Err(PortalError::invalid());
    }
    Ok(email)
}
pub fn validate_password(password: &str) -> Result<(), PortalError> {
    if !(8..=128).contains(&password.chars().count()) || password.len() > 71 {
        return Err(PortalError(
            StatusCode::BAD_REQUEST,
            "รหัสผ่านต้องมีอย่างน้อย 8 ตัวอักษร และไม่เกิน 71 ไบต์",
        ));
    }
    Ok(())
}
pub fn portal_origin() -> Result<String, PortalError> {
    let base = std::env::var("BASE_DOMAIN").map_err(|_| PortalError::unavailable())?;
    let url =
        reqwest::Url::parse(&format!("https://{base}")).map_err(|_| PortalError::unavailable())?;
    if url.host_str() != Some(base.as_str()) || url.port().is_some() || !url.username().is_empty() {
        return Err(PortalError::unavailable());
    }
    Ok(url.origin().ascii_serialization())
}
pub fn require_origin(headers: &HeaderMap) -> Result<(), PortalError> {
    let expected = portal_origin()?;
    let origin = headers.get("origin").and_then(|v| v.to_str().ok());
    let local = std::env::var("ADMIN_ALLOWED_DEV_ORIGIN").ok().filter(|s| {
        matches!(
            s.as_str(),
            "http://localhost:5173"
                | "http://127.0.0.1:5173"
                | "https://localhost:5173"
                | "https://127.0.0.1:5173"
        )
    });
    if origin == Some(expected.as_str())
        || (local.as_deref().is_some() && origin == local.as_deref())
    {
        Ok(())
    } else {
        Err(PortalError::forbidden())
    }
}
pub fn require_csrf(headers: &HeaderMap, token: &str) -> Result<(), PortalError> {
    require_origin(headers)?;
    let expected = csrf(token);
    let actual = headers
        .get("x-csrf-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    if bool::from(expected.as_bytes().ct_eq(actual.as_bytes())) {
        Ok(())
    } else {
        Err(PortalError::forbidden())
    }
}
pub async fn throttle(pool: &PgPool, key: &str, limit: i32) -> Result<(), PortalError> {
    // A fixed bounded window survives restarts; only hashed identifiers are retained.
    sqlx::query("DELETE FROM platform_auth_buckets WHERE (bucket,window_start) IN (SELECT bucket,window_start FROM platform_auth_buckets WHERE window_start < now()-interval '1 day' LIMIT 100)")
        .execute(pool).await?;
    let attempts: i32 = sqlx::query_scalar("INSERT INTO platform_auth_buckets(bucket,window_start,attempts) VALUES($1,date_bin('15 minutes',now(),'2000-01-01'::timestamptz),1) ON CONFLICT(bucket,window_start) DO UPDATE SET attempts=platform_auth_buckets.attempts+1 RETURNING attempts")
        .bind(digest(key)).fetch_one(pool).await?;
    if attempts > limit {
        Err(PortalError(
            StatusCode::TOO_MANY_REQUESTS,
            "กรุณารอสักครู่ก่อนลองอีกครั้ง",
        ))
    } else {
        Ok(())
    }
}
pub async fn password_hash(password: String) -> Result<String, PortalError> {
    validate_password(&password)?;
    tokio::task::spawn_blocking(move || hash_password(&password))
        .await
        .map_err(|_| PortalError::unavailable())?
        .map_err(|_| PortalError::unavailable())
}
pub async fn register(pool: &PgPool, input: Registration) -> Result<(), PortalError> {
    let email = normalize_email(&input.email)?;
    let name = input.name.trim();
    if name.is_empty() || name.chars().count() > 120 {
        return Err(PortalError::invalid());
    }
    let hash = password_hash(input.password).await?;
    sqlx::query("INSERT INTO platform_accounts(email,password_hash,name) VALUES($1,$2,$3) ON CONFLICT(email) DO NOTHING")
        .bind(&email).bind(hash).bind(name).execute(pool).await?;
    send_link(pool, &email, "verify").await
}
pub async fn send_link(pool: &PgPool, email: &str, purpose: &str) -> Result<(), PortalError> {
    let origin = portal_origin()?;
    let api_key = std::env::var("RESEND_API_KEY")
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or_else(PortalError::unavailable)?;
    let sender = std::env::var("RESEND_FROM")
        .ok()
        .filter(|s| !s.is_empty())
        .ok_or_else(PortalError::unavailable)?;
    let mut tx = pool.begin().await?;
    let account: Option<(Uuid, Option<chrono::DateTime<chrono::Utc>>)> =
        sqlx::query_as("SELECT id,verified_at FROM platform_accounts WHERE email=$1 FOR UPDATE")
            .bind(email)
            .fetch_optional(&mut *tx)
            .await?;
    let Some((id, verified)) = account else {
        return Ok(());
    };
    if purpose == "verify" && verified.is_some() {
        return Ok(());
    }
    if purpose == "reset" && verified.is_none() {
        return Ok(());
    }
    let token = random_token()?;
    sqlx::query("INSERT INTO platform_email_tokens(token_hash,account_id,purpose,expires_at) VALUES($1,$2,$3,now()+CASE WHEN $3='verify' THEN interval '24 hours' ELSE interval '1 hour' END) ON CONFLICT(account_id,purpose) DO UPDATE SET token_hash=EXCLUDED.token_hash,expires_at=EXCLUDED.expires_at")
        .bind(digest(&token)).bind(id).bind(purpose).execute(&mut *tx).await?;
    tx.commit().await?;
    let (path, subject) = if purpose == "verify" {
        ("verify-email", "ยืนยันอีเมล SchoolOrbit")
    } else {
        ("reset-password", "ตั้งรหัสผ่าน SchoolOrbit ใหม่")
    };
    #[derive(serde::Serialize)]
    struct Email<'a> {
        from: &'a str,
        to: [&'a str; 1],
        subject: &'a str,
        text: String,
    }
    let response = reqwest::Client::builder().timeout(std::time::Duration::from_secs(10)).build()
        .map_err(|_| PortalError::unavailable())?.post("https://api.resend.com/emails")
        .bearer_auth(api_key).header("Idempotency-Key", format!("admin-{purpose}-{}", digest(&token)))
        .json(&Email { from: &sender, to: [email], subject, text: format!("{subject}\n\n{origin}/{path}#token={token}\n\nหากคุณไม่ได้ขอทำรายการนี้ สามารถละเว้นอีเมลนี้ได้") })
        .send().await.map_err(|_| PortalError::unavailable())?;
    if !response.status().is_success() {
        tracing::warn!(
            operation = "admin_email_delivery",
            status = response.status().as_u16(),
            "Email delivery refused; request a new link"
        );
        return Err(PortalError::unavailable());
    }
    Ok(())
}
pub async fn consume_token(
    pool: &PgPool,
    token: &str,
    password: Option<String>,
) -> Result<(), PortalError> {
    if !valid_token(token) {
        return Err(PortalError::invalid());
    }
    let purpose = if password.is_some() {
        "reset"
    } else {
        "verify"
    };
    let hash = match password {
        Some(value) => Some(password_hash(value).await?),
        None => None,
    };
    let mut tx = pool.begin().await?;
    // Lock the account first, the same order as issuance, to serialize reset and resend.
    let account: Option<Uuid> = sqlx::query_scalar("SELECT a.id FROM platform_accounts a JOIN platform_email_tokens t ON t.account_id=a.id WHERE t.token_hash=$1 AND t.purpose=$2 FOR UPDATE OF a")
        .bind(digest(token)).bind(purpose).fetch_optional(&mut *tx).await?;
    let id = account.ok_or_else(PortalError::invalid)?;
    let consumed: Option<Uuid> = sqlx::query_scalar("DELETE FROM platform_email_tokens WHERE token_hash=$1 AND purpose=$2 AND expires_at>now() RETURNING account_id")
        .bind(digest(token)).bind(purpose).fetch_optional(&mut *tx).await?;
    if consumed != Some(id) {
        return Err(PortalError::invalid());
    }
    if let Some(hash) = hash {
        sqlx::query("UPDATE platform_accounts SET password_hash=$1 WHERE id=$2")
            .bind(hash)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM platform_sessions WHERE account_id=$1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    } else {
        sqlx::query(
            "UPDATE platform_accounts SET verified_at=COALESCE(verified_at,now()) WHERE id=$1",
        )
        .bind(id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
pub async fn login(pool: &PgPool, input: Credentials) -> Result<(Account, String), PortalError> {
    let email = normalize_email(&input.email)?;
    validate_password(&input.password)?;
    #[derive(sqlx::FromRow)]
    struct LoginRow {
        id: Uuid,
        password_hash: String,
    }
    let mut tx = pool.begin().await?;
    let row: Option<LoginRow> = sqlx::query_as("SELECT id,password_hash FROM platform_accounts WHERE email=$1 AND verified_at IS NOT NULL FOR UPDATE")
        .bind(&email).fetch_optional(&mut *tx).await?;
    let stored = row
        .as_ref()
        .map(|r| r.password_hash.clone())
        .unwrap_or_else(|| "$2b$12$rri4KlbA4XkEM.JADCiBdu3/cjxESITAXMOYiKoklPN9HW6vgjEP2".into());
    let valid = tokio::task::spawn_blocking(move || verify_password(&input.password, &stored))
        .await
        .map_err(|_| PortalError::unavailable())?
        .map_err(|_| PortalError::unavailable())?;
    if !valid {
        return Err(PortalError(
            StatusCode::UNAUTHORIZED,
            "อีเมลหรือรหัสผ่านไม่ถูกต้อง หรือยังไม่ได้ยืนยันอีเมล",
        ));
    }
    let id = row
        .ok_or(PortalError(
            StatusCode::UNAUTHORIZED,
            "อีเมลหรือรหัสผ่านไม่ถูกต้อง หรือยังไม่ได้ยืนยันอีเมล",
        ))?
        .id;
    let token = random_token()?;
    sqlx::query("DELETE FROM platform_sessions WHERE token_hash IN (SELECT token_hash FROM platform_sessions WHERE account_id=$1 ORDER BY created_at DESC OFFSET 9)")
        .bind(id).execute(&mut *tx).await?;
    sqlx::query("INSERT INTO platform_sessions(token_hash,account_id,expires_at) VALUES($1,$2,now()+interval '1 day')")
        .bind(digest(&token)).bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    let user = authenticate(pool, &token).await?;
    Ok((user, token))
}
pub async fn authenticate(pool: &PgPool, token: &str) -> Result<Account, PortalError> {
    if !valid_token(token) {
        return Err(PortalError::unauthorized());
    }
    sqlx::query_as("SELECT a.id,a.email,a.name,a.role FROM platform_accounts a JOIN platform_sessions s ON s.account_id=a.id WHERE s.token_hash=$1 AND s.expires_at>now() AND a.verified_at IS NOT NULL")
        .bind(digest(token)).fetch_optional(pool).await?.ok_or_else(PortalError::unauthorized)
}
pub async fn logout(pool: &PgPool, token: &str) -> Result<(), PortalError> {
    sqlx::query("DELETE FROM platform_sessions WHERE token_hash=$1")
        .bind(digest(token))
        .execute(pool)
        .await?;
    Ok(())
}
pub async fn bootstrap_operator(
    pool: &PgPool,
    email: &str,
    password: String,
    name: &str,
) -> Result<(), PortalError> {
    let email = normalize_email(email)?;
    if name.trim().is_empty() || name.chars().count() > 120 {
        return Err(PortalError::invalid());
    }
    let hash = password_hash(password).await?;
    // Never promote an existing self-registered customer or reset an existing operator implicitly.
    sqlx::query(
        "INSERT INTO platform_accounts(email,password_hash,name,role) VALUES($1,$2,$3,'operator')",
    )
    .bind(email)
    .bind(hash)
    .bind(name.trim())
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn email_normalization_does_not_accept_ambiguous_addresses() {
        assert_eq!(
            normalize_email(" Owner@School.Example ").unwrap(),
            "owner@school.example"
        );
        for value in [
            "a@@school.test",
            "a@school",
            "a\n@school.test",
            ".a@school.test",
            "a@-school.test",
        ] {
            assert!(normalize_email(value).is_err());
        }
    }
    #[test]
    fn passwords_are_never_silently_truncated() {
        assert!(validate_password("12345678").is_ok());
        assert!(validate_password(&"ก".repeat(24)).is_err());
        assert!(validate_password("short").is_err());
    }
    #[test]
    fn session_and_csrf_have_distinct_domains() {
        let token = random_token().unwrap();
        assert!(valid_token(&token));
        assert_ne!(digest(&token), csrf(&token));
        assert_ne!(token, random_token().unwrap());
        assert!(!valid_token("synthetic.legacy.jwt"));
    }
}
