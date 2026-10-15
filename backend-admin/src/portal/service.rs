use super::{auth::normalize_email, models::*, PortalError};
use axum::http::StatusCode;
use sqlx::PgPool;
use uuid::Uuid;

pub fn validate_subdomain(value: &str) -> Result<(), PortalError> {
    const RESERVED: &[&str] = &[
        "www",
        "admin",
        "school-api",
        "admin-api",
        "server",
        "sandbox",
        "api",
        "mail",
        "cdn",
        "local",
        "assets",
    ];
    if value.is_empty()
        || value.len() > 50
        || value.starts_with('-')
        || value.ends_with('-')
        || !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        || RESERVED.contains(&value)
    {
        return Err(PortalError::invalid());
    }
    Ok(())
}
pub fn validate_cadence(value: &str) -> Result<(), PortalError> {
    if matches!(value, "monthly" | "yearly") {
        Ok(())
    } else {
        Err(PortalError::invalid())
    }
}
pub async fn schools(pool: &PgPool, account: &Account) -> Result<Vec<CustomerSchool>, PortalError> {
    // No credentials/provider metadata in customer responses; expiry never filters access.
    Ok(sqlx::query_as("SELECT s.id,s.name,s.subdomain,p.cadence,p.ends_at,COALESCE(p.ends_at<=now(),false) AS expired FROM schools s JOIN school_memberships m ON m.school_id=s.id LEFT JOIN LATERAL (SELECT cadence,ends_at FROM school_service_periods WHERE school_id=s.id ORDER BY ends_at DESC LIMIT 1) p ON true WHERE m.account_id=$1 ORDER BY s.name,s.id LIMIT 100")
        .bind(account.id).fetch_all(pool).await?)
}
pub async fn requests(
    pool: &PgPool,
    account: &Account,
) -> Result<Vec<ServiceRequest>, PortalError> {
    Ok(sqlx::query_as("SELECT r.id,r.account_id,a.email,r.school_id,r.kind,r.school_name,r.subdomain,r.cadence,r.status,r.decision_reason,r.created_at FROM school_service_requests r JOIN platform_accounts a ON a.id=r.account_id WHERE ($2 OR r.account_id=$1) ORDER BY (r.status='pending') DESC,r.created_at DESC,r.id DESC LIMIT 100")
        .bind(account.id).bind(account.is_operator()).fetch_all(pool).await?)
}
pub async fn create_request(
    pool: &PgPool,
    account: &Account,
    mut input: ServiceRequestInput,
) -> Result<(), PortalError> {
    validate_cadence(&input.cadence)?;
    if input.kind == "renew_school" {
        let school: Option<(String,String)> = sqlx::query_as("SELECT s.name,s.subdomain FROM schools s JOIN school_memberships m ON m.school_id=s.id WHERE m.account_id=$1 AND s.id=$2")
            .bind(account.id).bind(input.school_id).fetch_optional(pool).await?;
        let (name, subdomain) = school.ok_or_else(PortalError::forbidden)?;
        input.school_name = name;
        input.subdomain = subdomain;
    } else if input.kind == "open_school" && input.school_id.is_none() {
        validate_subdomain(&input.subdomain)?;
        input.school_name = input.school_name.trim().to_string();
        if input.school_name.is_empty() || input.school_name.chars().count() > 255 {
            return Err(PortalError::invalid());
        }
    } else {
        return Err(PortalError::invalid());
    }
    let mut tx = pool.begin().await?;
    // Serialize per account so request quotas and replays remain deterministic.
    sqlx::query("SELECT id FROM platform_accounts WHERE id=$1 FOR UPDATE")
        .bind(account.id)
        .execute(&mut *tx)
        .await?;
    let existing: Option<(Uuid,Option<Uuid>,String,String,String,String)> = sqlx::query_as("SELECT account_id,school_id,kind,school_name,subdomain,cadence FROM school_service_requests WHERE id=$1")
        .bind(input.id).fetch_optional(&mut *tx).await?;
    if let Some(existing) = existing {
        if existing
            != (
                account.id,
                input.school_id,
                input.kind,
                input.school_name,
                input.subdomain,
                input.cadence,
            )
        {
            return Err(PortalError::forbidden());
        }
        return Ok(());
    }
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM school_service_requests WHERE account_id=$1 AND status='pending'",
    )
    .bind(account.id)
    .fetch_one(&mut *tx)
    .await?;
    if count >= 10 {
        return Err(PortalError(
            StatusCode::CONFLICT,
            "มีคำขอที่รอตรวจสอบครบแล้ว กรุณารอทีมงาน",
        ));
    }
    sqlx::query("INSERT INTO school_service_requests(id,account_id,school_id,kind,school_name,subdomain,cadence) VALUES($1,$2,$3,$4,$5,$6,$7)")
        .bind(input.id).bind(account.id).bind(input.school_id).bind(input.kind).bind(input.school_name).bind(input.subdomain).bind(input.cadence).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
