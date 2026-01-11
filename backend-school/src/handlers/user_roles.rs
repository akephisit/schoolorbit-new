use crate::db::school_mapping::get_school_database_url;
use crate::middleware::permission::check_permission;
use crate::models::staff::*;
use crate::permissions::registry::codes;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use uuid::Uuid;

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
    let _user = match check_permission(&headers, &pool, codes::ROLES_READ_ALL).await {
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
    let _user = match check_permission(&headers, &pool, codes::ROLES_ASSIGN_ALL).await {
        Ok(u) => u,
        Err(response) => return response,
    };

    // ===================================================================
    // Validate: Role user_type must match User user_type
    // ===================================================================
    
    // Get user's user_type
    let user_type: Option<String> = match sqlx::query_scalar(
        "SELECT user_type FROM users WHERE id = $1"
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await
    {
        Ok(ut) => ut,
        Err(e) => {
            eprintln!("❌ Failed to fetch user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถตรวจสอบข้อมูลผู้ใช้ได้"
                })),
            )
                .into_response();
        }
    };

    let user_type = match user_type {
        Some(ut) => ut,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": "ไม่พบผู้ใช้"
                })),
            )
                .into_response();
        }
    };

    // Get role's user_type
    let role_user_type: Option<String> = match sqlx::query_scalar(
        "SELECT user_type FROM roles WHERE id = $1 AND is_active = true"
    )
    .bind(&payload.role_id)
    .fetch_optional(&pool)
    .await
    {
        Ok(rut) => rut,
        Err(e) => {
            eprintln!("❌ Failed to fetch role: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถตรวจสอบข้อมูลบทบาทได้"
                })),
            )
                .into_response();
        }
    };

    let role_user_type = match role_user_type {
        Some(rut) => rut,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": "ไม่พบบทบาทหรือบทบาทไม่ active"
                })),
            )
                .into_response();
        }
    };

    // Validate: user_type must match
    if user_type != role_user_type {
        eprintln!(
            "❌ Role assignment validation failed: user_type '{}' != role.user_type '{}'",
            user_type, role_user_type
        );
        return (
            StatusCode::BAD_REQUEST,
            Json(json!({
                "success": false,
                "error": format!(
                    "ไม่สามารถมอบหมายบทบาทนี้ได้: บทบาทนี้สำหรับ {} เท่านั้น",
                    match role_user_type.as_str() {
                        "staff" => "บุคลากร",
                        "student" => "นักเรียน",
                        "parent" => "ผู้ปกครอง",
                        _ => "ผู้ใช้อื่น"
                    }
                )
            })),
        )
            .into_response();
    }


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
    let _user = match check_permission(&headers, &pool, codes::ROLES_REMOVE_ALL).await {
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
    let _user = match check_permission(&headers, &pool, codes::ROLES_READ_ALL).await {
        Ok(u) => u,
        Err(response) => return response,
    };

    // Get all permissions from user's roles (normalized schema)
    let permissions: Vec<String> = match sqlx::query_scalar(
        "SELECT DISTINCT p.code
         FROM user_roles ur
         JOIN role_permissions rp ON ur.role_id = rp.role_id
         JOIN permissions p ON rp.permission_id = p.id
         WHERE ur.user_id = $1 
           AND ur.ended_at IS NULL
         ORDER BY p.code"
    )
    .bind(user_id)
    .fetch_all(&pool)
    .await
    {
        Ok(perms) => perms,
        Err(e) => {
            eprintln!("❌ Database error ({}",e);
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

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": permissions
        })),
    )
        .into_response()
}
