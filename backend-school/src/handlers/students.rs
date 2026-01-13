/// Student Management Handlers (Axum Framework)
/// Provides APIs for student self-service and admin student management
use axum::{
    extract::{Path, Query, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::FromRow;
use uuid::Uuid;

use crate::db::school_mapping::get_school_database_url;
use crate::models::auth::User;
use crate::models::staff::UserPermissions; // For has_permission method
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::utils::field_encryption;
use crate::AppState;

// =========================================
// Response Structures
// =========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct StudentProfile {
    // User fields
    pub id: Uuid,
    pub username: String, // Add username
    pub national_id: Option<String>,
    pub email: Option<String>,
    pub first_name: String,
    pub last_name: String,
    pub title: Option<String>,
    pub nickname: Option<String>,
    pub phone: Option<String>,
    pub date_of_birth: Option<chrono::NaiveDate>,
    pub gender: Option<String>,
    pub address: Option<String>,
    pub profile_image_url: Option<String>,
    
    // Student info fields
    pub student_id: Option<String>,
    pub grade_level: Option<String>,
    pub class_room: Option<String>,
    pub student_number: Option<i32>,
    pub blood_type: Option<String>,
    pub allergies: Option<String>,
    pub medical_conditions: Option<String>,
}

#[derive(Debug, Serialize, FromRow)]
pub struct StudentListItem {
    pub id: Uuid,
    pub username: String, // Add username
    pub first_name: String,
    pub last_name: String,
    pub student_id: Option<String>,
    pub grade_level: Option<String>,
    pub class_room: Option<String>,
    pub status: String,
}

// =========================================
// Request Structures
// =========================================

#[derive(Debug, Deserialize)]
pub struct UpdateOwnProfileRequest {
    pub phone: Option<String>,
    pub address: Option<String>,
    pub nickname: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateStudentRequest {
    pub national_id: Option<String>,
    pub username: Option<String>, // Optional, will be generated if not provided
    pub email: Option<String>,
    pub password: String,
    pub first_name: String,
    pub last_name: String,
    pub title: Option<String>,
    pub student_id: String,
    pub grade_level: Option<String>,
    pub class_room: Option<String>,
    pub student_number: Option<i32>,
    pub date_of_birth: Option<String>, // Changed from NaiveDate to String for flexible parsing
    pub gender: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStudentRequest {
    pub email: Option<String>,
    pub first_name: Option<String>,
    pub last_name: Option<String>,
    pub phone: Option<String>,
    pub address: Option<String>,
    pub grade_level: Option<String>,
    pub class_room: Option<String>,
    pub student_number: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct ListStudentsQuery {
    pub page: Option<i64>,
    pub page_size: Option<i64>,
    pub grade_level: Option<String>,
    pub class_room: Option<String>,
    pub search: Option<String>,
}

// =========================================
// Helper Functions
// =========================================

/// Extract user from JWT token
async fn get_current_user(headers: &HeaderMap, pool: &sqlx::PgPool) -> Result<User, Response> {
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
) -> Result<User, Response> {
    let user = get_current_user(headers, pool).await?;
    
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

// =========================================
// Student Self-Service Handlers
// =========================================

/// GET /api/student/profile - นักเรียนดูข้อมูลตนเอง
pub async fn get_own_profile(
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
    
    // Get current user
    let user = match get_current_user(&headers, &pool).await {
        Ok(u) => u,
        Err(response) => return response,
    };
    
    // Query student profile
    let mut student = match sqlx::query_as::<_, StudentProfile>(
        r#"
        SELECT 
            u.id, u.username, u.national_id as national_id, u.email, u.first_name, u.last_name,
            u.title, u.nickname, u.phone, u.date_of_birth, u.gender,
            u.address, u.profile_image_url,
            s.student_id, s.grade_level, s.class_room, s.student_number,
            s.blood_type, s.allergies, s.medical_conditions as medical_conditions
        FROM users u
        LEFT JOIN student_info s ON u.id = s.user_id
        WHERE u.id = $1 AND u.user_type = 'student' AND u.status = 'active'
        "#
    )
    .bind(user.id)
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": "Student not found"
                })),
            ).into_response();
        }
        Err(e) => {
            eprintln!("❌ Failed to get student profile: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถดึงข้อมูลนักเรียนได้"
                })),
            ).into_response();
        }
    };

    // Decrypt fields
    if let Some(ref nid) = student.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            student.national_id = Some(dec);
        }
    }
    if let Some(ref mc) = student.medical_conditions {
        if let Ok(dec) = field_encryption::decrypt(mc) {
            student.medical_conditions = Some(dec);
        }
    }
    
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": student
        })),
    ).into_response()
}

