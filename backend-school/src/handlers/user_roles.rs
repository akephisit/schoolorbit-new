use crate::db::school_mapping::get_school_database_url;
use crate::models::auth::User;
use crate::models::staff::*;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use uuid::Uuid;

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
// Get User Roles
// ===================================================================

pub async fn get_user_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
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
    let _user = match check_permission(&headers, &pool, "roles.read.all").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    let roles = match sqlx::query_as::<_, Role>(
        "SELECT r.* FROM roles r
         JOIN user_roles ur ON ur.role_id = r.id
         WHERE ur.user_id = $1 
           AND ur.ended_at IS NULL
           AND r.is_active = true
         ORDER BY ur.is_primary DESC, r.level DESC"
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    {
        Ok(roles) => roles,
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
            "data": roles
        })),
    )
        .into_response()
}

// ===================================================================
// Assign Role to User
// ===================================================================

pub async fn assign_user_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<AssignRoleRequest>,
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
                    "error": "ไม่สามารถ่อมต่อฐานข้อมูลได้"
                })),
            )
                .into_response();
        }
    };

    // Check permission
    let _user = match check_permission(&headers, &pool, "roles.assign.all").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    let user_role_id: Uuid = match sqlx::query_scalar(
        "INSERT INTO user_roles (user_id, role_id, is_primary, started_at, notes)
         VALUES ($1, $2, $3, $4, $5)
         RETURNING id"
    )
    .bind(user_id)
    .bind(&payload.role_id)
    .bind(payload.is_primary.unwrap_or(false))
    .bind(payload.started_at.unwrap_or_else(|| chrono::Utc::now().naive_utc().date()))
    .bind(&payload.notes)
    .fetch_one(&pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            eprintln!("❌ Failed to assign role: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถมอบหมายบทบาทได้"
                })),
            )
                .into_response();
        }
    };

    (
        StatusCode::CREATED,
        Json(json!({
            "success": true,
            "message": "มอบหมายบทบาทสำเร็จ",
            "data": {
                "id": user_role_id
            }
        })),
    )
        .into_response()
}

// ===================================================================
// Remove Role from User
// ===================================================================

pub async fn remove_user_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, role_id)): Path<(Uuid, Uuid)>,
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
    let _user = match check_permission(&headers, &pool, "roles.remove.all").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    // Soft delete by setting ended_at
    let result = sqlx::query(
        "UPDATE user_roles 
         SET ended_at = CURRENT_DATE, updated_at = NOW()
         WHERE user_id = $1 AND role_id = $2 AND ended_at IS NULL"
    )
    .bind(user_id)
    .bind(role_id)
    .execute(&pool)
    .await;

    match result {
        Ok(result) if result.rows_affected() > 0 => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "ลบบทบาทสำเร็จ"
            })),
        )
            .into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": "ไม่พบการมอบหมายบทบาท"
            })),
        )
            .into_response(),
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาด"
                })),
            )
                .into_response()
        }
    }
}

// ===================================================================
// Get User Permissions
// ===================================================================

pub async fn get_user_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
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
    let _user = match check_permission(&headers, &pool, "roles.read.all").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    // Get all permissions from user's roles
    let rows: Vec<(Vec<String>,)> = match sqlx::query_as(
        "SELECT r.permissions
         FROM user_roles ur
         JOIN roles r ON ur.role_id = r.id
         WHERE ur.user_id = $1 
           AND ur.ended_at IS NULL
           AND r.is_active = true"
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    {
        Ok(rows) => rows,
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาด"
                })),
            )
                .into_response();
        }
    };

    // Flatten permission arrays (deduplicated)
    let mut permissions = Vec::new();
    for (perms,) in rows {
        for perm in perms {
            if !permissions.contains(&perm) {
                permissions.push(perm);
            }
        }
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": permissions
        })),
    )
        .into_response()
}