pub async fn assign(
    pool: &PgPool,
    operator: &Account,
    school: Uuid,
    email: &str,
) -> Result<(), PortalError> {
    if !operator.is_operator() {
        return Err(PortalError::forbidden());
    }
    let email = normalize_email(email)?;
    let account: Option<Uuid>=sqlx::query_scalar("SELECT id FROM platform_accounts WHERE email=$1 AND verified_at IS NOT NULL AND role='customer'")
        .bind(email).fetch_optional(pool).await?;
    let id = account.ok_or_else(PortalError::invalid)?;
    let assigned = sqlx::query("INSERT INTO school_memberships(school_id,account_id,granted_by) SELECT id,$2,$3 FROM schools WHERE id=$1 ON CONFLICT(school_id,account_id) DO NOTHING")
        .bind(school).bind(id).bind(operator.id).execute(pool).await?;
    if assigned.rows_affected() == 0 {
        let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM schools WHERE id=$1)")
            .bind(school)
            .fetch_one(pool)
            .await?;
        if !exists {
            return Err(PortalError(StatusCode::NOT_FOUND, "ไม่พบโรงเรียน"));
        }
    }
    Ok(())
}
pub async fn decide(
    pool: &PgPool,
    operator: &Account,
    request: Uuid,
    input: DecisionInput,
) -> Result<(), PortalError> {
    if !operator.is_operator() {
        return Err(PortalError::forbidden());
    }
    if input.reason.trim().is_empty() || input.reason.chars().count() > 500 {
        return Err(PortalError::invalid());
    }
    let mut tx = pool.begin().await?;
    let row: Option<(Uuid,Option<Uuid>,String,String,String,String)> = sqlx::query_as("SELECT account_id,school_id,kind,subdomain,cadence,status FROM school_service_requests WHERE id=$1 FOR UPDATE")
        .bind(request).fetch_optional(&mut *tx).await?;
    let (account, original_school, kind, subdomain, cadence, status) =
        row.ok_or_else(PortalError::invalid)?;
    if status != "pending" {
        return Err(PortalError(
            StatusCode::CONFLICT,
            "คำขอนี้ได้รับการพิจารณาแล้ว กรุณาโหลดข้อมูลใหม่",
        ));
    }
    if input.approve {
        let school = input
            .school_id
            .or(original_school)
            .ok_or_else(PortalError::invalid)?;
        if kind == "renew_school" && original_school != Some(school) {
            return Err(PortalError::invalid());
        }
        let current: Option<String> =
            sqlx::query_scalar("SELECT subdomain FROM schools WHERE id=$1 FOR UPDATE")
                .bind(school)
                .fetch_optional(&mut *tx)
                .await?;
        if current.as_deref() != Some(subdomain.as_str()) {
            return Err(PortalError::invalid());
        }
        if kind == "renew_school" {
            let member: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM school_memberships WHERE school_id=$1 AND account_id=$2)")
                .bind(school).bind(account).fetch_one(&mut *tx).await?;
            if !member {
                return Err(PortalError::forbidden());
            }
        }
        sqlx::query("INSERT INTO school_memberships(school_id,account_id,granted_by) VALUES($1,$2,$3) ON CONFLICT(school_id,account_id) DO NOTHING")
            .bind(school).bind(account).bind(operator.id).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO school_service_periods(school_id,cadence,starts_at,ends_at,granted_by) SELECT $1,$2,start_at,start_at+CASE WHEN $2='monthly' THEN interval '1 month' ELSE interval '1 year' END,$3 FROM (SELECT GREATEST(now(),COALESCE(max(ends_at),now())) AS start_at FROM school_service_periods WHERE school_id=$1) p")
            .bind(school).bind(cadence).bind(operator.id).execute(&mut *tx).await?;
        sqlx::query("UPDATE school_service_requests SET school_id=$2 WHERE id=$1")
            .bind(request)
            .bind(school)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("UPDATE school_service_requests SET status=$2,decided_by=$3,decision_reason=$4,decided_at=now() WHERE id=$1")
        .bind(request).bind(if input.approve {"approved"} else {"rejected"}).bind(operator.id).bind(input.reason.trim()).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn infrastructure_hostnames_are_reserved() {
        for name in [
            "admin",
            "www",
            "server",
            "sandbox",
            "admin-api",
            "school-api",
            "UPPER",
            "a.b",
            "-a",
        ] {
            assert!(validate_subdomain(name).is_err());
        }
        assert!(validate_subdomain("school-123").is_ok());
        assert!(validate_cadence("monthly").is_ok());
        assert!(validate_cadence("yearly").is_ok());
        assert!(validate_cadence("forever").is_err());
    }
}