/// PUT /api/student/profile - นักเรียนแก้ไขข้อมูลตนเอง (จำกัดฟิลด์)
pub async fn update_own_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateOwnProfileRequest>,
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
    
    // Get current user
    let user = match get_current_user(&headers, &pool).await {
        Ok(u) => u,
        Err(response) => return response,
    };
    
    // Update only allowed fields
    match sqlx::query(
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
    {
        Ok(_) => {
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "อัพเดตข้อมูลสำเร็จ"
                })),
            ).into_response()
        }
        Err(e) => {
            eprintln!("❌ Failed to update profile: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถอัพเดตข้อมูลได้"
                })),
            ).into_response()
        }
    }
}

// =========================================
// Admin/Staff Student Management Handlers
// =========================================

/// GET /api/students - รายชื่อนักเรียนทั้งหมด
pub async fn list_students(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<ListStudentsQuery>,
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
    
    // Check permission
    let _user = match check_user_permission(&headers, &pool, "student.read.all").await {
        Ok(u) => u,
        Err(response) => return response,
    };
    
    let page = filter.page.unwrap_or(1);
    let page_size = filter.page_size.unwrap_or(20).min(100);
    let offset = (page - 1) * page_size;
    
    // Build query with filters
    let mut query = String::from(
        r#"
        SELECT 
            u.id, u.username, u.first_name, u.last_name,
            s.student_id, s.grade_level, s.class_room,
            u.status
        FROM users u
        INNER JOIN student_info s ON u.id = s.user_id
        WHERE u.user_type = 'student'
        "#
    );
    
    let mut conditions = Vec::new();
    
    if let Some(ref grade_level) = filter.grade_level {
        conditions.push(format!("s.grade_level = '{}'", grade_level));
    }
    
    if let Some(ref class_room) = filter.class_room {
        conditions.push(format!("s.class_room = '{}'", class_room));
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
    
    query.push_str(" ORDER BY s.grade_level, s.class_room, s.student_number");
    query.push_str(&format!(" LIMIT {} OFFSET {}", page_size, offset));
    
    let students = match sqlx::query_as::<_, StudentListItem>(&query)
        .fetch_all(&pool)
        .await
    {
        Ok(s) => s,
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการดึงข้อมูล"
                })),
            ).into_response();
        }
    };
    
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": students,
            "page": page,
            "page_size": page_size
        })),
    ).into_response()
}

/// POST /api/students - เพิ่มนักเรียนใหม่
pub async fn create_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateStudentRequest>,
) -> Response {
    // Debug: Log incoming request
    eprintln!("🔍 Creating student with payload: {:?}", payload);
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
    
    // Check permission
    let _user = match check_user_permission(&headers, &pool, "student.create").await {
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
            ).into_response();
        }
    };
    
    // 1. Hash password
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
            ).into_response();
        }
    };
    
    // Parse date_of_birth if provided
    let date_of_birth = match &payload.date_of_birth {
        Some(date_str) if !date_str.is_empty() => {
            match chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
                Ok(date) => Some(date),
                Err(e) => {
                    eprintln!("❌ Invalid date format: {} (error: {})", date_str, e);
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(json!({
                            "success": false,
                            "error": "รูปแบบวันเกิดไม่ถูกต้อง (ต้องเป็น YYYY-MM-DD)"
                        })),
                    ).into_response();
                }
            }
        }
        _ => None,
    };
    
    
    // Encrypt national_id if provided
    let (encrypted_national_id, national_id_hash) = if let Some(nid) = &payload.national_id {
        if !nid.is_empty() {
             let enc = match field_encryption::encrypt(nid) {
                Ok(enc) => enc,
                Err(e) => {
                    eprintln!("❌ Encryption failed: {}", e);
                    let _ = tx.rollback().await;
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(json!({
                            "success": false,
                            "error": "เกิดข้อผิดพลาดในการประมวลผลข้อมูล"
                        })),
                    ).into_response();
                }
            };
            let hash = field_encryption::hash_for_search(nid);
            (Some(enc), Some(hash))
        } else {
            (None, None)
        }
    } else {
        (None, None)
    };
    
    // Determine Username
    // Priority: Supplied Username > 'S' + Student ID > 'S' + Random
    let username = if let Some(u) = &payload.username {
         if !u.is_empty() { u.clone() } else { format!("S{}", payload.student_id) }
    } else {
         format!("S{}", payload.student_id)
    };

    // 2. Create user
    let user_id: Uuid = match sqlx::query_scalar(
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
            ).into_response();
        }
    };
    
    // 3. Create student_info
    match sqlx::query(
        r#"
        INSERT INTO student_info (
            user_id, student_id, grade_level, class_room, student_number
        ) VALUES ($1, $2, $3, $4, $5)
        "#
    )
    .bind(user_id)
    .bind(&payload.student_id)
    .bind(&payload.grade_level)
    .bind(&payload.class_room)
    .bind(&payload.student_number)
    .execute(&mut *tx)
    .await
    {
        Ok(_) => {}
        Err(e) => {
            eprintln!("❌ Failed to create student_info: {}", e);
            let _ = tx.rollback().await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถสร้างข้อมูลนักเรียนได้"
                })),
            ).into_response();
        }
    };
    
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
    match tx.commit().await {
        Ok(_) => {
            (
                StatusCode::CREATED,
                Json(json!({
                    "success": true,
                    "id": user_id,
                    "username": username,
                    "message": "เพิ่มนักเรียนสำเร็จ"
                })),
            ).into_response()
        }
        Err(e) => {
            eprintln!("❌ Failed to commit transaction: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถบันทึกข้อมูลได้"
                })),
            ).into_response()
        }
    }
}

