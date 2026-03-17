use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::db::school_mapping::get_school_database_url;
use crate::modules::auth::models::User;
use crate::modules::auth::permissions::UserPermissions;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::utils::field_encryption;
use crate::AppState;
use crate::error::AppError;
use super::models::{CreateStudentRequest, ListStudentsQuery, StudentListItem, StudentProfile, StudentDbRow, UpdateOwnProfileRequest, UpdateStudentRequest};

// =========================================
// Helper Functions
// =========================================

/// Extract user from JWT token
async fn get_current_user(headers: &HeaderMap, pool: &sqlx::PgPool) -> Result<User, AppError> {
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
    if let Some(ref nid) = user.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            user.national_id = Some(dec);
        }
    }
    
    Ok(user)
}

/// Check if user has permission
async fn check_user_permission(
    headers: &HeaderMap,
    pool: &sqlx::PgPool,
    required_permission: &str,
) -> Result<User, AppError> {
    let user = get_current_user(headers, pool).await?;
    
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

// =========================================
// Student Self-Service Handlers
// =========================================

/// GET /api/student/profile - นักเรียนดูข้อมูลตนเอง
pub async fn get_own_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
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
    
    // Get current user
    let user = get_current_user(&headers, &pool).await?;
    
    // Query student profile
    let mut student_row = sqlx::query_as::<_, StudentDbRow>(
        r#"
        SELECT 
            u.id, u.username, u.national_id as national_id, u.email, u.first_name, u.last_name,
            u.title, u.nickname, u.phone, u.date_of_birth, u.gender,
            u.address, u.profile_image_url,
            s.student_id, 
            CASE gl.level_type 
                WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                WHEN 'primary' THEN CONCAT('ป.', gl.year)
                WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                ELSE CONCAT('?.', gl.year)
            END as grade_level, 
            c.name as class_room, 
            sce.class_number as student_number,
            s.blood_type, s.allergies, s.medical_conditions as medical_conditions,
            u.status
        FROM users u
        LEFT JOIN student_info s ON u.id = s.user_id
        LEFT JOIN LATERAL (
            SELECT student_id, class_room_id, class_number
            FROM student_class_enrollments 
            WHERE student_id = u.id 
            ORDER BY created_at DESC 
            LIMIT 1
        ) sce ON true
        LEFT JOIN class_rooms c ON sce.class_room_id = c.id
        LEFT JOIN grade_levels gl ON c.grade_level_id = gl.id
        WHERE u.id = $1 AND u.user_type = 'student' AND u.status = 'active'
        "#
    )
    .bind(user.id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to get student profile: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลนักเรียนได้".to_string())
    })?
    .ok_or(AppError::NotFound("Student not found".to_string()))?;

    // Decrypt fields
    // Decrypt fields
    // Decrypt fields
    // Decrypt fields
    if let Some(nid) = &student_row.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            student_row.national_id = Some(dec);
        }
    }
    if let Some(mc) = &student_row.medical_conditions {
        if let Ok(dec) = field_encryption::decrypt(mc) {
            student_row.medical_conditions = Some(dec);
        }
    }

    // Fetch parents for own profile too (if needed, currently not showing in own profile but good to have)
    let parents = sqlx::query_as::<_, crate::modules::students::models::ParentDto>(
        r#"
        SELECT 
            p.id, p.username, p.first_name, p.last_name, p.phone,
            sp.relationship, sp.is_primary
        FROM student_parents sp
        INNER JOIN users p ON sp.parent_user_id = p.id
        WHERE sp.student_user_id = $1
        ORDER BY sp.is_primary DESC, p.first_name ASC
        "#
    )
    .bind(user.id)
    .fetch_all(&pool)
    .await
    .unwrap_or_else(|_| Vec::new());
    
    let student = StudentProfile {
        info: student_row,
        parents,
    };
    
    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": student
        })),
    ))
}

