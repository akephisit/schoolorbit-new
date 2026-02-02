use crate::db::school_mapping::get_school_database_url;
use super::models::{Claims, LoginRequest, LoginResponse, LoginUser, User, UserResponse, ProfileResponse, UpdateProfileRequest, ChangePasswordRequest};
use crate::utils::jwt::JwtService;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::utils::field_encryption;
use crate::utils::file_url::get_file_url_from_string;
use crate::AppState;
use crate::error::AppError;
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use tower_cookies::{Cookie, Cookies};


/// Login handler
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    Json(payload): Json<LoginRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Extract subdomain from Origin header (secure, cannot be spoofed)
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    println!("🔐 Login attempt for school: {}, username: {}", subdomain, payload.username);

    // Get school database URL from mapping
    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียนหรือโรงเรียนไม่ได้เปิดใช้งาน".to_string())
        })?;

    // Connect to school database
    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to connect to school database: {}", e);
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;

    // Find user by username
    let user = sqlx::query_as::<_, LoginUser>(
        r#"
        SELECT id, username, password_hash, status, user_type, first_name, last_name, email, date_of_birth, profile_image_url
        FROM users
        WHERE username = $1 AND status = 'active'
        "#
    )
    .bind(&payload.username)
    .fetch_optional(&pool)
    .await?
    .ok_or(AppError::AuthError("ไม่พบผู้ใช้หรือบัญชีถูกระงับ".to_string()))?;

    // Verify password
    let is_valid = bcrypt::verify(&payload.password, &user.password_hash).unwrap_or(false);

    if !is_valid {
        return Err(AppError::AuthError("รหัสผ่านไม่ถูกต้อง".to_string()));
    }

    // Generate JWT token
    let token = JwtService::generate_token(
        &user.id.to_string(),
        &user.username,
        &user.user_type,
    ).map_err(|e| {
        eprintln!("❌ Failed to generate token: {}", e);
        AppError::InternalServerError("ไม่สามารถสร้าง token ได้".to_string())
    })?;

    // Fetch primary role name
    let primary_role_name: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT r.name 
         FROM user_roles ur
         JOIN roles r ON ur.role_id = r.id
         WHERE ur.user_id = $1 
           AND ur.is_primary = true 
           AND ur.ended_at IS NULL
         LIMIT 1"
    )
    .bind(&user.id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    // Fetch all user permissions from normalized schema
    // Fetch all user permissions (Role-based + Department-based)
    let permissions: Vec<String> = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT code FROM (
             -- 1. Role-based Permissions
             SELECT p.code
             FROM user_roles ur
             JOIN role_permissions rp ON ur.role_id = rp.role_id
             JOIN permissions p ON rp.permission_id = p.id
             WHERE ur.user_id = $1 
               AND ur.ended_at IS NULL
             
             UNION
             
             -- 2. Department-based Permissions
             SELECT p.code
             FROM staff_info si
             JOIN department_permissions dp ON si.department_id = dp.department_id
             JOIN permissions p ON dp.permission_id = p.id
             WHERE si.user_id = $1 
               AND si.is_active = true 
               AND (si.resigned_date IS NULL OR si.resigned_date > CURRENT_DATE)
         ) AS all_perms
         ORDER BY code"
    )
    .bind(&user.id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Create user response manually (LoginUser doesn't implement From)
    let user_response = UserResponse {
        id: user.id,
        username: user.username.clone(),
        national_id: None, // Don't send national_id on login via username for privacy
        email: user.email.clone(),
        first_name: user.first_name.clone(),
        last_name: user.last_name.clone(),
        user_type: user.user_type.clone(),
        phone: None,
        status: user.status.clone(),
        created_at: chrono::Utc::now(),
        primary_role_name,
        profile_image_url: get_file_url_from_string(&user.profile_image_url),
        permissions: Some(permissions),
    };

    // Set cookie (optional, based on remember_me)
    let max_age = if payload.remember_me.unwrap_or(false) {
        30 * 24 * 60 * 60 // 30 days in seconds
    } else {
        24 * 60 * 60 // 1 day in seconds
    };

    let mut cookie = Cookie::new("auth_token", token.clone());
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_same_site(tower_cookies::cookie::SameSite::Lax);
    cookie.set_max_age(time::Duration::seconds(max_age));
    
    cookies.add(cookie);

    Ok((
        StatusCode::OK,
        Json(LoginResponse {
            success: true,
            message: "เข้าสู่ระบบสำเร็จ".to_string(),
            user: user_response,
        }),
    ))
}

/// Logout handler
pub async fn logout(cookies: Cookies) -> Result<impl IntoResponse, AppError> {
    // Remove auth token cookie
    let mut cookie = Cookie::new("auth_token", "");
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::seconds(0)); // Expire immediately
    
    cookies.add(cookie);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "ออกจากระบบสำเร็จ"
        })),
    ))
}

