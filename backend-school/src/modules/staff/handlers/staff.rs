use crate::db::school_mapping::get_school_database_url;
use crate::modules::staff::models::*;
use crate::modules::auth::models::User;

use crate::utils::subdomain::extract_subdomain_from_request;
use crate::utils::field_encryption;
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
use chrono::Datelike;

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
    profile_image_url: Option<String>,
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
    use crate::modules::auth::permissions::UserPermissions;
    
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
    let mut user: User = match sqlx::query_as(
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
    
    // Decrypt national_id
    if let Some(encrypted_national_id) = user.national_id {
        if let Ok(decrypted_national_id) = field_encryption::decrypt(&encrypted_national_id) {
            user.national_id = Some(decrypted_national_id);
        } else {
            eprintln!("❌ Failed to decrypt national_id for user {}", user.id);
            user.national_id = None; // Or handle error as appropriate
        }
    }
    
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
    let auth_result = check_user_permission(&headers, &pool, "staff.read.all").await;
    let _user = match auth_result {
        Ok(u) => u,
        Err(_) => {
            match check_user_permission(&headers, &pool, "achievement.create.all").await {
                Ok(u) => u,
                Err(response) => return response,
            }
        }
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

    // Get user basic info (encryption key auto-set by pool)
    let mut user = match sqlx::query_as::<_, UserBasicRow>(
        "SELECT id, national_id, email, title, first_name, last_name, nickname, phone, 
                emergency_contact, line_id, date_of_birth, gender, address, hired_date,
                user_type, status, profile_image_url
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
    
    // Decrypt national_id
    if let Some(ref nid) = user.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            user.national_id = Some(dec);
        }
    }

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
        profile_image_url: user.profile_image_url,
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

    // Hash password (encryption key auto-set by pool)
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

    // Encrypt national_id
    let encrypted_national_id = match field_encryption::encrypt_optional(payload.national_id.as_deref()) {
        Ok(enc) => enc,
        Err(e) => {
             eprintln!("Encryption failed: {}", e);
             return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"success": false, "error": "Encryption error"}))).into_response();
        }
    };

    // Hash national_id for search
    let national_id_hash = payload.national_id.as_deref().map(|s| field_encryption::hash_for_search(s));

    // Generate running number for staff code if not provided
    // Pattern: T + Year(2) + Running(4) e.g., T670001
    let username = if let Some(u) = &payload.username {
         if !u.is_empty() { u.clone() } else { 
             // Generate default
             let thai_year = (chrono::Utc::now().year() + 543) % 100;
             let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE user_type = 'staff'")
                 .fetch_one(&pool).await.unwrap_or(0);
             format!("T{}{:04}", thai_year, count + 1)
         }
    } else {
         let thai_year = (chrono::Utc::now().year() + 543) % 100;
         let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM users WHERE user_type = 'staff'")
             .fetch_one(&pool).await.unwrap_or(0);
         format!("T{}{:04}", thai_year, count + 1)
    };

    // Check if user already exists (by username or national_id if provided)
    // Simplified: Just insert, if fail let unique constraint handle it or check username specifically.
    // For "Reactivation" logic, we might need to check if we want to support it still.
    // Given the prompt "don't care about old data", we can probably simplify this.
    // But let's try to keep it robust.

    // Create new user
    let user_id: Uuid = match sqlx::query_scalar(
        "INSERT INTO users (
            username, national_id, national_id_hash, email, password_hash, title, first_name, last_name, nickname,
            phone, emergency_contact, line_id, date_of_birth, gender, address,
            user_type, hired_date, status, profile_image_url
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, 'staff', $16, 'active', $17)
        RETURNING id",
    )
    .bind(&username)
    .bind(&encrypted_national_id)
    .bind(&national_id_hash)
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
    .bind(&payload.profile_image_url)
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
                    "error": "ไม่สามารถสร้างบุคลากรได้ (Username อาจซ้ำ)"
                })),
            )
            .into_response();
        }
    };



    // Create staff info (if provided)
    if let Some(staff_info) = &payload.staff_info {
        match sqlx::query(
            "INSERT INTO staff_info (
                user_id, education_level, major, university,
                teaching_license_number, teaching_license_expiry, metadata
            ) VALUES ($1, $2, $3, $4, $5, $6, '{}'::jsonb)",
        )
        .bind(user_id)
        .bind(&staff_info.education_level)
        .bind(&staff_info.major)
        .bind(&staff_info.university)
        .bind(&staff_info.teaching_license_number)
        .bind(&staff_info.teaching_license_expiry)
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
            profile_image_url = COALESCE($15, profile_image_url),
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
    .bind(&payload.profile_image_url)
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
                        "INSERT INTO staff_info (user_id, education_level, major, university, metadata)
                         VALUES ($1, $2, $3, $4, '{}'::jsonb)",
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

