use crate::db::school_mapping::get_school_database_url;
use crate::models::auth::Claims;
use crate::models::staff::*;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;
use axum::{
    extract::{Path, Query, Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde_json::json;
use uuid::Uuid;

// ===================================================================
// List Staff
// ===================================================================

/// List all staff members with filters
pub async fn list_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<StaffListFilter>,
) -> Response {
    // Extract subdomain
    let subdomain = match extract_subdomain_from_request(&headers) {
        Ok(s) => s,
        Err(response) => return response,
    };

    // Get school database
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

    // Get pool
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

    // Pagination
    let page = filter.page.unwrap_or(1);
    let page_size = filter.page_size.unwrap_or(20);
    let offset = (page - 1) * page_size;

    // Build query
    let mut query = String::from("
        SELECT DISTINCT
            u.id,
            si.employee_id,
            u.first_name,
            u.last_name,
            u.status
        FROM users u
        LEFT JOIN staff_info si ON u.id = si.user_id
        WHERE u.user_type = 'staff'
    ");

    // Apply filters
    if let Some(status) = &filter.status {
        query.push_str(&format!(" AND u.status = '{}'", status));
    }

    if let Some(search) = &filter.search {
        query.push_str(&format!(
            " AND (u.first_name ILIKE '%{}%' OR u.last_name ILIKE '%{}%' OR si.employee_id ILIKE '%{}%')",
            search, search, search
        ));
    }

    query.push_str(&format!(" ORDER BY u.first_name LIMIT {} OFFSET {}", page_size, offset));

    // Execute query
    let staff_rows = match sqlx::query_as::<_, (Uuid, Option<String>, String, String, String)>(&query)
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
                    "error": "เกิดข้อผิดพลาดในการดึงข้อมูล"
                })),
            )
                .into_response();
        }
    };

    // Get total count
    let count_query = "SELECT COUNT(DISTINCT u.id) FROM users u WHERE u.user_type = 'staff'";
    let total: i64 = match sqlx::query_scalar(count_query).fetch_one(&pool).await {
        Ok(count) => count,
        Err(_) => 0,
    };

    let total_pages = (total as f64 / page_size as f64).ceil() as i64;

    // Build response (simplified for now)
    let items: Vec<StaffListItem> = staff_rows
        .into_iter()
        .map(|(id, employee_id, first_name, last_name, status)| StaffListItem {
            id,
            employee_id,
            first_name,
            last_name,
            roles: vec![],
            departments: vec![],
            status,
        })
        .collect();

    let response = StaffListResponse {
        success: true,
        data: items,
        total,
        page,
        page_size,
        total_pages,
    };

    (StatusCode::OK, Json(response)).into_response()
}

// ===================================================================
// Get Staff Profile
// ===================================================================