/// Get current user handler (protected)
pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: Request,
) -> Result<impl IntoResponse, AppError> {
    // Extract subdomain from Origin header (secure)
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    // Extract claims from middleware
    let claims = req.extensions().get::<Claims>()
        .ok_or(AppError::AuthError("ไม่พบข้อมูลผู้ใช้".to_string()))?
        .clone();

    // Get school database URL
    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    // Get pool
    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาดในการเชื่อมต่อฐานข้อมูล".to_string())
        })?;

    // Fetch user from database
    let mut user = sqlx::query_as::<_, User>(
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
        .fetch_optional(&pool)
        .await?
        .ok_or(AppError::NotFound("ไม่พบผู้ใช้".to_string()))?;

    // Decrypt national_id
    // Decrypt national_id
    if let Some(nid) = &user.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            user.national_id = Some(dec);
        }
    }


    // Fetch primary role name
    let primary_role_name: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT r.name 
         FROM user_roles ur
         JOIN roles r ON ur.role_id = r.id
         WHERE ur.user_id = $1 
           AND ur.is_primary = true 
           AND ur.ended_at IS NULL
         LIMIT 1"
    )
    .bind(&user.id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    // Fetch all user permissions from normalized schema
    // Fetch all user permissions (Role-based + Department-based)
    let permissions: Vec<String> = sqlx::query_scalar::<_, String>(
        "SELECT DISTINCT code FROM (
             -- 1. Role-based Permissions
             SELECT p.code
             FROM user_roles ur
             JOIN role_permissions rp ON ur.role_id = rp.role_id
             JOIN permissions p ON rp.permission_id = p.id
             WHERE ur.user_id = $1 
               AND ur.ended_at IS NULL
             
             UNION
             
             -- 2. Department-based Permissions
             SELECT p.code
             FROM staff_info si
             JOIN department_permissions dp ON si.department_id = dp.department_id
             JOIN permissions p ON dp.permission_id = p.id
             WHERE si.user_id = $1 
               AND si.is_active = true 
               AND (si.resigned_date IS NULL OR si.resigned_date > CURRENT_DATE)
         ) AS all_perms
         ORDER BY code"
    )
    .bind(&user.id)
    .fetch_all(&pool)
    .await
    .unwrap_or_default();

    // Create response with primary role name and permissions
    let mut user_response = UserResponse::from(user);
    user_response.primary_role_name = primary_role_name;
    // Always send permissions array (even if empty) so frontend can check
    user_response.permissions = Some(permissions);

    Ok((StatusCode::OK, Json(user_response)))
}

/// Get full profile handler (GET /me/profile)
/// Returns complete user profile with all fields
/// Get full profile handler (GET /me/profile)
/// Returns complete user profile with all fields
pub async fn get_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: Request,
) -> Result<impl IntoResponse, AppError> {
    // Extract subdomain from Origin header (secure)
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    // Extract claims from middleware
    let claims = req.extensions().get::<Claims>()
        .ok_or(AppError::AuthError("ไม่พบข้อมูลผู้ใช้".to_string()))?
        .clone();

    // Get school database URL
    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    // Get pool
    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;

    // Fetch user from database
    let mut user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(uuid::Uuid::parse_str(&claims.sub).unwrap())
        .fetch_optional(&pool)
        .await?
        .ok_or(AppError::NotFound("ไม่พบผู้ใช้".to_string()))?;

    if let Some(nid) = &user.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            user.national_id = Some(dec);
        }
    }

    // Fetch primary role name
    let primary_role_name: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT r.name 
         FROM user_roles ur
         JOIN roles r ON ur.role_id = r.id
         WHERE ur.user_id = $1 
           AND ur.is_primary = true 
           AND ur.ended_at IS NULL
         LIMIT 1"
    )
    .bind(&user.id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    // Create full profile response with primary role name
    let mut profile_response = ProfileResponse::from(user);
    profile_response.primary_role_name = primary_role_name;

    Ok((StatusCode::OK, Json(profile_response)))
}

