use crate::db::school_mapping::get_school_database_url;
use crate::modules::auth::models::Claims;
use crate::modules::consent::models::{
    ConsentRecord, ConsentRecordResponse, ConsentSummary, ConsentType,
    ConsentTypeResponse, CreateConsentRequest, UserConsentStatus,
};
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;
use crate::error::AppError;
use axum::{
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

// ===================================================================
// Consent Types Management (ประเภทความยินยอม)
// ===================================================================

/// Get all consent types (filtered by user type)
/// GET /api/consent/types?user_type=student
pub async fn get_consent_types(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;

    // Get user_type from query params
    let user_type = headers
        .get("user-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("student");

    let consent_types = sqlx::query_as::<_, ConsentType>(
        "SELECT * FROM consent_types 
         WHERE is_active = true 
         AND $1 = ANY(applicable_user_types)
         ORDER BY priority DESC",
    )
    .bind(user_type)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    let responses: Vec<ConsentTypeResponse> = consent_types
        .into_iter()
        .map(ConsentTypeResponse::from)
        .collect();

    Ok((StatusCode::OK, Json(responses)))
}

// ===================================================================
// User Consents Management (ความยินยอมของผู้ใช้)
// ===================================================================

/// Get user's consent status
/// GET /api/consent/my-status
pub async fn get_my_consent_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: Request,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let claims = req.extensions().get::<Claims>()
        .ok_or(AppError::AuthError("ไม่พบข้อมูลผู้ใช้".to_string()))?
        .clone();

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::BadRequest("Invalid user ID".to_string()))?;

    // Get user type
    let user_type: String = sqlx::query_scalar("SELECT user_type FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get user type: {}", e);
             AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;

    // Get required consent types for this user type
    let required_types = sqlx::query_as::<_, ConsentType>(
        "SELECT * FROM consent_types 
         WHERE is_required = true 
         AND is_active = true
         AND $1 = ANY(applicable_user_types)",
    )
    .bind(&user_type)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    // Get user's consents
    let consents = sqlx::query_as::<_, ConsentRecord>(
        "SELECT * FROM consent_records 
         WHERE user_id = $1 
         ORDER BY created_at DESC",
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    // Convert to response format
    let consent_responses: Vec<ConsentRecordResponse> = consents
        .into_iter()
        .map(|c| {
            let data_categories: Vec<String> = serde_json::from_value(c.data_categories.clone())
                .unwrap_or_default();
            
            let is_expired = c.expires_at.map(|exp| exp < chrono::Utc::now()).unwrap_or(false);
            
            ConsentRecordResponse {
                id: c.id,
                user_id: c.user_id,
                user_type: c.user_type,
                consent_type: c.consent_type.clone(),
                consent_type_name: None, // Will be filled later
                purpose: c.purpose,
                data_categories,
                consent_status: c.consent_status,
                granted_at: c.granted_at,
                withdrawn_at: c.withdrawn_at,
                expires_at: c.expires_at,
                is_expired,
                is_required: false, // Will be filled later
                consent_method: c.consent_method,
                is_minor_consent: c.is_minor_consent,
                parent_guardian_name: c.parent_guardian_name,
                created_at: c.created_at,
            }
        })
        .collect();

    // Calculate compliance
    let granted_required_codes: Vec<String> = consent_responses
        .iter()
        .filter(|c| c.consent_status == "granted" && !c.is_expired)
        .map(|c| c.consent_type.clone())
        .collect();

    let required_codes: Vec<String> = required_types.iter().map(|t| t.code.clone()).collect();

    let missing_required: Vec<String> = required_codes
        .iter()
        .filter(|code| !granted_required_codes.contains(code))
        .cloned()
        .collect();

    let status = UserConsentStatus {
        user_id,
        user_type,
        total_required: required_codes.len() as i32,
        granted_required: granted_required_codes.len() as i32,
        is_compliant: missing_required.is_empty(),
        missing_required_consents: missing_required,
        consents: consent_responses,
    };

    Ok((StatusCode::OK, Json(status)))
}

/// Give consent (single or bulk)
/// POST /api/consent
pub async fn create_consent(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: Request,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let claims = req.extensions().get::<Claims>()
        .ok_or(AppError::AuthError("ไม่พบข้อมูลผู้ใช้".to_string()))?
        .clone();

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::BadRequest("Invalid user ID".to_string()))?;

    // Extract request body
    let (parts, body) = req.into_parts();
    let bytes = axum::body::to_bytes(body, usize::MAX).await
        .map_err(|e| {
            eprintln!("❌ Failed to read request body: {}", e);
            AppError::BadRequest("Invalid request body".to_string())
        })?;

    let payload: CreateConsentRequest = serde_json::from_slice(&bytes)
        .map_err(|e| {
            eprintln!("❌ Failed to parse request: {}", e);
            AppError::BadRequest("Invalid request format".to_string())
        })?;

    // Get user type
    let user_type: String = sqlx::query_scalar("SELECT user_type FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get user type: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;

    // Get consent type details
    let consent_type_data = sqlx::query_as::<_, ConsentType>(
        "SELECT * FROM consent_types WHERE code = $1 AND is_active = true",
    )
    .bind(&payload.consent_type)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?
    .ok_or(AppError::NotFound("ไม่พบประเภทความยินยอมนี้".to_string()))?;

    // Calculate expiration date
    let expires_at = consent_type_data.default_duration_days.map(|days| {
        chrono::Utc::now() + chrono::Duration::days(days as i64)
    });

    // Get IP and User Agent
    let ip_address = headers
        .get("x-forwarded-for")
        .or_else(|| headers.get("x-real-ip"))
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let user_agent = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    // Create consent record
    let granted_at = if payload.consent_status == "granted" {
        Some(chrono::Utc::now())
    } else {
        None
    };

    let consent_id = sqlx::query_scalar::<_, Uuid>(
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
    .bind(&consent_type_data.description.unwrap_or_default())
    .bind(serde_json::json!(["personal_info"])) // Default categories
    .bind(&payload.consent_status)
    .bind(granted_at)
    .bind(expires_at)
    .bind("web_form")
    .bind(ip_address)
    .bind(user_agent)
    .bind(&consent_type_data.consent_text_template)
    .bind(&consent_type_data.consent_version)
    .bind(payload.is_minor_consent.unwrap_or(false))
    .bind(&payload.parent_guardian_name)
    .bind(&payload.parent_relationship)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to create consent: {}", e);
        AppError::InternalServerError("ไม่สามารถบันทึกความยินยอมได้".to_string())
    })?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "message": "บันทึกความยินยอมสำเร็จ",
            "consent_id": consent_id
        })),
    ))
}

