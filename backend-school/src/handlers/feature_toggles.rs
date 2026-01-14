use crate::models::menu::FeatureToggle;
use crate::modules::auth::models::User;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::utils::jwt::JwtService;
use crate::utils::field_encryption;
use crate::AppState;

use axum::{
    extract::{State, Path},
    http::{StatusCode, HeaderMap},
    response::{Response, IntoResponse, Json as JsonResponse},
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
) -> Response {
    let (pool, _, permissions) = match get_pool_and_authenticate(&state, &headers).await {
        Ok(result) => result,
        Err(response) => return response,
    };

    let all_features = match sqlx::query_as::<_, FeatureToggle>(
        "SELECT id, code, name, name_en, module, is_enabled
         FROM feature_toggles
         ORDER BY module, name"
    )
    .fetch_all(&pool)
    .await
    {
        Ok(f) => f,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": format!("Failed to fetch features: {}", e)
                }))
            ).into_response();
        }
    };

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

    (
        StatusCode::OK,
        JsonResponse(FeatureListResponse {
            success: true,
            data: features,
        })
    )
        .into_response()
}

/// Get single feature toggle
pub async fn get_feature(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    let (pool, _, permissions) = match get_pool_and_authenticate(&state, &headers).await {
        Ok(result) => result,
        Err(response) => return response,
    };

    let feature = match sqlx::query_as::<_, FeatureToggle>(
        "SELECT id, code, name, name_en, module, is_enabled
         FROM feature_toggles
         WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(f)) => {
            // Check module permission
            if let Some(ref module) = f.module {
                if !has_module_permission(&permissions, module) {
                    return (
                        StatusCode::FORBIDDEN,
                        JsonResponse(serde_json::json!({
                            "success": false,
                            "error": format!("No permission for module '{}'", module)
                        }))
                    ).into_response();
                }
            }
            f
        },
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": "Feature toggle not found"
                }))
            ).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": format!("Database error: {}", e)
                }))
            ).into_response();
        }
    };

    (
        StatusCode::OK,
        JsonResponse(FeatureToggleResponse {
            success: true,
            data: Some(feature),
            message: None,
        })
    )
        .into_response()
}

/// Update feature toggle
pub async fn update_feature(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<UpdateFeatureRequest>,
) -> Response {
    let (pool, _, permissions) = match get_pool_and_authenticate(&state, &headers).await {
        Ok(result) => result,
        Err(response) => return response,
    };

    // First, get the feature to check its module
    let existing_feature = match sqlx::query_as::<_, FeatureToggle>(
        "SELECT id, code, name, name_en, module, is_enabled FROM feature_toggles WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(f)) => f,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": "Feature toggle not found"
                }))
            ).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(serde_json::json!({"success": false, "error": format!("Database error: {}", e)}))
            ).into_response();
        }
    };

    // Check module permission
    if let Some(ref module) = existing_feature.module {
        if !has_module_permission(&permissions, module) {
            return (
                StatusCode::FORBIDDEN,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": format!("No permission for module '{}'", module)
                }))
            ).into_response();
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

    let feature = match sqlx::query_as::<_, FeatureToggle>(&query)
        .bind(id)
        .fetch_optional(&pool)
        .await
    {
        Ok(Some(f)) => f,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": "Feature toggle not found"
                }))
            ).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": format!("Failed to update feature: {}", e)
                }))
            ).into_response();
        }
    };

    (
        StatusCode::OK,
        JsonResponse(FeatureToggleResponse {
            success: true,
            data: Some(feature),
            message: Some("Feature toggle updated successfully".to_string()),
        })
    )
        .into_response()
}

/// Quick toggle feature on/off
pub async fn toggle_feature(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    let (pool, _, permissions) = match get_pool_and_authenticate(&state, &headers).await {
        Ok(result) => result,
        Err(response) => return response,
    };

    // First, get the feature to check its module
    let existing_feature = match sqlx::query_as::<_, FeatureToggle>(
        "SELECT id, code, name, name_en, module, is_enabled FROM feature_toggles WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(f)) => f,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": "Feature toggle not found"
                }))
            ).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(serde_json::json!({"success": false, "error": format!("Database error: {}", e)}))
            ).into_response();
        }
    };

    // Check module permission
    if let Some(ref module) = existing_feature.module {
        if !has_module_permission(&permissions, module) {
            return (
                StatusCode::FORBIDDEN,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": format!("No permission for module '{}'", module)
                }))
            ).into_response();
        }
    }

    // Toggle the feature (flip is_enabled)
    let feature = match sqlx::query_as::<_, FeatureToggle>(
        "UPDATE feature_toggles 
         SET is_enabled = NOT is_enabled, updated_at = NOW()
         WHERE id = $1
         RETURNING id, code, name, name_en, module, is_enabled"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(f)) => f,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": "Feature toggle not found"
                }))
            ).into_response();
        }
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": format!("Failed to toggle feature: {}", e)
                }))
            ).into_response();
        }
    };

    // Extract values before moving feature
    let feature_code = feature.code.clone();
    let feature_enabled = feature.is_enabled;

    (
        StatusCode::OK,
        JsonResponse(FeatureToggleResponse {
            success: true,
            data: Some(feature),
            message: Some(format!("Feature {} {}", 
                feature_code,
                if feature_enabled { "enabled" } else { "disabled" }
            )),
        })
    )
        .into_response()
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