/// Update profile handler (PUT /me/profile)
/// Updates user's editable fields only
/// Update profile handler (PUT /me/profile)
/// Updates user's editable fields only
/// Update profile handler (PUT /me/profile)
/// Updates user's editable fields only
pub async fn update_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateProfileRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Extract subdomain from Origin header (secure)
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    // Extract token from Authorization header or cookie
    let token = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .or_else(|| {
            // Try cookie as fallback
            headers
                .get("cookie")
                .and_then(|h| h.to_str().ok())
                .and_then(|cookies| {
                    cookies
                        .split(';')
                        .find_map(|c| c.trim().strip_prefix("auth_token="))
                })
        })
        .ok_or(AppError::AuthError("Missing authentication token".to_string()))?;

    // Validate JWT
    let claims = JwtService::verify_token(token)
        .map_err(|_| AppError::AuthError("Invalid or expired token".to_string()))?;

    // Get school database URL
    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    // Get pool
    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;

    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| {
            eprintln!("❌ Invalid user ID: {}", e);
            AppError::BadRequest("Invalid user ID".to_string())
        })?;

    // Parse date_of_birth if provided
    let date_of_birth = payload.date_of_birth.as_ref().and_then(|dob| {
        chrono::NaiveDate::parse_from_str(dob, "%Y-%m-%d").ok()
    });

    // Update user profile
    sqlx::query(
        "UPDATE users 
         SET title = COALESCE($1, title),
             nickname = COALESCE($2, nickname),
             email = COALESCE($3, email),
             phone = COALESCE($4, phone),
             emergency_contact = COALESCE($5, emergency_contact),
             line_id = COALESCE($6, line_id),
             date_of_birth = COALESCE($7, date_of_birth),
             gender = COALESCE($8, gender),
             address = COALESCE($9, address),
             profile_image_url = COALESCE($10, profile_image_url),
             updated_at = NOW()
         WHERE id = $11"
    )
    .bind(&payload.title)
    .bind(&payload.nickname)
    .bind(&payload.email)
    .bind(&payload.phone)
    .bind(&payload.emergency_contact)
    .bind(&payload.line_id)
    .bind(date_of_birth)
    .bind(&payload.gender)
    .bind(&payload.address)
    .bind(&payload.profile_image_url)
    .bind(user_id)
    .execute(&pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to update profile: {}", e);
        AppError::InternalServerError("ไม่สามารถอัพเดทข้อมูลได้".to_string())
    })?;

    // Fetch updated user
    let mut user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&pool)
        .await?
        .ok_or(AppError::NotFound("ไม่พบผู้ใช้".to_string()))?;

    if let Some(nid) = &user.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            user.national_id = Some(dec);
        }
    }

    // Fetch primary role name
    let primary_role_name: Option<String> = sqlx::query_scalar::<_, String>(
        "SELECT r.name 
            FROM user_roles ur
            JOIN roles r ON ur.role_id = r.id
            WHERE ur.user_id = $1 
            AND ur.is_primary = true 
            AND ur.ended_at IS NULL
            LIMIT 1"
    )
    .bind(&user.id)
    .fetch_optional(&pool)
    .await
    .unwrap_or(None);

    // Return updated profile
    let mut profile_response = ProfileResponse::from(user);
    profile_response.primary_role_name = primary_role_name;

    Ok((StatusCode::OK, Json(profile_response)))
}

/// Change password handler (POST /me/change-password)
/// Changes user's password after verifying current password
/// Change password handler (POST /me/change-password)
/// Changes user's password after verifying current password
pub async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ChangePasswordRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Extract subdomain from Origin header (secure)
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    // Extract token from Authorization header or cookie
    let token = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer "))
        .or_else(|| {
            // Try cookie as fallback
            headers
                .get("cookie")
                .and_then(|h| h.to_str().ok())
                .and_then(|cookies| {
                    cookies
                        .split(';')
                        .find_map(|c| c.trim().strip_prefix("auth_token="))
                })
        })
        .ok_or(AppError::AuthError("Missing authentication token".to_string()))?;

    // Validate JWT
    let claims = JwtService::verify_token(token)
        .map_err(|_| AppError::AuthError("Invalid or expired token".to_string()))?;

    // Get school database URL
    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    // Get pool
    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;

    let user_id = uuid::Uuid::parse_str(&claims.sub)
        .map_err(|e| {
            eprintln!("❌ Invalid user ID: {}", e);
            AppError::BadRequest("Invalid user ID".to_string())
        })?;

    // Find user by id to verify current password
    let user = sqlx::query_as::<_, LoginUser>(
        r#"
        SELECT id, username, password_hash, status, user_type, first_name, last_name, email, date_of_birth, profile_image_url
        FROM users
        WHERE id = $1 AND status = 'active'
        "#
    )
    .bind(user_id)
    .fetch_optional(&pool)
    .await?
    .ok_or(AppError::NotFound("ไม่พบผู้ใช้หรือบัญชีถูกระงับ".to_string()))?;

    // Verify current (old) password
    let is_valid = bcrypt::verify(&payload.current_password, &user.password_hash).unwrap_or(false);

    if !is_valid {
        return Err(AppError::AuthError("รหัสผ่านปัจจุบันไม่ถูกต้อง".to_string()));
    }

    // Hash new password
    let new_password_hash = bcrypt::hash(&payload.new_password, 10)
        .map_err(|e| {
            eprintln!("❌ Failed to hash password: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาด".to_string())
        })?;

    // Update password
    sqlx::query("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2")
        .bind(new_password_hash)
        .bind(user_id)
        .execute(&pool)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to update password: {}", e);
            AppError::InternalServerError("ไม่สามารถเปลี่ยนรหัสผ่านได้".to_string())
        })?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "เปลี่ยนรหัสผ่านสำเร็จ"
        })),
    ))
}
