use crate::models::auth::User;
use crate::models::staff::UserPermissions;
use axum::{
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Shared permission check function
/// Extracts token from headers, verifies user, and checks permission
/// 
/// This is the centralized permission check used by all handlers
/// Returns User if authorized, or Response with appropriate error
pub async fn check_permission(
    headers: &HeaderMap,
    pool: &sqlx::PgPool,
    required_permission: &str,
) -> Result<User, Response> {
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