/// PUT /api/student/profile - นักเรียนแก้ไขข้อมูลตนเอง (จำกัดฟิลด์)
pub async fn update_own_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateOwnProfileRequest>,
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
    
    // Get current user
    let user = get_current_user(&headers, &pool).await?;
    
    // Update only allowed fields
    sqlx::query(
        r#"
        UPDATE users
        SET 
            phone = COALESCE($2, phone),
            address = COALESCE($3, address),
            nickname = COALESCE($4, nickname),
            updated_at = NOW()
        WHERE id = $1 AND user_type = 'student'
        "#
    )
    .bind(user.id)
    .bind(&payload.phone)
    .bind(&payload.address)
    .bind(&payload.nickname)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to update profile: {}", e);
        AppError::InternalServerError("ไม่สามารถอัพเดตข้อมูลได้".to_string())
    })?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "อัพเดตข้อมูลสำเร็จ"
        })),
    ))
}

// =========================================
// Admin/Staff Student Management Handlers
// =========================================

/// GET /api/students - รายชื่อนักเรียนทั้งหมด
pub async fn list_students(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<ListStudentsQuery>,
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
    check_user_permission(&headers, &pool, "student.read.all").await?;
    
    let page = filter.page.unwrap_or(1);
    let page_size = filter.page_size.unwrap_or(20).min(100);
    let offset = (page - 1) * page_size;
    
    // Build query with filters
    // JOIN with enrollments, class_rooms, and grade_levels to get current academic info
    let mut query = String::from(
        r#"
        SELECT 
            u.id, u.username, u.title, u.first_name, u.last_name,
            s.student_id, 
            CASE gl.level_type 
                WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                WHEN 'primary' THEN CONCAT('ป.', gl.year)
                WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                ELSE CONCAT('?.', gl.year)
            END as grade_level, 
            c.name as class_room,
            u.status
        FROM users u
        INNER JOIN student_info s ON u.id = s.user_id
        LEFT JOIN student_class_enrollments sce ON u.id = sce.student_id AND sce.status = 'active'
        LEFT JOIN class_rooms c ON sce.class_room_id = c.id
        LEFT JOIN grade_levels gl ON c.grade_level_id = gl.id
        WHERE u.user_type = 'student'
        "#
    );
    
    let mut conditions = Vec::new();
    


    if let Some(ref status) = filter.status {
        conditions.push(format!("u.status = '{}'", status));
    }
    
    if let Some(ref search) = filter.search {
        conditions.push(format!(
            "(u.first_name ILIKE '%{}%' OR u.last_name ILIKE '%{}%' OR s.student_id ILIKE '%{}%' OR u.username ILIKE '%{}%')",
            search, search, search, search
        ));
    }
    
    if !conditions.is_empty() {
        query.push_str(" AND ");
        query.push_str(&conditions.join(" AND "));
    }
    
    query.push_str(" ORDER BY CASE gl.level_type WHEN 'kindergarten' THEN 1 WHEN 'primary' THEN 2 WHEN 'secondary' THEN 3 ELSE 4 END, gl.year NULLS LAST, c.name NULLS LAST, s.student_number");
    query.push_str(&format!(" LIMIT {} OFFSET {}", page_size, offset));
    
    let students = sqlx::query_as::<_, StudentListItem>(&query)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Database error: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
        })?;
    
    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": students,
            "page": page,
            "page_size": page_size
        })),
    ))
}

