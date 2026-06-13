use std::collections::HashSet;

use sqlx::{types::Json, PgPool};
use uuid::Uuid;

use crate::error::AppError;

use super::models::{
    ConsentRecord, ConsentRecordResponse, ConsentSummary, ConsentType, ConsentTypeResponse,
    CreateConsentRequest, UserConsentStatus,
};

pub struct ConsentRequestContext {
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct ConsentRecordRow {
    id: Uuid,
    user_id: Uuid,
    user_type: String,
    consent_type: String,
    purpose: String,
    data_categories: Json<Vec<String>>,
    consent_status: String,
    granted_at: Option<chrono::DateTime<chrono::Utc>>,
    withdrawn_at: Option<chrono::DateTime<chrono::Utc>>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
    consent_method: String,
    ip_address: Option<String>,
    user_agent: Option<String>,
    consent_text: Option<String>,
    consent_version: String,
    is_minor_consent: bool,
    parent_guardian_id: Option<Uuid>,
    parent_guardian_name: Option<String>,
    parent_relationship: Option<String>,
    notes: Option<String>,
    metadata: serde_json::Value,
    created_at: chrono::DateTime<chrono::Utc>,
    updated_at: chrono::DateTime<chrono::Utc>,
}

impl From<ConsentRecordRow> for ConsentRecord {
    fn from(row: ConsentRecordRow) -> Self {
        Self {
            id: row.id,
            user_id: row.user_id,
            user_type: row.user_type,
            consent_type: row.consent_type,
            purpose: row.purpose,
            data_categories: row.data_categories.0,
            consent_status: row.consent_status,
            granted_at: row.granted_at,
            withdrawn_at: row.withdrawn_at,
            expires_at: row.expires_at,
            consent_method: row.consent_method,
            ip_address: row.ip_address,
            user_agent: row.user_agent,
            consent_text: row.consent_text,
            consent_version: row.consent_version,
            is_minor_consent: row.is_minor_consent,
            parent_guardian_id: row.parent_guardian_id,
            parent_guardian_name: row.parent_guardian_name,
            parent_relationship: row.parent_relationship,
            notes: row.notes,
            metadata: row.metadata,
            created_at: row.created_at,
            updated_at: row.updated_at,
        }
    }
}

fn default_consent_data_categories() -> Json<Vec<String>> {
    Json(vec!["personal_info".to_string()])
}

pub async fn list_consent_types(
    pool: &PgPool,
    user_type: &str,
) -> Result<Vec<ConsentTypeResponse>, AppError> {
    let consent_types = sqlx::query_as::<_, ConsentType>(
        "SELECT * FROM consent_types
         WHERE is_active = true
         AND $1 = ANY(applicable_user_types)
         ORDER BY priority DESC",
    )
    .bind(user_type)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    Ok(consent_types
        .into_iter()
        .map(ConsentTypeResponse::from)
        .collect())
}

pub async fn get_user_consent_status(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<UserConsentStatus, AppError> {
    let user_type = get_user_type(pool, user_id).await?;

    let required_types = sqlx::query_as::<_, ConsentType>(
        "SELECT * FROM consent_types
         WHERE is_required = true
         AND is_active = true
         AND $1 = ANY(applicable_user_types)",
    )
    .bind(&user_type)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    let consent_rows = sqlx::query_as::<_, ConsentRecordRow>(
        "SELECT * FROM consent_records
         WHERE user_id = $1
         ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    let consent_responses: Vec<ConsentRecordResponse> = consent_rows
        .into_iter()
        .map(ConsentRecord::from)
        .map(consent_record_response)
        .collect();

    let required_codes: Vec<String> = required_types
        .iter()
        .map(|item| item.code.clone())
        .collect();

    Ok(build_user_consent_status(
        user_id,
        user_type,
        required_codes,
        consent_responses,
    ))
}

pub async fn create_consent(
    pool: &PgPool,
    user_id: Uuid,
    payload: CreateConsentRequest,
    context: ConsentRequestContext,
) -> Result<Uuid, AppError> {
    let user_type = get_user_type(pool, user_id).await?;

    let consent_type_data = sqlx::query_as::<_, ConsentType>(
        "SELECT * FROM consent_types WHERE code = $1 AND is_active = true",
    )
    .bind(&payload.consent_type)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?
    .ok_or(AppError::NotFound("ไม่พบประเภทความยินยอมนี้".to_string()))?;

    let expires_at = consent_type_data
        .default_duration_days
        .map(|days| chrono::Utc::now() + chrono::Duration::days(days as i64));
    let granted_at = if payload.consent_status == "granted" {
        Some(chrono::Utc::now())
    } else {
        None
    };

    sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO consent_records (
            user_id, user_type, consent_type, purpose, data_categories,
            consent_status, granted_at, expires_at, consent_method,
            ip_address, user_agent, consent_text, consent_version,
            is_minor_consent, parent_guardian_name, parent_relationship
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16)
        RETURNING id",
    )
    .bind(user_id)
    .bind(&user_type)
    .bind(&payload.consent_type)
    .bind(consent_type_data.description.unwrap_or_default())
    .bind(default_consent_data_categories())
    .bind(&payload.consent_status)
    .bind(granted_at)
    .bind(expires_at)
    .bind("web_form")
    .bind(context.ip_address)
    .bind(context.user_agent)
    .bind(&consent_type_data.consent_text_template)
    .bind(&consent_type_data.consent_version)
    .bind(payload.is_minor_consent.unwrap_or(false))
    .bind(&payload.parent_guardian_name)
    .bind(&payload.parent_relationship)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to create consent: {}", e);
        AppError::InternalServerError("ไม่สามารถบันทึกความยินยอมได้".to_string())
    })
}

