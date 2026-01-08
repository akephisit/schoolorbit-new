use crate::db::school_mapping::get_school_database_url;
use crate::models::staff::*;
use crate::models::auth::User;

use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use serde_json::json;
use sqlx::FromRow;
use uuid::Uuid;

// Helper structs for query results
#[derive(Debug, FromRow)]
struct UserBasicRow {
    id: Uuid,
    national_id: Option<String>,
    email: Option<String>,
    title: Option<String>,
    first_name: String,
    last_name: String,
    nickname: Option<String>,
    phone: Option<String>,
    emergency_contact: Option<String>,
    line_id: Option<String>,
    date_of_birth: Option<chrono::NaiveDate>,
    gender: Option<String>,
    address: Option<String>,
    hired_date: Option<chrono::NaiveDate>,
    user_type: String,
    status: String,
}

#[derive(Debug, FromRow)]
struct StaffInfoRow {
    education_level: Option<String>,
    major: Option<String>,
    university: Option<String>,
}

#[derive(Debug, FromRow)]
struct RoleRow {
    id: Uuid,
    code: String,
    name: String,
    name_en: Option<String>,
    user_type: String, // Changed from category to user_type
    level: i32,
    is_primary: bool,
}

#[derive(Debug, FromRow)]
struct DepartmentRow {
    id: Uuid,
    code: String,
    name: String,
    position: String,
    is_primary_department: bool,
}

#[derive(Debug, FromRow)]
struct TeachingRow {
    id: Uuid,
    subject: String,
    grade_level: Option<String>,
    hours_per_week: Option<f64>,
    is_homeroom_teacher: bool,
    academic_year: String,
    semester: String,
    class_code: Option<String>,
    class_name: Option<String>,
}

// ===================================================================
// Helper Functions
// ===================================================================

/// Extract user from request and check permission
async fn check_user_permission(
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
        Err(e) => {
            eprintln!("❌ Failed to get user: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถดึงข้อมูลผู้ใช้ได้"
                })),
            ).into_response());
        }
    };
    
    // Check permission
    match user.has_permission(pool, required_permission).await {
        Ok(true) => Ok(user),
        Ok(false) => {
            Err((
                StatusCode::FORBIDDEN,
                Json(json!({
                    "success": false,
                    "error": format!("คุณไม่มีสิทธิ์ (ต้องการ {} permission)", required_permission)
                })),
            ).into_response())
        },
        Err(e) => {
            eprintln!("❌ Permission check error: {}", e);
            Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการตรวจสอบสิทธิ์"
                })),
            ).into_response())
        }
    }
}

// ===================================================================
// Handlers
// ===================================================================

// ===================================================================
// List Staff
// ===================================================================

