use crate::db::school_mapping::get_school_database_url;
use crate::modules::staff::models::*;
use crate::modules::auth::models::User;

use crate::utils::subdomain::extract_subdomain_from_request;
use crate::utils::field_encryption;
use crate::AppState;
use crate::error::AppError;
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
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
    username: String,
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
    category: Option<String>,
    org_type: Option<String>,
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
) -> Result<User, AppError> {
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
    let token = token_from_header.or(token_from_cookie)
        .ok_or(AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string()))?;

    
    // Verify token
    let claims = crate::utils::jwt::JwtService::verify_token(&token)
        .map_err(|_| AppError::AuthError("Token ไม่ถูกต้อง".to_string()))?;
    
    // Get user from database
    let mut user: User = sqlx::query_as(
        "SELECT 
            id,
            username,
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
    .map_err(|e| {
        eprintln!("❌ Failed to get user: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลผู้ใช้ได้".to_string())
    })?;
    
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
        Ok(false) => Err(AppError::Forbidden(format!("คุณไม่มีสิทธิ์ (ต้องการ {} permission)", required_permission))),
        Err(e) => {
            eprintln!("❌ Permission check error: {}", e);
            Err(AppError::InternalServerError("เกิดข้อผิดพลาดในการตรวจสอบสิทธิ์".to_string()))
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
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;
    
    // Check permission
    let auth_result = check_user_permission(&headers, &pool, "staff.read.all").await;
    match auth_result {
        Ok(_) => {},
        Err(_) => {
            check_user_permission(&headers, &pool, "achievement.create.all").await?;
        }
    };

    let page = filter.page.unwrap_or(1);
    let page_size = filter.page_size.unwrap_or(20);
    let offset = (page - 1) * page_size;

    let mut query = String::from(
        "SELECT DISTINCT u.id, u.username, u.title, u.first_name, u.last_name, u.status
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
            " AND (u.first_name ILIKE '%{}%' OR u.last_name ILIKE '%{}%' OR u.username ILIKE '%{}%')",
            search, search, search
        ));
    }

    query.push_str(&format!(
        " ORDER BY u.first_name LIMIT {} OFFSET {}",
        page_size, offset
    ));

    let staff_rows = sqlx::query_as::<_, (Uuid, String, Option<String>, String, String, String)>(&query)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
        })?;

    let count_query = "SELECT COUNT(DISTINCT u.id) FROM users u WHERE u.user_type = 'staff'";
    let total: i64 = sqlx::query_scalar(count_query).fetch_one(&pool).await.unwrap_or(0);

    let total_pages = (total as f64 / page_size as f64).ceil() as i64;

    let items: Vec<StaffListItem> = staff_rows
        .into_iter()
        .map(|(id, username, title, first_name, last_name, status)| StaffListItem {
            id,
            username,
            title: title.unwrap_or_default(),
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

    Ok((StatusCode::OK, Json(response)))
}

// ===================================================================
// Get Staff Profile
// ===================================================================

pub async fn get_staff_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(staff_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;
    
    // Check permission
    check_user_permission(&headers, &pool, "staff.read.all").await?;

    // Get user basic info (encryption key auto-set by pool)
    let mut user = sqlx::query_as::<_, UserBasicRow>(
        "SELECT id, username, national_id, email, title, first_name, last_name, nickname, phone, 
                emergency_contact, line_id, date_of_birth, gender, address, hired_date,
                user_type, status, profile_image_url
         FROM users 
         WHERE id = $1 AND user_type = 'staff'",
    )
    .bind(staff_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?
    .ok_or(AppError::NotFound("ไม่พบบุคลากร".to_string()))?;
    
    // Decrypt national_id
    if let Some(nid) = &user.national_id {
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
        "SELECT d.id, d.code, d.name, d.category, d.org_type, dm.position, dm.is_primary_department
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
        category: row.category,
        org_type: row.org_type,
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
        username: user.username,
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

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": profile
        })),
    ))
}

// ===================================================================
// Create Staff
// ===================================================================