/// POST /api/students - เพิ่มนักเรียนใหม่
pub async fn create_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateStudentRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Debug: Log incoming request
    eprintln!("🔍 Creating student with payload: {:?}", payload);
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
    check_user_permission(&headers, &pool, "student.create").await?;
    
    let mut tx = pool.begin().await.map_err(|e| {
        eprintln!("❌ Failed to start transaction: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการเริ่ม Transaction".to_string())
    })?;
    
    // 1. Hash password
    let password_hash = bcrypt::hash(&payload.password, bcrypt::DEFAULT_COST).map_err(|e| {
        eprintln!("❌ Password hashing failed: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการสร้างรหัสผ่าน".to_string())
    })?;
    
    // Parse date_of_birth if provided
    let date_of_birth = match &payload.date_of_birth {
        Some(date_str) if !date_str.is_empty() => {
            chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").map(Some).map_err(|e| {
                eprintln!("❌ Invalid date format: {} (error: {})", date_str, e);
                AppError::BadRequest("รูปแบบวันเกิดไม่ถูกต้อง (ต้องเป็น YYYY-MM-DD)".to_string())
            })?
        }
        _ => None,
    };
    
    
    // Encrypt national_id if provided
    let (encrypted_national_id, national_id_hash) = if let Some(nid) = &payload.national_id {
        if !nid.is_empty() {
             let enc = field_encryption::encrypt(nid).map_err(|e| {
                eprintln!("❌ Encryption failed: {}", e);
                AppError::InternalServerError("เกิดข้อผิดพลาดในการประมวลผลข้อมูล".to_string())
             })?;
            let hash = field_encryption::hash_for_search(nid);
            (Some(enc), Some(hash))
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };
    
    // Determine Username
    // Priority: Supplied Username > Student ID
    let username = if let Some(u) = &payload.username {
         if !u.is_empty() { u.clone() } else { payload.student_id.clone() }
    } else {
         payload.student_id.clone()
    };

    // 2. Create user
    let user_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO users (
            username, national_id, national_id_hash, email, password_hash,
            first_name, last_name, title, 
            user_type, status, date_of_birth, gender
        ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'student', 'active', $9, $10)
        RETURNING id
        "#
    )
    .bind(&username)
    .bind(&encrypted_national_id)
    .bind(&national_id_hash)
    .bind(&payload.email)
    .bind(&password_hash)
    .bind(&payload.first_name)
    .bind(&payload.last_name)
    .bind(&payload.title)
    .bind(&date_of_birth) // Use parsed date
    .bind(&payload.gender)
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
            AppError::InternalServerError("ไม่สามารถสร้างผู้ใช้งานได้".to_string())
        }
    })?;

    // 2.5 Handle Parents Creation
    if let Some(parents) = &payload.parents {
        for (index, parent) in parents.iter().enumerate() {
            // Check if parent exists by phone (username)
            let existing_parent = sqlx::query_scalar::<_, Uuid>(
                "SELECT id FROM users WHERE username = $1"
            )
            .bind(&parent.phone)
            .fetch_optional(&mut *tx)
            .await
            .map_err(|e| {
                 eprintln!("❌ Failed to check for existing parent: {}", e);
                 AppError::InternalServerError("เกิดข้อผิดพลาดในการตรวจสอบผู้ปกครอง".to_string())
            })?;

            let parent_id = if let Some(pid) = existing_parent {
                pid
            } else {
                 // Create new parent user
                 let parent_password_hash = bcrypt::hash(&parent.phone, bcrypt::DEFAULT_COST).map_err(|e| {
                    eprintln!("❌ Parent password hashing failed: {}", e);
                    AppError::InternalServerError("เกิดข้อผิดพลาดในการสร้างรหัสผ่านผู้ปกครอง".to_string())
                })?;
                
                // Encrypt parent national id if provided
                 let (parent_enc_nid, parent_nid_hash) = if let Some(nid) = &parent.national_id {
                    if !nid.is_empty() {
                         let enc = field_encryption::encrypt(nid).map_err(|e| {
                            eprintln!("❌ Parent Encryption failed: {}", e);
                            AppError::InternalServerError("เกิดข้อผิดพลาดในการประมวลผลข้อมูลผู้ปกครอง".to_string())
                         })?;
                        let hash = field_encryption::hash_for_search(nid);
                        (Some(enc), Some(hash))
                    } else {
                        (None, None)
                    }
                } else {
                    (None, None)
                };

                let new_parent_id: Uuid = sqlx::query_scalar(
                    r#"
                    INSERT INTO users (
                        username, national_id, national_id_hash, email, password_hash,
                        first_name, last_name, phone,
                        user_type, status
                    ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'parent', 'active')
                    RETURNING id
                    "#
                )
                .bind(&parent.phone) // Username as phone
                .bind(parent_enc_nid)
                .bind(parent_nid_hash)
                .bind(&parent.email)
                .bind(parent_password_hash)
                .bind(&parent.first_name)
                .bind(&parent.last_name)
                .bind(&parent.phone)
                .fetch_one(&mut *tx)
                .await
                .map_err(|e| {
                     eprintln!("❌ Failed to create parent: {}", e);
                     AppError::InternalServerError("ไม่สามารถสร้างบัญชีผู้ปกครองได้".to_string())
                })?;
                
                 // Assign PARENT role
                 let parent_role_id: Option<Uuid> = sqlx::query_scalar(
                    "SELECT id FROM roles WHERE code = 'PARENT' AND is_active = true"
                )
                .fetch_optional(&mut *tx)
                .await
                .ok()
                .flatten();

                if let Some(rid) = parent_role_id {
                     let _ = sqlx::query(
                        r#"
                        INSERT INTO user_roles (user_id, role_id, is_primary)
                        VALUES ($1, $2, true)
                        "#
                    )
                    .bind(new_parent_id)
                    .bind(rid)
                    .execute(&mut *tx)
                    .await;
                }

                new_parent_id
            };

            // Link parent to student
            // First parent in list is primary by default, or you can add checks
            let is_primary = index == 0; 
            
            sqlx::query(
                r#"
                INSERT INTO student_parents (student_user_id, parent_user_id, relationship, is_primary)
                VALUES ($1, $2, $3, $4)
                ON CONFLICT (student_user_id, parent_user_id) DO NOTHING
                "#
            )
            .bind(user_id)
            .bind(parent_id)
            .bind(&parent.relationship)
            .bind(is_primary)
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                 eprintln!("❌ Failed to link parent: {}", e);
                 AppError::InternalServerError("ไม่สามารถเชื่อมโยงผู้ปกครองได้".to_string())
            })?;
        }
    }
    
    // 3. Create student_info
    sqlx::query(
        r#"
        INSERT INTO student_info (
            user_id, student_id, student_number
        ) VALUES ($1, $2, $3)
        "#
    )
    .bind(user_id)
    .bind(&payload.student_id)
    .bind(&payload.student_number)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to create student_info: {}", e);
        AppError::InternalServerError("ไม่สามารถสร้างข้อมูลนักเรียนได้".to_string())
    })?;



    // 4. Assign STUDENT role
    let student_role_id: Option<Uuid> = sqlx::query_scalar(
        "SELECT id FROM roles WHERE code = 'STUDENT' AND is_active = true"
    )
    .fetch_optional(&mut *tx)
    .await
    .ok()
    .flatten();
    
    if let Some(role_id) = student_role_id {
        let _ = sqlx::query(
            r#"
            INSERT INTO user_roles (user_id, role_id, is_primary)
            VALUES ($1, $2, true)
            "#
        )
        .bind(user_id)
        .bind(role_id)
        .execute(&mut *tx)
        .await;
    }
    
    // Commit transaction
    tx.commit().await.map_err(|e| {
        eprintln!("❌ Failed to commit transaction: {}", e);
        AppError::InternalServerError("ไม่สามารถบันทึกข้อมูลได้".to_string())
    })?;

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "success": true,
            "id": user_id,
            "username": username,
            "message": "เพิ่มนักเรียนสำเร็จ"
        })),
    ))
}