/// Withdraw consent
/// POST /api/consent/:id/withdraw
pub async fn withdraw_consent(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(consent_id): Path<Uuid>,
    req: Request,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let claims = req.extensions().get::<Claims>()
        .ok_or(AppError::AuthError("ไม่พบข้อมูลผู้ใช้".to_string()))?
        .clone();

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
             AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::BadRequest("Invalid user ID".to_string()))?;

    // Check if consent belongs to user
    let consent = sqlx::query_as::<_, ConsentRecord>(
        "SELECT * FROM consent_records WHERE id = $1 AND user_id = $2",
    )
    .bind(consent_id)
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?
    .ok_or(AppError::NotFound("ไม่พบความยินยอมนี้".to_string()))?;

    // Check if it's a required consent
    let is_required: bool = sqlx::query_scalar(
        "SELECT is_required FROM consent_types WHERE code = $1",
    )
    .bind(&consent.consent_type)
    .fetch_one(&pool)
    .await
    .unwrap_or(false);

    if is_required {
        return Err(AppError::BadRequest("ไม่สามารถถอนความยินยอมที่จำเป็นได้".to_string()));
    }

    // Withdraw consent
    sqlx::query(
        "UPDATE consent_records 
         SET consent_status = 'withdrawn', withdrawn_at = NOW(), updated_at = NOW()
         WHERE id = $1",
    )
    .bind(consent_id)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to withdraw consent: {}", e);
        AppError::InternalServerError("ไม่สามารถถอนความยินยอมได้".to_string())
    })?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "ถอนความยินยอมสำเร็จ"
        })),
    ))
}

/// Get consent summary (Admin only)
/// GET /api/consent/summary
pub async fn get_consent_summary(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;

    // Get statistics
    let total_users: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE status = 'active'")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    let total_consents: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM consent_records")
        .fetch_one(&pool)
        .await
        .unwrap_or(0);

    let granted: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM consent_records WHERE consent_status = 'granted'",
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    let denied: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM consent_records WHERE consent_status = 'denied'")
            .fetch_one(&pool)
            .await
            .unwrap_or(0);

    let withdrawn: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM consent_records WHERE consent_status = 'withdrawn'",
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    let pending: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM consent_records WHERE consent_status = 'pending'",
    )
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    let compliance_rate = if total_users > 0 {
        (granted as f64 / total_users as f64) * 100.0
    } else {
        0.0
    };

    let summary = ConsentSummary {
        total_users,
        total_consents,
        granted,
        denied,
        withdrawn,
        pending,
        compliance_rate,
    };

    Ok((StatusCode::OK, Json(summary)))
}
