use axum::{
    extract::State,
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde_json::json;

use crate::AppState;
use crate::db::school_mapping::get_school_database_url;
use crate::error::AppError;
use crate::middleware::permission::check_permission;
use crate::permissions::registry::codes;
use crate::utils::subdomain::extract_subdomain_from_request;
use super::models::{SchoolSettings, UpdateSchoolSettingsRequest};

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain)
        .await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

/// GET /api/school/settings — staff only (SETTINGS_READ)
pub async fn get_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::SETTINGS_READ, &state.permission_cache).await {
        return Ok(r);
    }
    let settings = sqlx::query_as::<_, SchoolSettings>(
        "SELECT logo_url FROM school_settings LIMIT 1"
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("get_school_settings error: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })?
    .unwrap_or(SchoolSettings { logo_url: None });

    Ok(Json(json!({ "success": true, "data": settings })).into_response())
}

/// PATCH /api/school/settings — staff only (SETTINGS_UPDATE)
pub async fn update_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateSchoolSettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::SETTINGS_UPDATE, &state.permission_cache).await {
        return Ok(r);
    }
    sqlx::query(
        "UPDATE school_settings SET logo_url = $1, updated_at = NOW()"
    )
    .bind(&payload.logo_url)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("update_school_settings error: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })?;

    Ok(Json(json!({ "success": true })).into_response())
}

/// GET /api/school/public — no auth required
/// Returns logo_url (from local DB) + school_name (from backend-admin)
pub async fn get_public_info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_client, &subdomain)
        .await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))?;

    let settings = sqlx::query_as::<_, SchoolSettings>(
        "SELECT logo_url FROM school_settings LIMIT 1"
    )
    .fetch_optional(&pool)
    .await
    .unwrap_or(None)
    .unwrap_or(SchoolSettings { logo_url: None });

    let school_name = state.admin_client.get_school_name(&subdomain).await.ok();

    Ok(Json(json!({
        "success": true,
        "data": {
            "logoUrl": settings.logo_url,
            "schoolName": school_name,
        }
    })).into_response())
}
