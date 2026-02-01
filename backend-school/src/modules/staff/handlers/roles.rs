use crate::db::school_mapping::get_school_database_url;
use crate::middleware::permission::check_permission;
use crate::modules::staff::models::*;
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
    let _user = match check_permission(&headers, &pool, codes::ROLES_READ_ALL).await {
        Ok(u) => u,
        Err(response) => return response,
    };

    // Use JOIN to get permissions
    let roles = match sqlx::query_as::<_, Role>(
        "SELECT r.*, 
                COALESCE(
                    array_agg(p.code) FILTER (WHERE p.code IS NOT NULL), 
                    '{}'
                ) as permissions 
         FROM roles r
         LEFT JOIN role_permissions rp ON r.id = rp.role_id
         LEFT JOIN permissions p ON rp.permission_id = p.id
         WHERE r.is_active = true 
         GROUP BY r.id
         ORDER BY r.level DESC, r.name"
    )
    .fetch_all(&pool)
    .await
    {
        Ok(roles) => roles,
        Err(e) => {
            eprintln!("❌ Database error (list_roles): {}", e);
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
    let _user = match check_permission(&headers, &pool, codes::ROLES_READ_ALL).await {
        Ok(u) => u,
        Err(response) => return response,
    };

    let role = match sqlx::query_as::<_, Role>(
        "SELECT r.*, 
                COALESCE(
                    array_agg(p.code) FILTER (WHERE p.code IS NOT NULL), 
                    '{}'
                ) as permissions 
         FROM roles r
         LEFT JOIN role_permissions rp ON r.id = rp.role_id
         LEFT JOIN permissions p ON rp.permission_id = p.id
         WHERE r.id = $1
         GROUP BY r.id"
    )
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
    let _user = match check_permission(&headers, &pool, codes::ROLES_CREATE_ALL).await {
        Ok(u) => u,
        Err(response) => return response,
    };


    // Use transaction for atomic operations
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            eprintln!("❌ Failed to start transaction: {}", e);
             return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถเริ่มต้น Transaction ได้"
                })),
            )
            .into_response();
        }
    };

    let role_id: Uuid = match sqlx::query_scalar(
        "INSERT INTO roles (code, name, name_en, description, user_type, level)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(&payload.code)
    .bind(&payload.name)
    .bind(&payload.name_en)
    .bind(&payload.description)
    .bind(&payload.user_type)
    .bind(payload.level.unwrap_or(0))
    .fetch_one(&mut *tx)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            eprintln!("❌ Failed to create role: {}", e);
            let _ = tx.rollback().await;
            
            let err_msg = e.to_string();
            if err_msg.contains("duplicate key value") {
                 if err_msg.contains("code") {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({
                            "success": false,
                            "error": "รหัสบทบาทนี้มีอยู่ในระบบแล้ว"
                        })),
                    ).into_response();
                }
            }

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

    // Insert permissions if provided
    if let Some(permissions) = &payload.permissions {
        if !permissions.is_empty() {
            // Find permission IDs from codes
            let perm_ids: Vec<Uuid> = match sqlx::query_scalar(
                "SELECT id FROM permissions WHERE code = ANY($1)"
            )
            .bind(permissions)
            .fetch_all(&mut *tx)
            .await 
            {
                Ok(ids) => ids,
                Err(e) => {
                     eprintln!("❌ Failed to find permissions: {}", e);
                     let _ = tx.rollback().await;
                     return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "success": false,
                            "error": "ไม่พบสิทธิ์การใช้งานที่ระบุ"
                        })),
                    )
                    .into_response();
                }
            };
            
            // Insert into junction table
            for perm_id in perm_ids {
                if let Err(e) = sqlx::query(
                    "INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2)"
                )
                .bind(role_id)
                .bind(perm_id)
                .execute(&mut *tx)
                .await 
                {
                     eprintln!("❌ Failed to assign permission: {}", e);
                     let _ = tx.rollback().await;
                     return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "success": false,
                            "error": "ไม่สามารถกำหนดสิทธิ์ได้"
                        })),
                    )
                    .into_response();
                }
            }
        }
    }

    if let Err(e) = tx.commit().await {
         eprintln!("❌ Failed to commit transaction: {}", e);
         return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": "ไม่สามารถบันทึกข้อมูลได้"
            })),
        )
        .into_response();
    }

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
    let _user = match check_permission(&headers, &pool, codes::ROLES_UPDATE_ALL).await {
        Ok(u) => u,
        Err(response) => return response,
    };


    // Use transaction for atomic operations
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            eprintln!("❌ Failed to start transaction: {}", e);
             return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถเริ่มต้น Transaction ได้"
                })),
            )
            .into_response();
        }
    };

    // Update role metadata
    let result = sqlx::query(
        "UPDATE roles 
         SET 
            name = COALESCE($2, name),
            name_en = COALESCE($3, name_en),
            description = COALESCE($4, description),
            user_type = COALESCE($5, user_type),
            level = COALESCE($6, level),
            is_active = COALESCE($7, is_active),
            updated_at = NOW()
         WHERE id = $1",
    )
    .bind(role_id)
    .bind(&payload.name)
    .bind(&payload.name_en)
    .bind(&payload.description)
    .bind(&payload.user_type)
    .bind(&payload.level)
    .bind(&payload.is_active)
    .execute(&mut *tx)
    .await;

    if let Err(e) = result {
         eprintln!("❌ Failed to update role: {}", e);
         let _ = tx.rollback().await;

         let err_msg = e.to_string();
         if err_msg.contains("duplicate key value") {
             if err_msg.contains("code") {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(json!({
                        "success": false,
                        "error": "รหัสบทบาทนี้มีอยู่ในระบบแล้ว"
                    })),
                ).into_response();
            }
         }

         return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": "เกิดข้อผิดพลาดในการอัปเดตบทบาท"
            })),
        )
        .into_response();
    }
    
    // Check if role exists
    if result.unwrap().rows_affected() == 0 {
         let _ = tx.rollback().await;
         return (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": "ไม่พบบทบาท"
            })),
        )
        .into_response();
    }

    // Update permissions if provided
    if let Some(permissions) = &payload.permissions {
        // Delete existing permissions
        if let Err(e) = sqlx::query("DELETE FROM role_permissions WHERE role_id = $1")
            .bind(role_id)
            .execute(&mut *tx)
            .await 
        {
             eprintln!("❌ Failed to clear old permissions: {}", e);
             let _ = tx.rollback().await;
             return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการลบสิทธิ์เดิม"
                })),
            )
            .into_response();
        }

        if !permissions.is_empty() {
             // Find permission IDs from codes
            let perm_ids: Vec<Uuid> = match sqlx::query_scalar(
                "SELECT id FROM permissions WHERE code = ANY($1)"
            )
            .bind(permissions)
            .fetch_all(&mut *tx)
            .await 
            {
                Ok(ids) => ids,
                Err(e) => {
                     eprintln!("❌ Failed to find permissions: {}", e);
                     let _ = tx.rollback().await;
                     return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "success": false,
                            "error": "ไม่พบสิทธิ์การใช้งานที่ระบุ"
                        })),
                    )
                    .into_response();
                }
            };
            
            // Insert new permissions
             for perm_id in perm_ids {
                if let Err(e) = sqlx::query(
                    "INSERT INTO role_permissions (role_id, permission_id) VALUES ($1, $2)"
                )
                .bind(role_id)
                .bind(perm_id)
                .execute(&mut *tx)
                .await 
                {
                     eprintln!("❌ Failed to assign new permission: {}", e);
                     let _ = tx.rollback().await;
                     return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "success": false,
                            "error": "ไม่สามารถกำหนดสิทธิ์ใหม่ได้"
                        })),
                    )
                    .into_response();
                }
            }
        }
    }

    if let Err(e) = tx.commit().await {
         eprintln!("❌ Failed to commit transaction: {}", e);
         return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({
                "success": false,
                "error": "ไม่สามารถบันทึกข้อมูลได้"
            })),
        )
        .into_response();
    }

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "อัปเดตบทบาทสำเร็จ"
        })),
    )
        .into_response()
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
    let _user = match check_permission(&headers, &pool, codes::ROLES_READ_ALL).await {
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
    let _user = match check_permission(&headers, &pool, codes::ROLES_READ_ALL).await {
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
    let _user = match check_permission(&headers, &pool, codes::ROLES_CREATE_ALL).await {
        Ok(u) => u,
        Err(response) => return response,
    };

    let dept_id: Uuid = match sqlx::query_scalar(
        "INSERT INTO departments (code, name, name_en, description, parent_department_id, phone, email, location, category, org_type)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
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
    .bind(payload.category.unwrap_or_else(|| "administrative".to_string()))
    .bind(payload.org_type.unwrap_or_else(|| "unit".to_string()))
    .fetch_one(&pool)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            eprintln!("❌ Failed to create department: {}", e);
            
            let err_msg = e.to_string();
            if err_msg.contains("duplicate key value") {
                 if err_msg.contains("code") {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({
                            "success": false,
                            "error": "รหัสฝ่ายนี้มีอยู่ในระบบแล้ว"
                        })),
                    ).into_response();
                }
            }

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
    let _user = match check_permission(&headers, &pool, codes::ROLES_UPDATE_ALL).await {
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
            category = COALESCE($9, category),
            org_type = COALESCE($10, org_type),
            is_active = COALESCE($11, is_active),
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
    .bind(&payload.category)
    .bind(&payload.org_type)
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
            
            let err_msg = e.to_string();
            if err_msg.contains("duplicate key value") {
                 if err_msg.contains("code") {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({
                            "success": false,
                            "error": "รหัสฝ่ายนี้มีอยู่ในระบบแล้ว"
                        })),
                    ).into_response();
                }
            }

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