pub async fn list_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<StaffListFilter>,
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
    let _user = match check_user_permission(&headers, &pool, "staff.read.all").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    let page = filter.page.unwrap_or(1);
    let page_size = filter.page_size.unwrap_or(20);
    let offset = (page - 1) * page_size;

    let mut query = String::from(
        "SELECT DISTINCT u.id, u.first_name, u.last_name, u.status
         FROM users u
         WHERE u.user_type = 'staff'",
    );

    // Default to active staff only (unless status filter is explicitly provided)
    if let Some(status) = &filter.status {
        query.push_str(&format!(" AND u.status = '{}'", status));
    } else {
        // Default: show only active staff
        query.push_str(" AND u.status = 'active'");
    }


    if let Some(search) = &filter.search {
        query.push_str(&format!(
            " AND (u.first_name ILIKE '%{}%' OR u.last_name ILIKE '%{}%')",
            search, search
        ));
    }

    query.push_str(&format!(
        " ORDER BY u.first_name LIMIT {} OFFSET {}",
        page_size, offset
    ));

    let staff_rows = match sqlx::query_as::<_, (Uuid, String, String, String)>(&query)
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

    let count_query = "SELECT COUNT(DISTINCT u.id) FROM users u WHERE u.user_type = 'staff'";
    let total: i64 = match sqlx::query_scalar(count_query).fetch_one(&pool).await {
        Ok(count) => count,
        Err(_) => 0,
    };

    let total_pages = (total as f64 / page_size as f64).ceil() as i64;

    let items: Vec<StaffListItem> = staff_rows
        .into_iter()
        .map(|(id, first_name, last_name, status)| StaffListItem {
            id,
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

pub async fn get_staff_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(staff_id): Path<Uuid>,
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
    let _user = match check_user_permission(&headers, &pool, "staff.read.all").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    if let Err(e) = crate::utils::encryption::setup_encryption_key(&pool).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "System error"}))).into_response();
    }

    // Get user basic info
    let user = match sqlx::query_as::<_, UserBasicRow>(
        "SELECT id, pgp_sym_decrypt(national_id, current_setting('app.encryption_key')) as national_id, email, title, first_name, last_name, nickname, phone, 
                emergency_contact, line_id, date_of_birth, gender, address, hired_date,
                user_type, status
         FROM users 
         WHERE id = $1 AND user_type = 'staff'",
    )
    .bind(staff_id)
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
    let staff_info = sqlx::query_as::<_, StaffInfoRow>(
        "SELECT education_level, major, university
         FROM staff_info 
         WHERE user_id = $1",
    )
    .bind(staff_id)
    .fetch_optional(&pool)
    .await
    .ok()
    .flatten();

    // Get roles
    let roles = sqlx::query_as::<_, RoleRow>(
        "SELECT r.id, r.code, r.name, r.name_en, r.user_type, r.level, ur.is_primary
         FROM user_roles ur
         JOIN roles r ON ur.role_id = r.id
         WHERE ur.user_id = $1 AND ur.ended_at IS NULL
         ORDER BY ur.is_primary DESC, r.level DESC",
    )
    .bind(staff_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default()
    .into_iter()
    .map(|row| RoleResponse {
        id: row.id,
        code: row.code,
        name: row.name,
        name_en: row.name_en,
        user_type: row.user_type,
        level: row.level,
        is_primary: Some(row.is_primary),
    })
    .collect();

    // Get departments
    let departments = sqlx::query_as::<_, DepartmentRow>(
        "SELECT d.id, d.code, d.name, dm.position, dm.is_primary_department
         FROM department_members dm
         JOIN departments d ON dm.department_id = d.id
         WHERE dm.user_id = $1 AND dm.ended_at IS NULL
         ORDER BY dm.is_primary_department DESC",
    )
    .bind(staff_id)
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
    let teaching_assignments = sqlx::query_as::<_, TeachingRow>(
        "SELECT ta.id, ta.subject, ta.grade_level, ta.hours_per_week, ta.is_homeroom_teacher,
                ta.academic_year, ta.semester, c.code as class_code, c.name as class_name
         FROM teaching_assignments ta
         LEFT JOIN classes c ON ta.class_id = c.id
         WHERE ta.teacher_id = $1 AND ta.ended_at IS NULL
         ORDER BY ta.grade_level, ta.subject",
    )
    .bind(staff_id)
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
        hours_per_week: row.hours_per_week,
        academic_year: row.academic_year,
        semester: row.semester,
    })
    .collect();

    let profile = StaffProfileResponse {
        id: user.id,
        national_id: user.national_id,
        email: user.email,
        title: user.title,
        first_name: user.first_name,
        last_name: user.last_name,
        nickname: user.nickname,
        phone: user.phone,
        emergency_contact: user.emergency_contact,
        line_id: user.line_id,
        date_of_birth: user.date_of_birth.map(|d| d.to_string()),
        gender: user.gender,
        address: user.address,
        hired_date: user.hired_date.map(|d| d.to_string()),
        user_type: user.user_type,
        status: user.status,
        staff_info: staff_info.map(|si| StaffInfoResponse {
            education_level: si.education_level,
            major: si.major,
            university: si.university,
        }),
        roles,
        departments,
        teaching_assignments,
        permissions: vec![],
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

pub async fn create_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateStaffRequest>,
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
    let _user = match check_user_permission(&headers, &pool, "staff.create.all").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    // Setup encryption key for encrypted columns
    if let Err(e) = crate::utils::encryption::setup_encryption_key(&pool).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({"error": "System error"})),
        ).into_response();
    }

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

    // Check if user already exists (might be inactive)
    let existing_user: Option<(Uuid, String)> = sqlx::query_as(
        "SELECT id, status FROM users WHERE pgp_sym_decrypt(national_id, current_setting('app.encryption_key')) = $1"
    )
    .bind(&payload.national_id)
    .fetch_optional(&mut *tx)
    .await
    .ok()
    .flatten();

    let user_id: Uuid = if let Some((existing_id, status)) = existing_user {
        if status == "inactive" {
            // Reactivate existing user
            println!("♻️  Reactivating inactive user: {}", existing_id);
            
            // Clean up old relationships first
            println!("🧹 Cleaning up old user_roles and department_members...");
            
            // Delete old user_roles
            let _ = sqlx::query("DELETE FROM user_roles WHERE user_id = $1")
                .bind(existing_id)
                .execute(&mut *tx)
                .await;
            
            // Delete old department_members
            let _ = sqlx::query("DELETE FROM department_members WHERE user_id = $1")
                .bind(existing_id)
                .execute(&mut *tx)
                .await;
            
            // Delete old teaching_assignments if exists
            let _ = sqlx::query("DELETE FROM teaching_assignments WHERE teacher_id = $1")
                .bind(existing_id)
                .execute(&mut *tx)
                .await;
            
            // Update user info
            match sqlx::query(
                "UPDATE users SET 
                    email = $1, password_hash = $2, title = $3, first_name = $4, last_name = $5, 
                    nickname = $6, phone = $7, emergency_contact = $8, line_id = $9, 
                    date_of_birth = $10, gender = $11, address = $12, hired_date = $13,
                    status = 'active', updated_at = NOW()
                WHERE id = $14"
            )
            .bind(&payload.email)
            .bind(&password_hash)
            .bind(&payload.title)
            .bind(&payload.first_name)
            .bind(&payload.last_name)
            .bind(&payload.nickname)
            .bind(&payload.phone)
            .bind(&payload.emergency_contact)
            .bind(&payload.line_id)
            .bind(&payload.date_of_birth)
            .bind(&payload.gender)
            .bind(&payload.address)
            .bind(&payload.hired_date)
            .bind(existing_id)
            .execute(&mut *tx)
            .await
            {
                Ok(_) => {
                    println!("✅ User reactivated and cleaned up successfully");
                    existing_id
                },
                Err(e) => {
                    eprintln!("❌ Failed to reactivate user: {}", e);
                    let _ = tx.rollback().await;
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "success": false,
                            "error": "ไม่สามารถเปิดใช้งานบุคลากรได้"
                        })),
                    )
                        .into_response();
                }
            }
        } else {
            // User exists and is active - duplicate!
            let _ = tx.rollback().await;
            return (
                StatusCode::CONFLICT,
                Json(json!({
                    "success": false,
                    "error": "เลขบัตรประชาชนนี้มีในระบบแล้ว"
                })),
            )
                .into_response();
        }
    } else {
        // Create new user
        match sqlx::query_scalar(
            "INSERT INTO users (
                national_id, email, password_hash, title, first_name, last_name, nickname,
                phone, emergency_contact, line_id, date_of_birth, gender, address,
                user_type, hired_date, status
            ) VALUES (pgp_sym_encrypt($1, current_setting('app.encryption_key')), $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, 'staff', $14, 'active')
            RETURNING id",
        )
        .bind(&payload.national_id)
        .bind(&payload.email)
        .bind(&password_hash)
        .bind(&payload.title)
        .bind(&payload.first_name)
        .bind(&payload.last_name)
        .bind(&payload.nickname)
        .bind(&payload.phone)
        .bind(&payload.emergency_contact)
        .bind(&payload.line_id)
        .bind(&payload.date_of_birth)
        .bind(&payload.gender)
        .bind(&payload.address)
        .bind(&payload.hired_date)
        .fetch_one(&mut *tx)
        .await
        {
            Ok(id) => id,
            Err(e) => {
                eprintln!("❌ Failed to create user: {}", e);
                let _ = tx.rollback().await;
                
                // More detailed error message
                let error_msg = if e.to_string().contains("unique constraint") {
                    if e.to_string().contains("email") {
                        "อีเมลนี้มีในระบบแล้ว"
                    } else {
                        "ข้อมูลซ้ำในระบบ กรุณาตรวจสอบข้อมูล"
                    }
                } else if e.to_string().contains("null value") {
                    &format!("ข้อมูลบังคับกรอกไม่ครบ: {}", e)
                } else {
                    &format!("ไม่สามารถสร้างผู้ใช้งานได้: {}", e)
                };
                
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({
                        "success": false,
                        "error": error_msg
                    })),
                )
                    .into_response();
            }
        }
    };


    // Create staff info (if provided)
    if let Some(staff_info) = &payload.staff_info {
        let work_days_json = serde_json::to_value(staff_info.work_days.clone().unwrap_or_default())
            .unwrap_or(serde_json::Value::Null);

        match sqlx::query(
            "INSERT INTO staff_info (
                user_id, education_level, major, university,
                teaching_license_number, teaching_license_expiry, work_days, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, '{}'::jsonb)",
        )
        .bind(user_id)
        .bind(&staff_info.education_level)
        .bind(&staff_info.major)
        .bind(&staff_info.university)
        .bind(&staff_info.teaching_license_number)
        .bind(&staff_info.teaching_license_expiry)
        .bind(&work_days_json)
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
    }



    // ===================================================================
    // Validate: All roles must have user_type = 'staff'
    // ===================================================================
    if !payload.role_ids.is_empty() {
        let invalid_roles: Vec<String> = sqlx::query_as::<_, (String,)>(
            "SELECT code FROM roles 
             WHERE id = ANY($1) 
               AND (user_type != 'staff' OR is_active = false)"
        )
        .bind(&payload.role_ids)
        .fetch_all(&mut *tx)
        .await
        .unwrap_or_default()
        .into_iter()
        .map(|(code,)| code)
        .collect();

        if !invalid_roles.is_empty() {
            eprintln!(
                "❌ Role validation failed for staff: invalid roles = {:?}",
                invalid_roles
            );
            let _ = tx.rollback().await;
            return (
                StatusCode::BAD_REQUEST,
                Json(json!({
                    "success": false,
                    "error": format!(
                        "บทบาทต่อไปนี้ไม่สามารถใช้กับบุคลากรได้: {}",
                        invalid_roles.join(", ")
                    )
                })),
            )
                .into_response();
        }
    }

    // Assign roles
    for role_id in &payload.role_ids {
        let is_primary = payload.primary_role_id.as_ref() == Some(role_id);

        let _ = sqlx::query(
            "INSERT INTO user_roles (user_id, role_id, is_primary, started_at)
             VALUES ($1, $2, $3, CURRENT_DATE)",
        )
        .bind(user_id)
        .bind(role_id)
        .bind(is_primary)
        .execute(&mut *tx)
        .await;
    }

    // Assign departments
    if let Some(dept_assignments) = &payload.department_assignments {
        for assignment in dept_assignments {
            let _ = sqlx::query(
                "INSERT INTO department_members (
                    user_id, department_id, position, is_primary_department, 
                    responsibilities, started_at
                ) VALUES ($1, $2, $3, $4, $5, CURRENT_DATE)",
            )
            .bind(user_id)
            .bind(assignment.department_id)
            .bind(&assignment.position)
            .bind(assignment.is_primary.unwrap_or(false))
            .bind(&assignment.responsibilities)
            .execute(&mut *tx)
            .await;
        }
    }

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