pub async fn create_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateStaffRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;
    
    // Check permission
    check_user_permission(&headers, &pool, "staff.create.all").await?;

    // Hash password (encryption key auto-set by pool)
    let password_hash = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST)
        .map_err(|e| {
            eprintln!("❌ Password hashing failed: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาดในการสร้างรหัสผ่าน".to_string())
        })?;

    let mut tx = pool.begin().await.map_err(|e| {
        eprintln!("❌ Failed to start transaction: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

    // Encrypt national_id
    let encrypted_national_id = field_encryption::encrypt_optional(payload.national_id.as_deref())
        .map_err(|e| {
             eprintln!("Encryption failed: {}", e);
             AppError::InternalServerError("Encryption error".to_string())
        })?;

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

    // Create new user
    let user_id: Uuid = sqlx::query_scalar(
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

    .map_err(|e| {
        eprintln!("❌ Failed to create user: {}", e);
        if e.to_string().contains("duplicate key value violates unique constraint") {
             if e.to_string().contains("users_username_key") {
                AppError::BadRequest("รหัสผู้ใช้งาน (Username) นี้มีอยู่ในระบบแล้ว กรุณาใช้รหัสอื่น".to_string())
             } else if e.to_string().contains("users_national_id_hash_key") {
                 AppError::BadRequest("รหัสบัตรประชาชนนี้มีอยู่ในระบบแล้ว".to_string())
             } else if e.to_string().contains("users_email_key") {
                  AppError::BadRequest("อีเมลนี้มีอยู่ในระบบแล้ว".to_string())
             } else {
                 AppError::BadRequest("ข้อมูลบางอย่างซ้ำกับที่มีในระบบ".to_string())
             }
        } else {
            AppError::InternalServerError("ไม่สามารถสร้างบุคลากรได้".to_string())
        }
    })?;

    // Create staff info (if provided)
    if let Some(staff_info) = &payload.staff_info {
        sqlx::query(
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
        .map_err(|e| {
            eprintln!("❌ Failed to create staff info: {}", e);
            AppError::InternalServerError("ไม่สามารถสร้างข้อมูลบุคลากรได้".to_string())
        })?;
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
            return Err(AppError::BadRequest(format!("มีบทบาทที่ไม่ถูกต้องสำหรับบุคลากร: {:?}", invalid_roles)));
        }
    }

    // Assign roles
    for role_id in &payload.role_ids {
        let is_primary = payload.primary_role_id.as_ref() == Some(role_id);

        sqlx::query(
            "INSERT INTO user_roles (user_id, role_id, is_primary, started_at)
             VALUES ($1, $2, $3, CURRENT_DATE)",
        )
        .bind(user_id)
        .bind(role_id)
        .bind(is_primary)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to assign role: {}", e);
             AppError::InternalServerError("ไม่สามารถบันทึกบทบาทได้".to_string())
        })?;
    }

    // Assign departments
    if let Some(dept_assignments) = &payload.department_assignments {
        for assignment in dept_assignments {
            sqlx::query(
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
            .await
            .map_err(|e| {
                eprintln!("❌ Failed to assign department: {}", e);
                 AppError::InternalServerError("ไม่สามารถบันทึกแผนกได้".to_string())
            })?;
        }
    }

    tx.commit().await.map_err(|e| {
        eprintln!("❌ Failed to commit transaction: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการบันทึกข้อมูล".to_string())
    })?;

    println!("✅ Staff created successfully: {}", user_id);
    Ok((
        StatusCode::CREATED,
        Json(json!({
            "success": true,
            "message": "สร้างบุคลากรสำเร็จ",
            "data": {
                "id": user_id
            }
        })),
    ))
}

// ===================================================================
// Update Staff
// ===================================================================

pub async fn update_staff(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(staff_id): Path<Uuid>,
    Json(payload): Json<UpdateStaffRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;
    
    // Check permission
    check_user_permission(&headers, &pool, "staff.update.all").await?;

    let mut tx = pool.begin().await.map_err(|e| {
        eprintln!("❌ Failed to start transaction: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
    })?;

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
                    sqlx::query(
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
                    .await
                    .map_err(|e| {
                        eprintln!("❌ Failed to update staff_info: {}", e);
                        AppError::InternalServerError("ไม่สามารถอัพเดตข้อมูลบุคลากรได้".to_string())
                    })?;
                } else {
                    // Create new record
                    sqlx::query(
                        "INSERT INTO staff_info (user_id, education_level, major, university, metadata)
                         VALUES ($1, $2, $3, $4, '{}'::jsonb)",
                    )
                    .bind(staff_id)
                    .bind(&staff_info.education_level)
                    .bind(&staff_info.major)
                    .bind(&staff_info.university)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| {
                        eprintln!("❌ Failed to create staff_info: {}", e);
                        AppError::InternalServerError("ไม่สามารถสร้างข้อมูลบุคลากรได้".to_string())
                    })?;
                }
            }

            // Update roles if provided
            if let Some(role_ids) = &payload.role_ids {
                // Delete existing roles
                sqlx::query("DELETE FROM user_roles WHERE user_id = $1")
                    .bind(staff_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| {
                        eprintln!("❌ Failed to delete roles: {}", e);
                        AppError::InternalServerError("ไม่สามารถอัพเดตบทบาทได้".to_string())
                    })?;

                // Insert new roles
                for role_id in role_ids {
                    let is_primary = payload.primary_role_id.as_ref() == Some(role_id);

                    sqlx::query(
                        "INSERT INTO user_roles (user_id, role_id, is_primary, started_at)
                         VALUES ($1, $2, $3, CURRENT_DATE)",
                    )
                    .bind(staff_id)
                    .bind(role_id)
                    .bind(is_primary)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| {
                        eprintln!("❌ Failed to insert role: {}", e);
                        AppError::InternalServerError("ไม่สามารถเพิ่มบทบาทได้".to_string())
                    })?;
                }
            }

            // Update departments if provided
            if let Some(dept_assignments) = &payload.department_assignments {
                // Delete existing department assignments
                sqlx::query("DELETE FROM department_members WHERE user_id = $1")
                    .bind(staff_id)
                    .execute(&mut *tx)
                    .await
                    .map_err(|e| {
                        eprintln!("❌ Failed to delete department members: {}", e);
                        AppError::InternalServerError("ไม่สามารถอัพเดตแผนกได้".to_string())
                    })?;

                // Insert new department assignments
                for assignment in dept_assignments {
                    sqlx::query(
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
                    .await
                    .map_err(|e| {
                       eprintln!("❌ Failed to insert department member: {}", e);
                       AppError::InternalServerError("ไม่สามารถเพิ่มแผนกได้".to_string())
                    })?;
                }
            }

            tx.commit().await.map_err(|e| {
                eprintln!("❌ Failed to commit transaction: {}", e);
                AppError::InternalServerError("เกิดข้อผิดพลาดในการบันทึกข้อมูล".to_string())
            })?;
            
            Ok((
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "อัปเดตข้อมูลสำเร็จ"
                })),
            ))
        }
        Ok(_) => {
            let _ = tx.rollback().await;
            Err(AppError::NotFound("ไม่พบบุคลากร".to_string()))
        }
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            let _ = tx.rollback().await;
            Err(AppError::InternalServerError("เกิดข้อผิดพลาดในการอัปเดตข้อมูล".to_string()))
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
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;
    
    // Check permission
    check_user_permission(&headers, &pool, "staff.delete.all").await?;

    let result = sqlx::query(
        "UPDATE users 
         SET status = 'inactive', updated_at = NOW()
         WHERE id = $1 AND user_type = 'staff'",
    )
    .bind(staff_id)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการลบบุคลากร".to_string())
    })?;

    if result.rows_affected() > 0 {
        Ok((
            StatusCode::OK,
            Json(json!({
                "success": true,
                "message": "ลบบุคลากรสำเร็จ"
            })),
        ))
    } else {
        Err(AppError::NotFound("ไม่พบบุคลากร".to_string()))
    }
}