/// Helper: Get pool and check module permission
async fn get_pool_and_check_module(
    state: &AppState,
    headers: &HeaderMap,
    module: &str,
) -> Result<(PgPool, User), Response> {
    // Extract subdomain
    let subdomain = match extract_subdomain_from_request(headers) {
        Ok(s) => s,
        Err(response) => return Err(response),
    };

    // Get database URL
    let db_url = match crate::db::school_mapping::get_school_database_url(&state.admin_pool, &subdomain).await {
        Ok(url) => url,
        Err(e) => {
            return Err((
                StatusCode::NOT_FOUND,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": format!("School not found: {}", e)
                }))
            ).into_response());
        }
    };

    // Get pool
    let pool = match state.pool_manager.get_pool(&db_url, &subdomain).await {
        Ok(p) => p,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": format!("Database error: {}", e)
                }))
            ).into_response());
        }
    };

    // Get user and permissions
    let (user, permissions) = match authenticate_user(headers, &pool).await {
        Ok(result) => result,
        Err(e) => return Err(e),
    };

    // Check module permission
    if !has_module_permission(&permissions, module) {
        return Err((
            StatusCode::FORBIDDEN,
            JsonResponse(serde_json::json!({
                "success": false,
                "error": format!("No permission for module '{}'", module)
            }))
        ).into_response());
    }

    Ok((pool, user))
}

/// Helper: Authenticate user and get their permissions
async fn authenticate_user(
    headers: &HeaderMap,
    pool: &PgPool,
) -> Result<(User, Vec<String>), Response> {
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
    let token = match token_from_header.or(token_from_cookie) {
        Some(t) => t,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": "No authentication token found"
                }))
            ).into_response());
        }
    };


    // Validate token and extract claims
    let claims = match JwtService::verify_token(&token) {
        Ok(c) => c,
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": "Invalid or expired token"
                }))
            ).into_response());
        }
    };

    // Get user from database
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": "Invalid user ID in token"
                }))
            ).into_response());
        }
    };

    let mut user = match sqlx::query_as::<_, User>(
        "SELECT 
            id,
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
    {
        Ok(Some(u)) => u,
        Ok(None) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": "User not found"
                }))
            ).into_response());
        }
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": format!("Database error: {}", e)
                }))
            ).into_response());
        }
    };

    // Decrypt national_id
    if let Some(ref nid) = user.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            user.national_id = Some(dec);
        }
    }

    // Get user permissions
    let permissions: Vec<String> = match sqlx::query_scalar(
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
    {
        Ok(p) => p,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": format!("Failed to fetch permissions: {}", e)
                }))
            ).into_response());
        }
    };

    Ok((user, permissions))
}

/// Helper: Get pool and authenticate (for list_features which filters by modules)
async fn get_pool_and_authenticate(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(PgPool, User, Vec<String>), Response> {
    // Extract subdomain
    let subdomain = match extract_subdomain_from_request(headers) {
        Ok(s) => s,
        Err(response) => return Err(response),
    };

    // Get database URL
    let db_url = match crate::db::school_mapping::get_school_database_url(&state.admin_pool, &subdomain).await {
        Ok(url) => url,
        Err(e) => {
            return Err((
                StatusCode::NOT_FOUND,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": format!("School not found: {}", e)
                }))
            ).into_response());
        }
    };

    // Get pool
    let pool = match state.pool_manager.get_pool(&db_url, &subdomain).await {
        Ok(p) => p,
        Err(e) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                JsonResponse(serde_json::json!({
                    "success": false,
                    "error": format!("Database error: {}", e)
                }))
            ).into_response());
        }
    };

    // Get user and permissions
    let (user, permissions) = match authenticate_user(headers, &pool).await {
        Ok(result) => result,
        Err(e) => return Err(e),
    };

    Ok((pool, user, permissions))
}
