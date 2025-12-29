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
// List Roles
// ===================================================================

pub async fn list_roles(
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
    let _user = match check_permission(&headers, &pool, "roles.read.all").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    let roles = match sqlx::query_as::<_, Role>(
        "SELECT * FROM roles WHERE is_active = true ORDER BY level DESC, name"
    )
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
// Get Role by ID
// ===================================================================

pub async fn get_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(role_id): Path<Uuid>,
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

    let role = match sqlx::query_as::<_, Role>("SELECT * FROM roles WHERE id = $1")
        .bind(role_id)
        .fetch_optional(&pool)
        .await
    {
        Ok(Some(role)) => role,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": "ไม่พบบทบาท"
                })),
            )
                .into_response();
        }
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

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": role
        })),
    )
        .into_response()
}

// ===================================================================
// Create Role
// ===================================================================

pub async fn create_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateRoleRequest>,
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
    let _user = match check_permission(&headers, &pool, "roles.create.all").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    // Use Vec<String> directly (no JSON conversion needed)
    let permissions = payload.permissions.unwrap_or_default();

    let role_id: Uuid = match sqlx::query_scalar(
        "INSERT INTO roles (code, name, name_en, description, category, level, permissions)
         VALUES ($1, $2, $3, $4, $5, $6, $7)
         RETURNING id",
    )
    .bind(&payload.code)
    .bind(&payload.name)
    .bind(&payload.name_en)
    .bind(&payload.description)
    .bind(&payload.category)
    .bind(payload.level.unwrap_or(0))
    .bind(&permissions)
    .fetch_one(&pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            eprintln!("❌ Failed to create role: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถสร้างบทบาทได้"
                })),
            )
                .into_response();
        }
    };

    (
        StatusCode::CREATED,
        Json(json!({
            "success": true,
            "message": "สร้างบทบาทสำเร็จ",
            "data": {
                "id": role_id
            }
        })),
    )
        .into_response()
}

// ===================================================================
// Update Role
// ===================================================================

pub async fn update_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(role_id): Path<Uuid>,
    Json(payload): Json<UpdateRoleRequest>,
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
    let _user = match check_permission(&headers, &pool, "roles.update.all").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    // Use Vec<String> directly (no JSON conversion needed)
    let permissions = payload.permissions.as_ref();

    let result = sqlx::query(
        "UPDATE roles 
         SET 
            name = COALESCE($2, name),
            name_en = COALESCE($3, name_en),
            description = COALESCE($4, description),
            category = COALESCE($5, category),
            level = COALESCE($6, level),
            permissions = COALESCE($7, permissions),
            is_active = COALESCE($8, is_active),
            updated_at = NOW()
         WHERE id = $1",
    )
    .bind(role_id)
    .bind(&payload.name)
    .bind(&payload.name_en)
    .bind(&payload.description)
    .bind(&payload.category)
    .bind(&payload.level)
    .bind(&permissions)
    .bind(&payload.is_active)
    .execute(&pool)
    .await;

    match result {
        Ok(result) if result.rows_affected() > 0 => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "อัปเดตบทบาทสำเร็จ"
            })),
        )
            .into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": "ไม่พบบทบาท"
            })),
        )
            .into_response(),
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการอัปเดตบทบาท"
                })),
            )
                .into_response()
        }
    }
}

// ===================================================================
// List Departments
// ===================================================================

pub async fn list_departments(
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
    let _user = match check_permission(&headers, &pool, "roles.read.all").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    let departments = match sqlx::query_as::<_, Department>(
        "SELECT * FROM departments WHERE is_active = true ORDER BY display_order, name"
    )
    .fetch_all(&pool)
    .await
    {
        Ok(depts) => depts,
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
            "data": departments
        })),
    )
        .into_response()
}

// ===================================================================
// Get Department by ID
// ===================================================================

pub async fn get_department(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(dept_id): Path<Uuid>,
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

    let department = match sqlx::query_as::<_, Department>(
        "SELECT * FROM departments WHERE id = $1"
    )
    .bind(dept_id)
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(dept)) => dept,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": "ไม่พบฝ่าย"
                })),
            )
                .into_response();
        }
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

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": department
        })),
    )
        .into_response()
}

// ===================================================================
// Create Department
// ===================================================================

pub async fn create_department(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateDepartmentRequest>,
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
    let _user = match check_permission(&headers, &pool, "roles.create.all").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    let dept_id: Uuid = match sqlx::query_scalar(
        "INSERT INTO departments (code, name, name_en, description, parent_department_id, phone, email, location)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
         RETURNING id",
    )
    .bind(&payload.code)
    .bind(&payload.name)
    .bind(&payload.name_en)
    .bind(&payload.description)
    .bind(&payload.parent_department_id)
    .bind(&payload.phone)
    .bind(&payload.email)
    .bind(&payload.location)
    .fetch_one(&pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            eprintln!("❌ Failed to create department: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถสร้างฝ่ายได้"
                })),
            )
                .into_response();
        }
    };

    (
        StatusCode::CREATED,
        Json(json!({
            "success": true,
            "message": "สร้างฝ่ายสำเร็จ",
            "data": {
                "id": dept_id
            }
        })),
    )
        .into_response()
}

// ===================================================================
// Update Department
// ===================================================================

pub async fn update_department(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(dept_id): Path<Uuid>,
    Json(payload): Json<UpdateDepartmentRequest>,
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
    let _user = match check_permission(&headers, &pool, "roles.update.all").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    let result = sqlx::query(
        "UPDATE departments 
         SET 
            name = COALESCE($2, name),
            name_en = COALESCE($3, name_en),
            description = COALESCE($4, description),
            parent_department_id = COALESCE($5, parent_department_id),
            phone = COALESCE($6, phone),
            email = COALESCE($7, email),
            location = COALESCE($8, location),
            is_active = COALESCE($9, is_active),
            updated_at = NOW()
         WHERE id = $1",
    )
    .bind(dept_id)
    .bind(&payload.name)
    .bind(&payload.name_en)
    .bind(&payload.description)
    .bind(&payload.parent_department_id)
    .bind(&payload.phone)
    .bind(&payload.email)
    .bind(&payload.location)
    .bind(&payload.is_active)
    .execute(&pool)
    .await;

    match result {
        Ok(result) if result.rows_affected() > 0 => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "อัปเดตฝ่ายสำเร็จ"
            })),
        )
            .into_response(),
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": "ไม่พบฝ่าย"
            })),
        )
            .into_response(),
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการอัปเดตฝ่าย"
                })),
            )
                .into_response()
        }
    }
}
