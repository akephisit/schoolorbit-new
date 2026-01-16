use crate::modules::menu::models::FeatureToggle;
use crate::modules::auth::models::User;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::utils::jwt::JwtService;
use crate::utils::field_encryption;
use crate::AppState;
use crate::error::AppError;

use axum::{
    extract::{State, Path},
    http::{StatusCode, HeaderMap},
    response::{IntoResponse, Json as JsonResponse},
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

/// Request body for updating feature toggle
#[derive(Debug, Deserialize)]
pub struct UpdateFeatureRequest {
    pub is_enabled: Option<bool>,
}

/// Response for feature toggle operations
#[derive(Debug, Serialize)]
pub struct FeatureToggleResponse {
    pub success: bool,
    pub data: Option<FeatureToggle>,
    pub message: Option<String>,
}

/// Response for list of feature toggles
#[derive(Debug, Serialize)]
pub struct FeatureListResponse {
    pub success: bool,
    pub data: Vec<FeatureToggle>,
}

/// List all feature toggles (filtered by user permissions)
pub async fn list_features(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (pool, _, permissions) = get_pool_and_authenticate(&state, &headers).await?;

    let all_features = sqlx::query_as::<_, FeatureToggle>(
        "SELECT id, code, name, name_en, module, is_enabled
         FROM feature_toggles
         ORDER BY module, name"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to fetch features: {}", e)))?;

    // Filter features by user's module permissions
    let features: Vec<FeatureToggle> = all_features
        .into_iter()
        .filter(|f| {
            if let Some(ref module) = f.module {
                has_module_permission(&permissions, module)
            } else {
                true // No module restriction
            }
        })
        .collect();

    Ok((
        StatusCode::OK,
        JsonResponse(FeatureListResponse {
            success: true,
            data: features,
        })
    ))
}

/// Get single feature toggle
pub async fn get_feature(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (pool, _, permissions) = get_pool_and_authenticate(&state, &headers).await?;

    let feature = sqlx::query_as::<_, FeatureToggle>(
        "SELECT id, code, name, name_en, module, is_enabled
         FROM feature_toggles
         WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Database error: {}", e)))?
    .ok_or(AppError::NotFound("Feature toggle not found".to_string()))?;

    // Check module permission
    if let Some(ref module) = feature.module {
        if !has_module_permission(&permissions, module) {
            return Err(AppError::Forbidden(format!("No permission for module '{}'", module)));
        }
    }

    Ok((
        StatusCode::OK,
        JsonResponse(FeatureToggleResponse {
            success: true,
            data: Some(feature),
            message: None,
        })
    ))
}

/// Update feature toggle
pub async fn update_feature(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<UpdateFeatureRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (pool, _, permissions) = get_pool_and_authenticate(&state, &headers).await?;

    // First, get the feature to check its module
    let existing_feature = sqlx::query_as::<_, FeatureToggle>(
        "SELECT id, code, name, name_en, module, is_enabled FROM feature_toggles WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Database error: {}", e)))?
    .ok_or(AppError::NotFound("Feature toggle not found".to_string()))?;

    // Check module permission
    if let Some(ref module) = existing_feature.module {
        if !has_module_permission(&permissions, module) {
            return Err(AppError::Forbidden(format!("No permission for module '{}'", module)));
        }
    }

    // Build dynamic update query
    let mut updates = vec![];
    let mut query = String::from("UPDATE feature_toggles SET updated_at = NOW()");
    
    if let Some(enabled) = data.is_enabled {
        updates.push(format!("is_enabled = {}", enabled));
    }
    
    if !updates.is_empty() {
        query.push_str(", ");
        query.push_str(&updates.join(", "));
    }
    
    query.push_str(" WHERE id = $1 RETURNING id, code, name, name_en, module, is_enabled");

    let feature = sqlx::query_as::<_, FeatureToggle>(&query)
        .bind(id)
        .fetch_optional(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to update feature: {}", e)))?
        .ok_or(AppError::NotFound("Feature toggle not found".to_string()))?;

    Ok((
        StatusCode::OK,
        JsonResponse(FeatureToggleResponse {
            success: true,
            data: Some(feature),
            message: Some("Feature toggle updated successfully".to_string()),
        })
    ))
}