pub async fn withdraw_consent(
    pool: &PgPool,
    user_id: Uuid,
    consent_id: Uuid,
) -> Result<(), AppError> {
    let consent_type: String = sqlx::query_scalar(
        "SELECT consent_type FROM consent_records WHERE id = $1 AND user_id = $2",
    )
    .bind(consent_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        tracing::error!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?
    .ok_or(AppError::NotFound("ไม่พบความยินยอมนี้".to_string()))?;

    let is_required: bool =
        sqlx::query_scalar("SELECT is_required FROM consent_types WHERE code = $1")
            .bind(&consent_type)
            .fetch_optional(pool)
            .await
            .map_err(|e| {
                tracing::error!("Database error: {}", e);
                AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
            })?
            .unwrap_or(false);

    if is_required {
        return Err(AppError::BadRequest(
            "ไม่สามารถถอนความยินยอมที่จำเป็นได้".to_string(),
        ));
    }

    sqlx::query(
        "UPDATE consent_records
         SET consent_status = 'withdrawn', withdrawn_at = NOW(), updated_at = NOW()
         WHERE id = $1",
    )
    .bind(consent_id)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to withdraw consent: {}", e);
        AppError::InternalServerError("ไม่สามารถถอนความยินยอมได้".to_string())
    })?;

    Ok(())
}

pub async fn get_consent_summary(pool: &PgPool) -> Result<ConsentSummary, AppError> {
    let total_users =
        count_scalar(pool, "SELECT COUNT(*) FROM users WHERE status = 'active'").await?;
    let total_consents = count_scalar(pool, "SELECT COUNT(*) FROM consent_records").await?;
    let granted = count_scalar(
        pool,
        "SELECT COUNT(*) FROM consent_records WHERE consent_status = 'granted'",
    )
    .await?;
    let denied = count_scalar(
        pool,
        "SELECT COUNT(*) FROM consent_records WHERE consent_status = 'denied'",
    )
    .await?;
    let withdrawn = count_scalar(
        pool,
        "SELECT COUNT(*) FROM consent_records WHERE consent_status = 'withdrawn'",
    )
    .await?;
    let pending = count_scalar(
        pool,
        "SELECT COUNT(*) FROM consent_records WHERE consent_status = 'pending'",
    )
    .await?;

    let compliance_rate = consent_compliance_rate(granted, total_users);

    Ok(ConsentSummary {
        total_users,
        total_consents,
        granted,
        denied,
        withdrawn,
        pending,
        compliance_rate,
    })
}

