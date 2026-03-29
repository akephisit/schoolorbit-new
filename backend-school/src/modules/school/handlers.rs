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
use crate::utils::file_url::get_file_url_from_string;
use crate::utils::subdomain::extract_subdomain_from_request;
use super::models::{SchoolSettingsRow, SchoolSettingsResponse, UpdateSchoolSettingsRequest};
use crate::services::r2_client::R2Client;

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
    let row = sqlx::query_as::<_, SchoolSettingsRow>(
        "SELECT logo_path, logo_file_id FROM school_settings LIMIT 1"
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("get_school_settings error: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })?
    .unwrap_or(SchoolSettingsRow { logo_path: None, logo_file_id: None });

    let response = SchoolSettingsResponse {
        logo_url: get_file_url_from_string(&row.logo_path),
        logo_file_id: row.logo_file_id,
    };

    Ok(Json(json!({ "success": true, "data": response })).into_response())
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
        "UPDATE school_settings SET logo_path = $1, logo_file_id = $2, updated_at = NOW()"
    )
    .bind(&payload.logo_path)
    .bind(&payload.logo_file_id)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("update_school_settings error: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })?;

    Ok(Json(json!({ "success": true })).into_response())
}

/// GET /api/school/public — no auth required
/// Returns logoUrl (built from logo_path) + schoolName (from backend-admin)
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

    let row = sqlx::query_as::<_, SchoolSettingsRow>(
        "SELECT logo_path, logo_file_id FROM school_settings LIMIT 1"
    )
    .fetch_optional(&pool)
    .await
    .unwrap_or(None)
    .unwrap_or(SchoolSettingsRow { logo_path: None, logo_file_id: None });

    let logo_url = get_file_url_from_string(&row.logo_path);
    let school_name = state.admin_client.get_school_name(&subdomain).await.ok();

    Ok(Json(json!({
        "success": true,
        "data": {
            "logoUrl": logo_url,
            "schoolName": school_name,
        }
    })).into_response())
}

/// DELETE /api/school/settings/logo — staff only (SETTINGS_UPDATE)
/// ลบ logo จาก R2 และล้าง logo_path/logo_file_id ใน school_settings
pub async fn delete_logo(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::SETTINGS_UPDATE, &state.permission_cache).await {
        return Ok(r);
    }

    let row = sqlx::query_as::<_, SchoolSettingsRow>(
        "SELECT logo_path, logo_file_id FROM school_settings LIMIT 1"
    )
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("delete_logo fetch error: {}", e);
        AppError::InternalServerError("Database error".to_string())
    })?
    .unwrap_or(SchoolSettingsRow { logo_path: None, logo_file_id: None });

    // ลบไฟล์จาก R2
    if let Some(path) = &row.logo_path {
        match R2Client::new().await {
            Ok(r2) => {
                if let Err(e) = r2.delete_file(path).await {
                    eprintln!("Failed to delete logo from R2: {}", e);
                }
            }
            Err(e) => eprintln!("R2Client init error: {}", e),
        }
    }

    // Hard-delete record ใน files table
    if let Some(file_id) = row.logo_file_id {
        let _ = sqlx::query("DELETE FROM files WHERE id = $1")
            .bind(file_id)
            .execute(&pool)
            .await;
    }

    // ล้างใน school_settings
    sqlx::query("UPDATE school_settings SET logo_path = NULL, logo_file_id = NULL, updated_at = NOW()")
        .execute(&pool)
        .await
        .map_err(|e| {
            eprintln!("delete_logo clear error: {}", e);
            AppError::InternalServerError("Database error".to_string())
        })?;

    Ok(Json(json!({ "success": true })).into_response())
}
