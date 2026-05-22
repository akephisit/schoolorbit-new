use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json as JsonResponse},
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::auth::models::User;
use crate::modules::menu::models::FeatureToggle;
use crate::modules::system::services::feature_toggle_service;
use crate::utils::field_encryption;
use crate::utils::jwt::JwtService;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct UpdateFeatureRequest {
    pub is_enabled: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct FeatureToggleResponse {
    pub success: bool,
    pub data: Option<FeatureToggle>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct FeatureListResponse {
    pub success: bool,
    pub data: Vec<FeatureToggle>,
}

pub async fn list_features(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (pool, _, permissions) = get_pool_and_authenticate(&state, &headers).await?;

    let all_features = feature_toggle_service::list_features(&pool).await?;
    let features: Vec<FeatureToggle> = all_features.into_iter()
        .filter(|f| match f.module {
            Some(ref module) => has_module_permission(&permissions, module),
            None => true,
        })
        .collect();

    Ok((StatusCode::OK, JsonResponse(FeatureListResponse { success: true, data: features })))
}

pub async fn get_feature(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (pool, _, permissions) = get_pool_and_authenticate(&state, &headers).await?;
    let feature = feature_toggle_service::get_feature(&pool, id).await?;

    if let Some(ref module) = feature.module {
        if !has_module_permission(&permissions, module) {
            return Err(AppError::Forbidden(format!("No permission for module '{}'", module)));
        }
    }

    Ok((StatusCode::OK, JsonResponse(FeatureToggleResponse {
        success: true, data: Some(feature), message: None,
    })))
}

pub async fn update_feature(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<UpdateFeatureRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (pool, _, permissions) = get_pool_and_authenticate(&state, &headers).await?;
    let existing = feature_toggle_service::get_feature(&pool, id).await?;

    if let Some(ref module) = existing.module {
        if !has_module_permission(&permissions, module) {
            return Err(AppError::Forbidden(format!("No permission for module '{}'", module)));
        }
    }

    let feature = feature_toggle_service::update_feature(&pool, id, data.is_enabled).await?;
    Ok((StatusCode::OK, JsonResponse(FeatureToggleResponse {
        success: true,
        data: Some(feature),
        message: Some("Feature toggle updated successfully".to_string()),
    })))
}

pub async fn toggle_feature(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (pool, _, permissions) = get_pool_and_authenticate(&state, &headers).await?;
    let existing = feature_toggle_service::get_feature(&pool, id).await?;

    if let Some(ref module) = existing.module {
        if !has_module_permission(&permissions, module) {
            return Err(AppError::Forbidden(format!("No permission for module '{}'", module)));
        }
    }

    let feature = feature_toggle_service::toggle_feature(&pool, id).await?;
    let feature_code = feature.code.clone();
    let feature_enabled = feature.is_enabled;

    Ok((StatusCode::OK, JsonResponse(FeatureToggleResponse {
        success: true,
        data: Some(feature),
        message: Some(format!("Feature {} {}",
            feature_code,
            if feature_enabled { "enabled" } else { "disabled" })),
    })))
}

fn has_module_permission(user_permissions: &[String], module: &str) -> bool {
    if module.is_empty() { return true; }
    let prefix = format!("{}.", module);
    user_permissions.iter().any(|perm| perm.starts_with(&prefix) || perm.starts_with("*."))
}

async fn authenticate_user(headers: &HeaderMap, pool: &PgPool) -> Result<(User, Vec<String>), AppError> {
    let token_from_header = headers.get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer ").map(|t| t.to_string()));
    let token_from_cookie = headers.get("Cookie")
        .and_then(|h| h.to_str().ok())
        .and_then(|cookie| JwtService::extract_token_from_cookie(Some(cookie)));

    let token = token_from_header.or(token_from_cookie)
        .ok_or(AppError::AuthError("No authentication token found".to_string()))?;

    let claims = JwtService::verify_token(&token)
        .map_err(|_| AppError::AuthError("Invalid or expired token".to_string()))?;
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::AuthError("Invalid user ID in token".to_string()))?;

    let mut user = sqlx::query_as::<_, User>(
        "SELECT id, username, national_id, email, password_hash, first_name, last_name,
                user_type, phone, date_of_birth, address, status, metadata, created_at, updated_at,
                title, nickname, emergency_contact, line_id, gender, profile_image_url,
                hired_date, resigned_date
         FROM users WHERE id = $1"
    )
    .bind(user_id).fetch_optional(pool).await
    .map_err(|e| AppError::InternalServerError(format!("Database error: {}", e)))?
    .ok_or(AppError::AuthError("User not found".to_string()))?;

    if let Some(ref nid) = user.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            user.national_id = Some(dec);
        }
    }

    let permissions: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT p.code
         FROM user_roles ur
         JOIN roles r ON ur.role_id = r.id
         JOIN role_permissions rp ON r.id = rp.role_id
         JOIN permissions p ON rp.permission_id = p.id
         WHERE ur.user_id = $1 AND ur.ended_at IS NULL AND r.is_active = true"
    )
    .bind(user.id).fetch_all(pool).await
    .map_err(|e| AppError::InternalServerError(format!("Failed to fetch permissions: {}", e)))?;

    Ok((user, permissions))
}

async fn get_pool_and_authenticate(state: &AppState, headers: &HeaderMap) -> Result<(PgPool, User, Vec<String>), AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;
    let db_url = crate::db::school_mapping::get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|e| AppError::NotFound(format!("School not found: {}", e)))?;
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|e| AppError::InternalServerError(format!("Database error: {}", e)))?;
    let (user, permissions) = authenticate_user(headers, &pool).await?;
    Ok((pool, user, permissions))
}