pub async fn update_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(staff_id): Path<Uuid>,
    Json(payload): Json<UpdateStaffRequest>,
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
    let _user = match check_user_permission(&headers, &pool, "staff.update.all").await {
        Ok(u) => u,
        Err(response) => return response,
    };

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

    // Update user table
    let result = sqlx::query(
        "UPDATE users 
         SET 
            title = COALESCE($2, title),
            first_name = COALESCE($3, first_name),
            last_name = COALESCE($4, last_name),
            nickname = COALESCE($5, nickname),
            email = COALESCE($6, email),
            phone = COALESCE($7, phone),
            emergency_contact = COALESCE($8, emergency_contact),
            line_id = COALESCE($9, line_id),
            date_of_birth = COALESCE($10, date_of_birth),
            gender = COALESCE($11, gender),
            address = COALESCE($12, address),
            hired_date = COALESCE($13, hired_date),
            status = COALESCE($14, status),
            updated_at = NOW()
         WHERE id = $1 AND user_type = 'staff'",
    )
    .bind(staff_id)
    .bind(&payload.title)
    .bind(&payload.first_name)
    .bind(&payload.last_name)
    .bind(&payload.nickname)
    .bind(&payload.email)
    .bind(&payload.phone)
    .bind(&payload.emergency_contact)
    .bind(&payload.line_id)
    .bind(&payload.date_of_birth)
    .bind(&payload.gender)
    .bind(&payload.address)
    .bind(&payload.hired_date)
    .bind(&payload.status)
    .execute(&mut *tx)
    .await;

    match result {
        Ok(result) if result.rows_affected() > 0 => {
            // Update staff_info if provided
            if let Some(staff_info) = &payload.staff_info {
                // Check if staff_info exists
                let exists: bool = sqlx::query_scalar(
                    "SELECT EXISTS(SELECT 1 FROM staff_info WHERE user_id = $1)"
                )
                .bind(staff_id)
                .fetch_one(&mut *tx)
                .await
                .unwrap_or(false);

                if exists {
                    // Update existing record
                    let _ = sqlx::query(
                        "UPDATE staff_info 
                         SET 
                            education_level = COALESCE($2, education_level),
                            major = COALESCE($3, major),
                            university = COALESCE($4, university),
                            updated_at = NOW()
                         WHERE user_id = $1",
                    )
                    .bind(staff_id)
                    .bind(&staff_info.education_level)
                    .bind(&staff_info.major)
                    .bind(&staff_info.university)
                    .execute(&mut *tx)
                    .await;
                } else {
                    // Create new record
                    let _ = sqlx::query(
                        "INSERT INTO staff_info (user_id, education_level, major, university, work_days, metadata)
                         VALUES ($1, $2, $3, $4, '[]'::jsonb, '{}'::jsonb)",
                    )
                    .bind(staff_id)
                    .bind(&staff_info.education_level)
                    .bind(&staff_info.major)
                    .bind(&staff_info.university)
                    .execute(&mut *tx)
                    .await;
                }
            }

            // Update roles if provided
            if let Some(role_ids) = &payload.role_ids {
                // Delete existing roles
                let _ = sqlx::query("DELETE FROM user_roles WHERE user_id = $1")
                    .bind(staff_id)
                    .execute(&mut *tx)
                    .await;

                // Insert new roles
                for role_id in role_ids {
                    let is_primary = payload.primary_role_id.as_ref() == Some(role_id);

                    let _ = sqlx::query(
                        "INSERT INTO user_roles (user_id, role_id, is_primary, started_at)
                         VALUES ($1, $2, $3, CURRENT_DATE)",
                    )
                    .bind(staff_id)
                    .bind(role_id)
                    .bind(is_primary)
                    .execute(&mut *tx)
                    .await;
                }
            }

            // Update departments if provided
            if let Some(dept_assignments) = &payload.department_assignments {
                // Delete existing department assignments
                let _ = sqlx::query("DELETE FROM department_members WHERE user_id = $1")
                    .bind(staff_id)
                    .execute(&mut *tx)
                    .await;

                // Insert new department assignments
                for assignment in dept_assignments {
                    let _ = sqlx::query(
                        "INSERT INTO department_members (
                            user_id, department_id, position, is_primary_department, 
                            responsibilities, started_at
                        ) VALUES ($1, $2, $3, $4, $5, CURRENT_DATE)",
                    )
                    .bind(staff_id)
                    .bind(assignment.department_id)
                    .bind(&assignment.position)
                    .bind(assignment.is_primary.unwrap_or(false))
                    .bind(&assignment.responsibilities)
                    .execute(&mut *tx)
                    .await;
                }
            }

            match tx.commit().await {
                Ok(_) => (
                    StatusCode::OK,
                    Json(json!({
                        "success": true,
                        "message": "อัปเดตข้อมูลสำเร็จ"
                    })),
                )
                    .into_response(),
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
        Ok(_) => {
            let _ = tx.rollback().await;
            (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": "ไม่พบบุคลากร"
                })),
            )
                .into_response()
        }
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            let _ = tx.rollback().await;
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

pub async fn delete_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(staff_id): Path<Uuid>,
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
    let _user = match check_user_permission(&headers, &pool, "staff.delete.all").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    let result = sqlx::query(
        "UPDATE users 
         SET status = 'inactive', updated_at = NOW()
         WHERE id = $1 AND user_type = 'staff'",
    )
    .bind(staff_id)
    .execute(&pool)
    .await;

    match result {
        Ok(result) if result.rows_affected() > 0 => (
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "ลบบุคลากรสำเร็จ"
            })),
        )
            .into_response(),
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