/// GET /api/students/:id - ดูข้อมูลนักเรียน
pub async fn get_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
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
    check_user_permission(&headers, &pool, "student.read.all").await?;
    
    // Query student profile with joined class info
    let mut student_row = sqlx::query_as::<_, StudentDbRow>(
        r#"
        SELECT 
            u.id, u.username, u.national_id as national_id, u.email, u.first_name, u.last_name,
            u.title, u.nickname, u.phone, u.date_of_birth, u.gender,
            u.address, u.profile_image_url, u.status,
            s.student_id, 
            CASE gl.level_type 
                WHEN 'kindergarten' THEN CONCAT('อ.', gl.year)
                WHEN 'primary' THEN CONCAT('ป.', gl.year)
                WHEN 'secondary' THEN CONCAT('ม.', gl.year)
                ELSE CONCAT('?.', gl.year)
            END as grade_level, 
            c.name as class_room, 
            sce.class_number as student_number,
            s.blood_type, s.allergies, s.medical_conditions as medical_conditions
        FROM users u
        LEFT JOIN student_info s ON u.id = s.user_id
        LEFT JOIN student_class_enrollments sce ON u.id = sce.student_id AND sce.status = 'active'
        LEFT JOIN class_rooms c ON sce.class_room_id = c.id
        LEFT JOIN grade_levels gl ON c.grade_level_id = gl.id
        WHERE u.id = $1 AND u.user_type = 'student'
        "#
    )
    .bind(student_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Database error: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
    })?
    .ok_or(AppError::NotFound("Student not found".to_string()))?;

    // Fetch parents for this student
    let parents = sqlx::query_as::<_, crate::modules::students::models::ParentDto>(
        r#"
        SELECT 
            p.id, p.username, p.first_name, p.last_name, p.phone,
            sp.relationship, sp.is_primary
        FROM student_parents sp
        INNER JOIN users p ON sp.parent_user_id = p.id
        WHERE sp.student_user_id = $1
        ORDER BY sp.is_primary DESC, p.first_name ASC
        "#
    )
    .bind(student_id)
    .fetch_all(&pool)
    .await
    .unwrap_or_else(|_| Vec::new());
    
    // student_row.parents = parents; // Removed, StudentDbRow doesn't have parents

    // Decrypt fields
    // Decrypt fields
    if let Some(nid) = &student_row.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            student_row.national_id = Some(dec);
        }
    }
    if let Some(mc) = &student_row.medical_conditions {
        if let Ok(dec) = field_encryption::decrypt(mc) {
            student_row.medical_conditions = Some(dec);
        }
    }
    
    let student = StudentProfile {
        info: student_row,
        parents,
    };
    
    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": student
        })),
    ))
}