/// Get detailed staff profile by ID
pub async fn get_staff_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(staff_id): Path<Uuid>,
) -> Response {
    // Extract subdomain
    let subdomain = match extract_subdomain_from_request(&headers) {
        Ok(s) => s,
        Err(response) => return response,
    };

    // Get school database
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

    // Get pool
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

    // Get user basic info
    let user = match sqlx::query!(
        "SELECT id, national_id, email, title, first_name, last_name, nickname, phone, 
                user_type, status
         FROM users 
         WHERE id = $1 AND user_type = 'staff'",
        staff_id
    )
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": "ไม่พบบุคลากร"
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

    // Get staff info
    let staff_info = sqlx::query!(
        "SELECT employee_id, employment_type, education_level, major, university
         FROM staff_info 
         WHERE user_id = $1",
        staff_id
    )
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();

    // Get roles
    let roles = sqlx::query!(
        "SELECT r.id, r.code, r.name, r.name_en, r.category, r.level, ur.is_primary
         FROM user_roles ur
         JOIN roles r ON ur.role_id = r.id
         WHERE ur.user_id = $1 AND ur.ended_at IS NULL
         ORDER BY ur.is_primary DESC, r.level DESC",
        staff_id
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|row| RoleResponse {
        id: row.id,
        code: row.code,
        name: row.name,
        name_en: row.name_en,
        category: row.category,
        level: row.level,
        is_primary: Some(row.is_primary),
    })
    .collect();

    // Get departments
    let departments = sqlx::query!(
        "SELECT d.id, d.code, d.name, dm.position, dm.is_primary_department
         FROM department_members dm
         JOIN departments d ON dm.department_id = d.id
         WHERE dm.user_id = $1 AND dm.ended_at IS NULL
         ORDER BY dm.is_primary_department DESC",
        staff_id
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|row| DepartmentResponse {
        id: row.id,
        code: row.code,
        name: row.name,
        position: Some(row.position),
        is_primary_department: Some(row.is_primary_department),
    })
    .collect();

    // Get teaching assignments
    let teaching_assignments = sqlx::query!(
        "SELECT ta.id, ta.subject, ta.grade_level, ta.hours_per_week, ta.is_homeroom_teacher,
                ta.academic_year, ta.semester, c.code as class_code, c.name as class_name
         FROM teaching_assignments ta
         LEFT JOIN classes c ON ta.class_id = c.id
         WHERE ta.teacher_id = $1 AND ta.ended_at IS NULL
         ORDER BY ta.grade_level, ta.subject",
        staff_id
    )
    .fetch_all(&pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|row| TeachingAssignmentResponse {
        id: row.id,
        subject: row.subject,
        grade_level: row.grade_level,
        class_code: row.class_code,
        class_name: row.class_name,
        is_homeroom_teacher: row.is_homeroom_teacher,
        hours_per_week: row.hours_per_week.map(|h| h as f64),
        academic_year: row.academic_year,
        semester: row.semester,
    })
    .collect();

    // Build profile response
    let profile = StaffProfileResponse {
        id: user.id,
        national_id: user.national_id,
        email: user.email,
        title: user.title,
        first_name: user.first_name,
        last_name: user.last_name,
        nickname: user.nickname,
        phone: user.phone,
        user_type: user.user_type,
        status: user.status,
        staff_info: staff_info.map(|si| StaffInfoResponse {
            employee_id: si.employee_id,
            employment_type: si.employment_type,
            education_level: si.education_level,
            major: si.major,
            university: si.university,
        }),
        roles,
        departments,
        teaching_assignments,
        permissions: vec![], // TODO: Collect from roles
    };

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": profile
        })),
    )
        .into_response()
}

// ===================================================================
// Create Staff
// ===================================================================

/// Create new staff member
pub async fn create_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateStaffRequest>,
) -> Response {
    // Extract subdomain
    let subdomain = match extract_subdomain_from_request(&headers) {
        Ok(s) => s,
        Err(response) => return response,
    };

    // Get school database
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

    // Get pool
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

    // Hash password
    let password_hash = match bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST) {
        Ok(hash) => hash,
        Err(e) => {
            eprintln!("❌ Password hashing failed: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการสร้างรหัสผ่าน"
                })),
            )
                .into_response();
        }
    };

    // Start transaction
    let mut tx = match pool.begin().await {
        Ok(tx) => tx,
        Err(e) => {
            eprintln!("❌ Failed to start transaction: {}", e);
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

    // Create user
    let user_id = match sqlx::query_scalar!(
        "INSERT INTO users (
            national_id, email, password_hash, title, first_name, last_name, nickname,
            phone, emergency_contact, line_id, date_of_birth, gender, address,
            user_type, hired_date, status
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 'staff', $14, 'active')
        RETURNING id",
        payload.national_id,
        payload.email,
        password_hash,
        payload.title,
        payload.first_name,
        payload.last_name,
        payload.nickname,
        payload.phone,
        payload.emergency_contact,
        payload.line_id,
        payload.date_of_birth,
        payload.gender,
        payload.address,
        payload.hired_date
    )
    .fetch_one(&mut *tx)
    .await
    {
        Ok(id) => id,
        Err(e) => {
            eprintln!("❌ Failed to create user: {}", e);
            let _ = tx.rollback().await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถสร้างผู้ใช้งานได้"
                })),
            )
                .into_response();
        }
    };

    // Create staff info
    let work_days_json = serde_json::to_value(payload.staff_info.work_days.unwrap_or_default())
        .unwrap_or(serde_json::Value::Null);

    match sqlx::query!(
        "INSERT INTO staff_info (
            user_id, employee_id, employment_type, education_level, major, university,
            teaching_license_number, teaching_license_expiry, work_days, metadata
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, '{}'::jsonb)",
        user_id,
        payload.staff_info.employee_id,
        payload.staff_info.employment_type,
        payload.staff_info.education_level,
        payload.staff_info.major,
        payload.staff_info.university,
        payload.staff_info.teaching_license_number,
        payload.staff_info.teaching_license_expiry,
        work_days_json
    )
    .execute(&mut *tx)
    .await
    {
        Ok(_) => {}
        Err(e) => {
            eprintln!("❌ Failed to create staff info: {}", e);
            let _ = tx.rollback().await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถสร้างข้อมูลบุคลากรได้"
                })),
            )
                .into_response();
        }
    };

    // Assign roles
    for role_id in &payload.role_ids {
        let is_primary = payload.primary_role_id.as_ref() == Some(role_id);
        
        match sqlx::query!(
            "INSERT INTO user_roles (user_id, role_id, is_primary, started_at)
             VALUES ($1, $2, $3, CURRENT_DATE)",
            user_id,
            role_id,
            is_primary
        )
        .execute(&mut *tx)
        .await
        {
            Ok(_) => {}
            Err(e) => {
                eprintln!("❌ Failed to assign role: {}", e);
                // Continue anyway
            }
        }
    }

    // Assign departments
    if let Some(dept_assignments) = &payload.department_assignments {
        for assignment in dept_assignments {
            match sqlx::query!(
                "INSERT INTO department_members (
                    user_id, department_id, position, is_primary_department, 
                    responsibilities, started_at
                ) VALUES ($1, $2, $3, $4, $5, CURRENT_DATE)",
                user_id,
                assignment.department_id,
                assignment.position,
                assignment.is_primary.unwrap_or(false),
                assignment.responsibilities
            )
            .execute(&mut *tx)
            .await
            {
                Ok(_) => {}
                Err(e) => {
                    eprintln!("❌ Failed to assign department: {}", e);
                    // Continue anyway
                }
            }
        }
    }

    // Commit transaction
    match tx.commit().await {
        Ok(_) => {
            println!("✅ Staff created successfully: {}", user_id);
            (
                StatusCode::CREATED,
                Json(json!({
                    "success": true,
                    "message": "สร้างบุคลากรสำเร็จ",
                    "data": {
                        "id": user_id
                    }
                })),
            )
                .into_response()
        }
        Err(e) => {
            eprintln!("❌ Failed to commit transaction: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการบันทึกข้อมูล"
                })),
            )
                .into_response()
        }
    }
}