/// Quick toggle feature on/off
pub async fn toggle_feature(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (pool, _, permissions) = get_pool_and_authenticate(&state, &headers).await?;

    // First, get the feature to check its module
    let existing_feature = sqlx::query_as::<_, FeatureToggle>(
        "SELECT id, code, name, name_en, module, is_enabled FROM feature_toggles WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Database error: {}", e)))?
    .ok_or(AppError::NotFound("Feature toggle not found".to_string()))?;

    // Check module permission
    if let Some(ref module) = existing_feature.module {
        if !has_module_permission(&permissions, module) {
            return Err(AppError::Forbidden(format!("No permission for module '{}'", module)));
        }
    }

    // Toggle the feature (flip is_enabled)
    let feature = sqlx::query_as::<_, FeatureToggle>(
        "UPDATE feature_toggles 
         SET is_enabled = NOT is_enabled, updated_at = NOW()
         WHERE id = $1
         RETURNING id, code, name, name_en, module, is_enabled"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to toggle feature: {}", e)))?
    .ok_or(AppError::NotFound("Feature toggle not found".to_string()))?;

    // Extract values before moving feature
    let feature_code = feature.code.clone();
    let feature_enabled = feature.is_enabled;

    Ok((
        StatusCode::OK,
        JsonResponse(FeatureToggleResponse {
            success: true,
            data: Some(feature),
            message: Some(format!("Feature {} {}", 
                feature_code,
                if feature_enabled { "enabled" } else { "disabled" }
            )),
        })
    ))
}

// ==================== Helper Functions ====================

/// Helper: Check if user has ANY permission in the specified module
fn has_module_permission(user_permissions: &[String], module: &str) -> bool {
    if module.is_empty() {
        return true; // No permission required
    }
    
    let prefix = format!("{}.", module);
    user_permissions.iter().any(|perm| {
        perm.starts_with(&prefix) || perm.starts_with("*.")
    })
}

/// Helper: Authenticate user and get their permissions
async fn authenticate_user(
    headers: &HeaderMap,
    pool: &PgPool,
) -> Result<(User, Vec<String>), AppError> {
    // Try to extract token from Authorization header first
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok());
    
    let token_from_header = auth_header
        .and_then(|h| {
            if h.starts_with("Bearer ") {
                Some(h[7..].to_string())
            } else {
                None
            }
        });

    // Fallback to cookie
    let token_from_cookie = headers
        .get("Cookie")
        .and_then(|h| h.to_str().ok())
        .and_then(|cookie| JwtService::extract_token_from_cookie(Some(cookie)));

    // Use Authorization header first, then cookie
    let token = token_from_header.or(token_from_cookie)
        .ok_or(AppError::AuthError("No authentication token found".to_string()))?;

    // Validate token and extract claims
    let claims = JwtService::verify_token(&token)
        .map_err(|_| AppError::AuthError("Invalid or expired token".to_string()))?;

    // Get user from database
    let user_id = Uuid::parse_str(&claims.sub)
         .map_err(|_| AppError::AuthError("Invalid user ID in token".to_string()))?;

    let mut user = sqlx::query_as::<_, User>(
        "SELECT 
            id,
            username,
            national_id,
            email,
            password_hash,
            first_name,
            last_name,
            user_type,
            phone,
            date_of_birth,
            address,
            status,
            metadata,
            created_at,
            updated_at,
            title,
            nickname,
            emergency_contact,
            line_id,
            gender,
            profile_image_url,
            hired_date,
            resigned_date
         FROM users 
         WHERE id = $1"
    )
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Database error: {}", e)))?
        .ok_or(AppError::AuthError("User not found".to_string()))?;

    // Decrypt national_id
    if let Some(ref nid) = user.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            user.national_id = Some(dec);
        }
    }

    // Get user permissions
    let permissions: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT p.code 
         FROM user_roles ur
         JOIN roles r ON ur.role_id = r.id
         JOIN role_permissions rp ON r.id = rp.role_id
         JOIN permissions p ON rp.permission_id = p.id
         WHERE ur.user_id = $1 
           AND ur.ended_at IS NULL
           AND r.is_active = true"
    )
    .bind(user.id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to fetch permissions: {}", e)))?;

    Ok((user, permissions))
}

/// Helper: Get pool and authenticate (for list_features which filters by modules)
async fn get_pool_and_authenticate(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(PgPool, User, Vec<String>), AppError> {
    // Extract subdomain
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;


    // Get database URL
    let db_url = crate::db::school_mapping::get_school_database_url(&state.admin_pool, &subdomain).await
        .map_err(|e| AppError::NotFound(format!("School not found: {}", e)))?;

    // Get pool
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
         .map_err(|e| AppError::InternalServerError(format!("Database error: {}", e)))?;

    // Get user and permissions
    let (user, permissions) = authenticate_user(headers, &pool).await?;

    Ok((pool, user, permissions))
}