async fn get_user_type(pool: &PgPool, user_id: Uuid) -> Result<String, AppError> {
    sqlx::query_scalar("SELECT user_type FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get user type: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })
}

async fn count_scalar(pool: &PgPool, query: &str) -> Result<i64, AppError> {
    sqlx::query_scalar(query)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            tracing::error!("Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })
}

fn consent_record_response(record: ConsentRecord) -> ConsentRecordResponse {
    let is_expired = record
        .expires_at
        .map(|expires_at| expires_at < chrono::Utc::now())
        .unwrap_or(false);

    ConsentRecordResponse {
        id: record.id,
        user_id: record.user_id,
        user_type: record.user_type,
        consent_type: record.consent_type,
        consent_type_name: None,
        purpose: record.purpose,
        data_categories: record.data_categories,
        consent_status: record.consent_status,
        granted_at: record.granted_at,
        withdrawn_at: record.withdrawn_at,
        expires_at: record.expires_at,
        is_expired,
        is_required: false,
        consent_method: record.consent_method,
        is_minor_consent: record.is_minor_consent,
        parent_guardian_name: record.parent_guardian_name,
        created_at: record.created_at,
    }
}

fn build_user_consent_status(
    user_id: Uuid,
    user_type: String,
    required_codes: Vec<String>,
    consents: Vec<ConsentRecordResponse>,
) -> UserConsentStatus {
    let required_code_set: HashSet<&str> = required_codes.iter().map(String::as_str).collect();
    let granted_required_codes: HashSet<&str> = consents
        .iter()
        .filter(|consent| {
            required_code_set.contains(consent.consent_type.as_str())
                && consent.consent_status == "granted"
                && !consent.is_expired
        })
        .map(|consent| consent.consent_type.as_str())
        .collect();

    let missing_required: Vec<String> = required_codes
        .iter()
        .filter(|code| !granted_required_codes.contains(code.as_str()))
        .cloned()
        .collect();

    UserConsentStatus {
        user_id,
        user_type,
        total_required: required_codes.len() as i32,
        granted_required: granted_required_codes.len() as i32,
        is_compliant: missing_required.is_empty(),
        missing_required_consents: missing_required,
        consents,
    }
}

fn consent_compliance_rate(granted: i64, total_users: i64) -> f64 {
    if total_users > 0 {
        (granted as f64 / total_users as f64) * 100.0
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn consent_response(code: &str, status: &str, is_expired: bool) -> ConsentRecordResponse {
        ConsentRecordResponse {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            user_type: "student".to_string(),
            consent_type: code.to_string(),
            consent_type_name: None,
            purpose: "test".to_string(),
            data_categories: vec![],
            consent_status: status.to_string(),
            granted_at: None,
            withdrawn_at: None,
            expires_at: None,
            is_expired,
            is_required: false,
            consent_method: "web_form".to_string(),
            is_minor_consent: false,
            parent_guardian_name: None,
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn consent_status_marks_user_compliant_when_all_required_are_granted_and_current() {
        let user_id = Uuid::new_v4();
        let status = build_user_consent_status(
            user_id,
            "student".to_string(),
            vec!["pdpa".to_string(), "photo".to_string()],
            vec![
                consent_response("pdpa", "granted", false),
                consent_response("photo", "granted", false),
            ],
        );

        assert!(status.is_compliant);
        assert_eq!(status.total_required, 2);
        assert_eq!(status.granted_required, 2);
        assert!(status.missing_required_consents.is_empty());
    }

    #[test]
    fn consent_status_reports_missing_expired_or_denied_required_consents() {
        let user_id = Uuid::new_v4();
        let status = build_user_consent_status(
            user_id,
            "student".to_string(),
            vec![
                "pdpa".to_string(),
                "photo".to_string(),
                "health".to_string(),
            ],
            vec![
                consent_response("pdpa", "granted", false),
                consent_response("photo", "granted", true),
                consent_response("health", "denied", false),
            ],
        );

        assert!(!status.is_compliant);
        assert_eq!(status.granted_required, 1);
        assert_eq!(
            status.missing_required_consents,
            vec!["photo".to_string(), "health".to_string()]
        );
    }

    #[test]
    fn consent_status_counts_only_required_granted_consents() {
        let user_id = Uuid::new_v4();
        let status = build_user_consent_status(
            user_id,
            "student".to_string(),
            vec!["pdpa".to_string()],
            vec![
                consent_response("pdpa", "granted", false),
                consent_response("optional_marketing", "granted", false),
            ],
        );

        assert!(status.is_compliant);
        assert_eq!(status.total_required, 1);
        assert_eq!(status.granted_required, 1);
    }

    #[test]
    fn compliance_rate_is_zero_when_there_are_no_users() {
        assert_eq!(consent_compliance_rate(0, 5), 0.0);
    }

    #[test]
    fn compliance_rate_uses_granted_consents_over_total_users() {
        assert_eq!(consent_compliance_rate(4, 10), 40.0);
    }

    #[test]
    fn default_consent_categories_are_typed_and_stable() {
        assert_eq!(
            default_consent_data_categories().0,
            vec!["personal_info".to_string()]
        );
    }
}