// ===================================================================
// Update Staff
// ===================================================================

/// Update staff member
pub async fn update_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(staff_id): Path<Uuid>,
    Json(payload): Json<UpdateStaffRequest>,
) -> Response {
    // Extract subdomain
    let subdomain = match extract_subdomain_from_request(&headers) {
        Ok(s) => s,
        Err(response) => return response,
    };

    // Get school database
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

    // Get pool
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

    // Build dynamic update query (simplified version)
    let result = sqlx::query!(
        "UPDATE users 
         SET 
            title = COALESCE($2, title),
            first_name = COALESCE($3, first_name),
            last_name = COALESCE($4, last_name),
            nickname = COALESCE($5, nickname),
            phone = COALESCE($6, phone),
            updated_at = NOW()
         WHERE id = $1 AND user_type = 'staff'",
        staff_id,
        payload.title,
        payload.first_name,
        payload.last_name,
        payload.nickname,
        payload.phone
    )
    .execute(&pool)
    .await;

    match result {
        Ok(result) if result.rows_affected() > 0 => {
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "อัปเดตข้อมูลสำเร็จ"
                })),
            )
                .into_response()
        }
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": "ไม่พบบุคลากร"
            })),
        )
            .into_response(),
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการอัปเดตข้อมูล"
                })),
            )
                .into_response()
        }
    }
}

// ===================================================================
// Delete Staff (Soft Delete)
// ===================================================================

/// Delete (deactivate) staff member
pub async fn delete_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(staff_id): Path<Uuid>,
) -> Response {
    // Extract subdomain
    let subdomain = match extract_subdomain_from_request(&headers) {
        Ok(s) => s,
        Err(response) => return response,
    };

    // Get school database
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

    // Get pool
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

    // Soft delete (set status to inactive)
    let result = sqlx::query!(
        "UPDATE users 
         SET status = 'inactive', updated_at = NOW()
         WHERE id = $1 AND user_type = 'staff'",
        staff_id
    )
    .execute(&pool)
    .await;

    match result {
        Ok(result) if result.rows_affected() > 0 => {
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "ลบบุคลากรสำเร็จ"
                })),
            )
                .into_response()
        }
        Ok(_) => (
            StatusCode::NOT_FOUND,
            Json(json!({
                "success": false,
                "error": "ไม่พบบุคลากร"
            })),
        )
            .into_response(),
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการลบบุคลากร"
                })),
            )
                .into_response()
        }
    }
}