/// PUT /api/students/:id - แก้ไขข้อมูลนักเรียน
pub async fn update_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
    Json(payload): Json<UpdateStudentRequest>,
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
    check_user_permission(&headers, &pool, "student.update.all").await?;
    
    let mut tx = pool.begin().await.map_err(|e| {
        eprintln!("❌ Failed to start transaction: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการเริ่ม Transaction".to_string())
    })?;
    
    // Update users table
    sqlx::query(
        r#"
        UPDATE users
        SET 
            email = COALESCE($2, email),
            first_name = COALESCE($3, first_name),
            last_name = COALESCE($4, last_name),
            phone = COALESCE($5, phone),
            address = COALESCE($6, address),
            updated_at = NOW()
        WHERE id = $1 AND user_type = 'student'
        "#
    )
    .bind(student_id)
    .bind(&payload.email)
    .bind(&payload.first_name)
    .bind(&payload.last_name)
    .bind(&payload.phone)
    .bind(&payload.address)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to update user: {}", e);
        AppError::InternalServerError("ไม่สามารถอัพเดตข้อมูลได้".to_string())
    })?;
    
    // Update student_info table
    sqlx::query(
        r#"
        UPDATE student_info
        SET 
            student_number = COALESCE($2, student_number),
            updated_at = NOW()
        WHERE user_id = $1
        "#
    )
    .bind(student_id)
    .bind(&payload.student_number)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to update student_info: {}", e);
        AppError::InternalServerError("ไม่สามารถอัพเดตข้อมูลได้".to_string())
    })?;
    
    tx.commit().await.map_err(|e| {
        eprintln!("❌ Failed to commit transaction: {}", e);
        AppError::InternalServerError("ไม่สามารถบันทึกข้อมูลได้".to_string())
    })?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "อัพเดตข้อมูลนักเรียนสำเร็จ"
        })),
    ))
}

/// DELETE /api/students/:id - ลบนักเรียน (soft delete)
pub async fn delete_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
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
    check_user_permission(&headers, &pool, "student.delete").await?;
    
    // Soft delete — wrap in transaction to also drop enrollments
    let mut tx = pool.begin().await
        .map_err(|e| {
            eprintln!("❌ Failed to begin transaction: {}", e);
            AppError::InternalServerError("ไม่สามารถลบนักเรียนได้".to_string())
        })?;

    sqlx::query(
        r#"
        UPDATE users
        SET status = 'inactive',
            username = username || '_del_' || CAST(EXTRACT(EPOCH FROM NOW()) AS BIGINT),
            updated_at = NOW()
        WHERE id = $1 AND user_type = 'student'
        "#
    )
    .bind(student_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to delete student: {}", e);
        AppError::InternalServerError("ไม่สามารถลบนักเรียนได้".to_string())
    })?;

    sqlx::query(
        r#"
        UPDATE student_class_enrollments
        SET status = 'dropped', updated_at = NOW()
        WHERE student_id = $1 AND status = 'active'
        "#
    )
    .bind(student_id)
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to drop enrollments: {}", e);
        AppError::InternalServerError("ไม่สามารถลบนักเรียนได้".to_string())
    })?;

    tx.commit().await
        .map_err(|e| {
            eprintln!("❌ Failed to commit transaction: {}", e);
            AppError::InternalServerError("ไม่สามารถลบนักเรียนได้".to_string())
        })?;

    Ok((
        StatusCode::OK,
        Json(json!({
            "success": true,
            "message": "ลบนักเรียนสำเร็จ"
        })),
    ))
}