/// GET /api/students/:id - ดูข้อมูลนักเรียน
pub async fn get_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
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
    
    // Check permission
    let _user = match check_user_permission(&headers, &pool, "student.read.all").await {
        Ok(u) => u,
        Err(response) => return response,
    };
    
    let mut student = match sqlx::query_as::<_, StudentProfile>(
        r#"
        SELECT 
            u.id, u.username, u.national_id as national_id, u.email, u.first_name, u.last_name,
            u.title, u.nickname, u.phone, u.date_of_birth, u.gender,
            u.address, u.profile_image_url,
            s.student_id, s.grade_level, s.class_room, s.student_number,
            s.blood_type, s.allergies, s.medical_conditions as medical_conditions
        FROM users u
        LEFT JOIN student_info s ON u.id = s.user_id
        WHERE u.id = $1 AND u.user_type = 'student'
        "#
    )
    .bind(student_id)
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(s)) => s,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": "Student not found"
                })),
            ).into_response();
        }
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการดึงข้อมูล"
                })),
            ).into_response();
        }
    };

    // Decrypt fields
    if let Some(ref nid) = student.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            student.national_id = Some(dec);
        }
    }
    if let Some(ref mc) = student.medical_conditions {
        if let Ok(dec) = field_encryption::decrypt(mc) {
            student.medical_conditions = Some(dec);
        }
    }
    
    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": student
        })),
    ).into_response()
}

/// PUT /api/students/:id - แก้ไขข้อมูลนักเรียน
pub async fn update_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
    Json(payload): Json<UpdateStudentRequest>,
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
    
    // Check permission
    let _user = match check_user_permission(&headers, &pool, "student.update.all").await {
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
            ).into_response();
        }
    };
    
    // Update users table
    match sqlx::query(
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
    {
        Ok(_) => {}
        Err(e) => {
            eprintln!("❌ Failed to update user: {}", e);
            let _ = tx.rollback().await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถอัพเดตข้อมูลได้"
                })),
            ).into_response();
        }
    }
    
    // Update student_info table
    match sqlx::query(
        r#"
        UPDATE student_info
        SET 
            grade_level = COALESCE($2, grade_level),
            class_room = COALESCE($3, class_room),
            student_number = COALESCE($4, student_number),
            updated_at = NOW()
        WHERE user_id = $1
        "#
    )
    .bind(student_id)
    .bind(&payload.grade_level)
    .bind(&payload.class_room)
    .bind(&payload.student_number)
    .execute(&mut *tx)
    .await
    {
        Ok(_) => {}
        Err(e) => {
            eprintln!("❌ Failed to update student_info: {}", e);
            let _ = tx.rollback().await;
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถอัพเดตข้อมูลได้"
                })),
            ).into_response();
        }
    }
    
    match tx.commit().await {
        Ok(_) => {
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "อัพเดตข้อมูลนักเรียนสำเร็จ"
                })),
            ).into_response()
        }
        Err(e) => {
            eprintln!("❌ Failed to commit transaction: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถบันทึกข้อมูลได้"
                })),
            ).into_response()
        }
    }
}

/// DELETE /api/students/:id - ลบนักเรียน (soft delete)
pub async fn delete_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
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
    
    // Check permission
    let _user = match check_user_permission(&headers, &pool, "student.delete").await {
        Ok(u) => u,
        Err(response) => return response,
    };
    
    // Soft delete by setting status to inactive
    match sqlx::query(
        r#"
        UPDATE users 
        SET status = 'inactive', updated_at = NOW() 
        WHERE id = $1 AND user_type = 'student'
        "#
    )
    .bind(student_id)
    .execute(&pool)
    .await
    {
        Ok(_) => {
            (
                StatusCode::OK,
                Json(json!({
                    "success": true,
                    "message": "ลบนักเรียนสำเร็จ"
                })),
            ).into_response()
        }
        Err(e) => {
            eprintln!("❌ Failed to delete student: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถลบนักเรียนได้"
                })),
            ).into_response()
        }
    }
}