pub async fn get_public_staff_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(staff_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;

    // Authentication Only
    let auth_header = headers.get(header::AUTHORIZATION).and_then(|h| h.to_str().ok());
    let token_from_header = auth_header.and_then(|h| if h.starts_with("Bearer ") { Some(h[7..].to_string()) } else { None });
    let token_from_cookie = headers.get(header::COOKIE).and_then(|h| h.to_str().ok()).and_then(|cookie| crate::utils::jwt::JwtService::extract_token_from_cookie(Some(cookie)));
    
    let token = token_from_header.or(token_from_cookie)
        .ok_or(AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string()))?;
    
    if let Err(_) = crate::utils::jwt::JwtService::verify_token(&token) {
        return Err(AppError::AuthError("Token ไม่ถูกต้อง".to_string()));
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
    let user_rec = sqlx::query_as::<_, PublicUserRow>(
        "SELECT id, first_name, last_name, nickname, email, user_type, status, profile_image_url, title, phone, hired_date
         FROM users WHERE id = $1 AND user_type = 'staff'"
    )
    .bind(staff_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Database error (user): {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
    })?
    .ok_or(AppError::NotFound("ไม่พบบุคลากร".to_string()))?;

    #[derive(sqlx::FromRow)]
    struct PublicRoleRow {
        id: Uuid,
        code: String,
        name: String,
        level: Option<i32>,
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

    Ok((StatusCode::OK, Json(json!({ "success": true, "data": response_data }))))
}
