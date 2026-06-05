use sqlx::PgPool;
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
        eprintln!("Database error: {}", e);
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
        eprintln!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    let consents = sqlx::query_as::<_, ConsentRecord>(
        "SELECT * FROM consent_records
         WHERE user_id = $1
         ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        eprintln!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    let consent_responses: Vec<ConsentRecordResponse> =
        consents.into_iter().map(consent_record_response).collect();

    let granted_required_codes: Vec<String> = consent_responses
        .iter()
        .filter(|consent| consent.consent_status == "granted" && !consent.is_expired)
        .map(|consent| consent.consent_type.clone())
        .collect();

    let required_codes: Vec<String> = required_types
        .iter()
        .map(|item| item.code.clone())
        .collect();

    let missing_required: Vec<String> = required_codes
        .iter()
        .filter(|code| !granted_required_codes.contains(code))
        .cloned()
        .collect();

    Ok(UserConsentStatus {
        user_id,
        user_type,
        total_required: required_codes.len() as i32,
        granted_required: granted_required_codes.len() as i32,
        is_compliant: missing_required.is_empty(),
        missing_required_consents: missing_required,
        consents: consent_responses,
    })
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
        eprintln!("Database error: {}", e);
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
    .bind(serde_json::json!(["personal_info"]))
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
        eprintln!("Failed to create consent: {}", e);
        AppError::InternalServerError("ไม่สามารถบันทึกความยินยอมได้".to_string())
    })
}

pub async fn withdraw_consent(
    pool: &PgPool,
    user_id: Uuid,
    consent_id: Uuid,
) -> Result<(), AppError> {
    let consent = sqlx::query_as::<_, ConsentRecord>(
        "SELECT * FROM consent_records WHERE id = $1 AND user_id = $2",
    )
    .bind(consent_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| {
        eprintln!("Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?
    .ok_or(AppError::NotFound("ไม่พบความยินยอมนี้".to_string()))?;

    let is_required: bool =
        sqlx::query_scalar("SELECT is_required FROM consent_types WHERE code = $1")
            .bind(&consent.consent_type)
            .fetch_optional(pool)
            .await
            .map_err(|e| {
                eprintln!("Database error: {}", e);
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
        eprintln!("Failed to withdraw consent: {}", e);
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

    let compliance_rate = if total_users > 0 {
        (granted as f64 / total_users as f64) * 100.0
    } else {
        0.0
    };

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
            eprintln!("Failed to get user type: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })
}

async fn count_scalar(pool: &PgPool, query: &str) -> Result<i64, AppError> {
    sqlx::query_scalar(query)
        .fetch_one(pool)
        .await
        .map_err(|e| {
            eprintln!("Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })
}

fn consent_record_response(record: ConsentRecord) -> ConsentRecordResponse {
    let data_categories: Vec<String> =
        serde_json::from_value(record.data_categories.clone()).unwrap_or_default();
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
        data_categories,
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
