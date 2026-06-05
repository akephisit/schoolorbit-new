use axum::{extract::State, http::HeaderMap, response::IntoResponse, Json};
use serde_json::json;

use super::models::UpdateSchoolSettingsRequest;
use super::services as school_service;
use crate::error::AppError;
use crate::middleware::permission::load_actor_context;
use crate::permissions::registry::codes;
use crate::utils::tenant::{resolve_tenant_context, resolve_tenant_pool};
use crate::AppState;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    resolve_tenant_pool(state, headers).await
}

/// GET /api/school/settings — staff only (SETTINGS_READ)
pub async fn get_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = load_actor_context(&headers, &pool, &state.permission_cache).await?;
    actor.require_permission(codes::SETTINGS_READ)?;

    let response = school_service::get_settings_response(&pool).await?;

    Ok(Json(json!({ "success": true, "data": response })).into_response())
}

/// PATCH /api/school/settings — staff only (SETTINGS_UPDATE)
pub async fn update_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateSchoolSettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = load_actor_context(&headers, &pool, &state.permission_cache).await?;
    actor.require_permission(codes::SETTINGS_UPDATE)?;

    school_service::update_settings(&pool, payload).await?;

    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

/// GET /api/school/public — no auth required
/// Returns logoUrl (built from logo_path) + schoolName (from backend-admin)
pub async fn get_public_info(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let tenant = resolve_tenant_context(&state, &headers).await?;
    let pool = tenant.pool;

    let logo_url = school_service::get_settings_response(&pool).await?.logo_url;
    let school_name = state
        .admin_client
        .get_school_name(&tenant.subdomain)
        .await
        .ok();

    Ok(Json(json!({ "success": true, "data": {
            "logoUrl": logo_url,
            "schoolName": school_name,
        } }))
    .into_response())
}

/// DELETE /api/school/settings/logo — staff only (SETTINGS_UPDATE)
/// ลบ logo จาก R2 และล้าง logo_path/logo_file_id ใน school_settings
pub async fn delete_logo(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = load_actor_context(&headers, &pool, &state.permission_cache).await?;
    actor.require_permission(codes::SETTINGS_UPDATE)?;

    school_service::delete_logo(&pool).await?;

    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}
