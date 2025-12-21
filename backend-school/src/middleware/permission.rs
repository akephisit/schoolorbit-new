use crate::models::auth::{Claims, User};
use crate::models::staff::UserPermissions;
use axum::{
    body::Body,
    extract::Request,
    http::{Response, StatusCode},
    middleware::Next,
    response::IntoResponse,
    Json,
};
use serde_json::json;

/// Permission middleware - check if user has required permission
/// NOTE: This is the OLD hardcoded version - kept for backward compatibility
/// Use check_user_permission_db() for database-driven checks
pub async fn require_permission(
    permission: &'static str,
) -> impl Fn(Request, Next) -> std::pin::Pin<Box<dyn std::future::Future<Output = Response<Body>> + Send>> + Clone {
    move |req: Request, next: Next| {
        Box::pin(async move {
            // Get claims from request extensions
            let claims = req.extensions().get::<Claims>().cloned();

            if let Some(claims) = claims {
                // Check if user has the required permission
                // For now, simple role-based check
                // TODO: Fetch actual permissions from database
                
                let has_permission = match permission {
                    // Admin has all permissions
                    _ if claims.user_type.as_str() == "admin" => true,
                    
                    // Staff management permissions
                    "staff.view" => ["admin", "director", "secretary"].contains(&claims.user_type.as_str()),
                    "staff.create" => ["admin", "director"].contains(&claims.user_type.as_str()),
                    "staff.edit" => ["admin", "director"].contains(&claims.user_type.as_str()),
                    "staff.delete" => ["admin", "director"].contains(&claims.user_type.as_str()),
                    
                    // Role management permissions
                    "roles.view" => ["admin", "director"].contains(&claims.user_type.as_str()),
                    "roles.manage" => ["admin"].contains(&claims.user_type.as_str()),
                    
                    // Department management permissions
                    "departments.view" => ["admin", "director", "dept_head"].contains(&claims.user_type.as_str()),
                    "departments.manage" => ["admin", "director"].contains(&claims.user_type.as_str()),
                    
                    _ => false,
                };

                if has_permission {
                    next.run(req).await
                } else {
                    (
                        StatusCode::FORBIDDEN,
                        Json(json!({
                            "success": false,
                            "error": "คุณไม่มีสิทธิ์เข้าถึงฟีเจอร์นี้"
                        })),
                    )
                        .into_response()
                }
            } else {
                (
                    StatusCode::UNAUTHORIZED,
                    Json(json!({
                        "success": false,
                        "error": "กรุณาเข้าสู่ระบบ"
                    })),
                )
                    .into_response()
            }
        })
    }
}

/// Helper function to check permission from database
/// This will be called from handlers to get actual permissions
pub async fn check_user_permissions(
    pool: &sqlx::PgPool,
    user_id: uuid::Uuid,
) -> Result<Vec<String>, sqlx::Error> {
    // Query to get all permissions from user's roles
    let permissions: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT p.code
         FROM user_roles ur
         JOIN roles r ON ur.role_id = r.id
         JOIN permissions p ON p.code = ANY(r.permissions::text[])
         WHERE ur.user_id = $1 AND ur.ended_at IS NULL AND r.is_active = true",
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(permissions)
}

/// Check if user has specific permission
pub async fn has_permission(
    pool: &sqlx::PgPool,
    user_id: uuid::Uuid,
    permission: &str,
) -> Result<bool, sqlx::Error> {
    let permissions = check_user_permissions(pool, user_id).await?;
    Ok(permissions.contains(&permission.to_string()))
}

/// NEW: Database-driven permission check using UserPermissions trait
/// This is the RECOMMENDED way to check permissions
pub async fn check_user_permission_db(
    user: &User,
    pool: &sqlx::PgPool,
    permission: &str,
) -> Result<bool, sqlx::Error> {
    // Use the UserPermissions trait
    user.has_permission(pool, permission).await
}
