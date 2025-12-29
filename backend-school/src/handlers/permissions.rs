use crate::db::school_mapping::get_school_database_url;
use crate::models::auth::User;
use crate::models::staff::Permission;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use std::collections::HashMap;

// ===================================================================
// Helper Functions
// ===================================================================

/// Check user permission
async fn check_permission(
    headers: &HeaderMap,
    pool: &sqlx::PgPool,
    required_permission: &str,
) -> Result<User, Response> {
    use crate::models::staff::UserPermissions;
    
    // Try to extract token from Authorization header first
    let auth_header = headers
        .get(header::AUTHORIZATION)
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
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .and_then(|cookie| crate::utils::jwt::JwtService::extract_token_from_cookie(Some(cookie)));

    // Use Authorization header first, then cookie
    let token = match token_from_header.or(token_from_cookie) {
        Some(t) => t,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "error": "กรุณาเข้าสู่ระบบ"
                })),
            ).into_response());
        }
    };

    // Verify token
    let claims = match crate::utils::jwt::JwtService::verify_token(&token) {
        Ok(c) => c,
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "error": "Token ไม่ถูกต้อง"
                })),
            ).into_response());
        }
    };
    
    // Get user from database
    let user: User = match sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(uuid::Uuid::parse_str(&claims.sub).unwrap())
        .fetch_one(pool)
        .await
    {
        Ok(u) => u,
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "error": "ไม่พบข้อมูลผู้ใช้"
                })),
            ).into_response());
        }
    };

    // Check permission
    match user.has_permission(pool, required_permission).await {
        Ok(true) => Ok(user),
        Ok(false) => Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "success": false,
                "error": format!("ไม่มีสิทธิ์ {}", required_permission)
            })),
        ).into_response()),
        Err(e) => {
            eprintln!("❌ Failed to check permission: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถตรวจสอบสิทธิ์ได้"
                })),
            ).into_response())
        }
    }
}

// ===================================================================
// List All Permissions
// ===================================================================

pub async fn list_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let subdomain = match extract_subdomain_from_request(&headers) {
        Ok(s) => s,
        Err(response) => return response,
    };

    let db_url = match get_school_database_url(&state.admin_pool, &subdomain).await {
        Ok(url) => url,
        Err(e) => {
            eprintln!("❌ Failed to get school database: {}", e);
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": "ไม่พบโรงเรียน"
                })),
            )
                .into_response();
        }
    };

    let pool = match state.pool_manager.get_pool(&db_url, &subdomain).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ Failed to get database pool: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถเชื่อมต่อฐานข้อมูลได้"
                })),
            )
                .into_response();
        }
    };

    // Check permission
    let _user = match check_permission(&headers, &pool, "settings.read").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    let permissions = match sqlx::query_as::<_, Permission>(
        "SELECT * FROM permissions ORDER BY module, action, code"
    )
    .fetch_all(&pool)
    .await
    {
        Ok(perms) => perms,
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการดึงข้อมูล"
                })), 
            )
                .into_response();
        }
    };

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": permissions
        })),
    )
        .into_response()
}

// ===================================================================
// List Permissions Grouped by Module
// ===================================================================

pub async fn list_permissions_by_module(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let subdomain = match extract_subdomain_from_request(&headers) {
        Ok(s) => s,
        Err(response) => return response,
    };

    let db_url = match get_school_database_url(&state.admin_pool, &subdomain).await {
        Ok(url) => url,
        Err(e) => {
            eprintln!("❌ Failed to get school database: {}", e);
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": "ไม่พบโรงเรียน"
                })),
            )
                .into_response();
        }
    };

    let pool = match state.pool_manager.get_pool(&db_url, &subdomain).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ Failed to get database pool: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถเชื่อมต่อฐานข้อมูลได้"
                })),
            )
                .into_response();
        }
    };

    // Check permission
    let _user = match check_permission(&headers, &pool, "settings.read").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    let permissions = match sqlx::query_as::<_, Permission>(
        "SELECT * FROM permissions ORDER BY module, action, code"
    )
    .fetch_all(&pool)
    .await
    {
        Ok(perms) => perms,
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการดึงข้อมูล"
                })),
            )
                .into_response();
        }
    };

    // Group permissions by module
    let mut grouped: HashMap<String, Vec<Permission>> = HashMap::new();
    for perm in permissions {
        grouped
            .entry(perm.module.clone())
            .or_insert_with(Vec::new)
            .push(perm);
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": grouped
        })),
    )
        .into_response()
}