pub async fn get_public_staff_profile(
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
            ).into_response();
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
            ).into_response();
        }
    };

    // Authentication Only
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|h| h.to_str().ok());
    let token_from_header = auth_header.and_then(|h| if h.starts_with("Bearer ") { Some(h[7..].to_string()) } else { None });
    let token_from_cookie = headers.get(header::COOKIE).and_then(|h| h.to_str().ok()).and_then(|cookie| crate::utils::jwt::JwtService::extract_token_from_cookie(Some(cookie)));
    
    let token = match token_from_header.or(token_from_cookie) {
        Some(t) => t,
        None => return (StatusCode::UNAUTHORIZED, Json(json!({"success": false,"error": "กรุณาเข้าสู่ระบบ"}))).into_response(),
    };
    
    if let Err(_) = crate::utils::jwt::JwtService::verify_token(&token) {
        return (StatusCode::UNAUTHORIZED, Json(json!({"success": false,"error": "Token ไม่ถูกต้อง"}))).into_response();
    }

    // Helper Structs for Public Profile
    #[derive(sqlx::FromRow)]
    struct PublicUserRow {
        id: Uuid,
        first_name: String,
        last_name: String,
        nickname: Option<String>,
        email: Option<String>,
        user_type: String,
        status: String,
        profile_image_url: Option<String>,
        title: Option<String>,
        phone: Option<String>,
        hired_date: Option<chrono::NaiveDate>,
    }

    // 1. Get User Basic Info
    // Note: Alias phone_number no longer needed if we map to 'phone' field in struct
    let user_rec = match sqlx::query_as::<_, PublicUserRow>(
        "SELECT id, first_name, last_name, nickname, email, user_type, status, profile_image_url, title, phone, hired_date
         FROM users WHERE id = $1 AND user_type = 'staff'"
    )
    .bind(staff_id)
    .fetch_optional(&pool)
    .await {
         Ok(Some(u)) => u,
         Ok(None) => return (StatusCode::NOT_FOUND, Json(json!({ "success": false, "error": "ไม่พบบุคลากร" }))).into_response(),
         Err(e) => {
             eprintln!("❌ Database error (user): {}", e);
             return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({ "success": false, "error": "เกิดข้อผิดพลาดในการดึงข้อมูล" }))).into_response();
         }
    };

    #[derive(sqlx::FromRow)]
    struct PublicRoleRow {
        id: Uuid,
        code: String,
        name: String,
        level: Option<i32>, // level in roles is i32
    }

    // 2. Get Roles
    let roles = sqlx::query_as::<_, PublicRoleRow>(
        "SELECT r.id, r.code, r.name, r.level 
         FROM user_roles ur
         JOIN roles r ON ur.role_id = r.id
         WHERE ur.user_id = $1"
    )
    .bind(staff_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    #[derive(sqlx::FromRow)]
    struct PublicDeptRow {
        id: Uuid,
        code: String,
        name: String,
        position: String,
    }

    // 3. Get Departments
    let departments = sqlx::query_as::<_, PublicDeptRow>(
        "SELECT d.id, d.code, d.name, dm.position
         FROM department_members dm
         JOIN departments d ON dm.department_id = d.id
         WHERE dm.user_id = $1"
    )
    .bind(staff_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Construct Response
    let response_data = json!({
        "id": user_rec.id,
        "first_name": user_rec.first_name,
        "last_name": user_rec.last_name,
        "nickname": user_rec.nickname,
        "title": user_rec.title,
        "email": user_rec.email,
        "phone": user_rec.phone,
        "hired_date": user_rec.hired_date,
        "profile_image_url": user_rec.profile_image_url,
        "user_type": user_rec.user_type,
        "status": user_rec.status,
        "roles": roles.into_iter().map(|r| json!({
            "id": r.id,
            "code": r.code,
            "name": r.name,
            "level": r.level
        })).collect::<Vec<_>>(),
        "departments": departments.into_iter().map(|d| json!({
            "id": d.id,
            "code": d.code,
            "name": d.name,
            "position": d.position
        })).collect::<Vec<_>>()
    });

    (StatusCode::OK, Json(json!({ "success": true, "data": response_data }))).into_response()
}
