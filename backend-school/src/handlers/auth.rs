use crate::db::school_mapping::get_school_database_url;
use crate::models::auth::{Claims, LoginRequest, LoginResponse, User, UserResponse, ProfileResponse, UpdateProfileRequest, ChangePasswordRequest};
use crate::utils::jwt::JwtService;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use tower_cookies::{Cookie, Cookies};

/// Login handler
pub async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    cookies: Cookies,
    Json(payload): Json<LoginRequest>,
) -> Response {
    // Extract subdomain from Origin header (secure, cannot be spoofed)
    let subdomain = match extract_subdomain_from_request(&headers) {
        Ok(s) => s,
        Err(response) => return response,
    };

    println!("🔐 Login attempt for school: {}, national ID: {}", subdomain, payload.national_id);

    // Validate national ID format
    if payload.national_id.len() != 13 || !payload.national_id.chars().all(|c| c.is_ascii_digit()) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "success": false,
                "error": "เลขบัตรประชาชนต้องเป็นตัวเลข 13 หลัก"
            })),
        )
            .into_response();
    }

    // Get school database URL from mapping
    let db_url = match get_school_database_url(&state.admin_pool, &subdomain).await {
        Ok(url) => url,
        Err(e) => {
            eprintln!("❌ Failed to get school database: {}", e);
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "success": false,
                    "error": "ไม่พบโรงเรียนหรือโรงเรียนไม่ได้เปิดใช้งาน"
                })),
            )
                .into_response();
        }
    };

    // Connect to school database
    let pool = match state.pool_manager.get_pool(&db_url, &subdomain).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ Failed to connect to school database: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": "ไม่สามารถเชื่อมต่อฐานข้อมูลได้"
                })),
            )
                .into_response();
        }
    };

    // Set encryption key in session for encrypted field operations
    let encryption_key = match crate::utils::encryption::get_encryption_key() {
        Ok(key) => key,
        Err(e) => {
            eprintln!("❌ Encryption key not set: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": "ระบบไม่พร้อมใช้งาน กรุณาติดต่อผู้ดูแลระบบ"
                })),
            )
                .into_response();
        }
    };

    // Set encryption key for this session
    if let Err(e) = sqlx::query(&format!("SET LOCAL app.encryption_key = '{}'", encryption_key))
        .execute(&pool)
        .await
    {
        eprintln!("❌ Failed to set encryption key: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": "เกิดข้อผิดพลาดในระบบรักษาความปลอดภัย"
            })),
        )
            .into_response();
    }

    // Fetch user from database (with encrypted national_id decryption)
    let user = match sqlx::query_as::<_, User>(
        "SELECT 
            id,
            pgp_sym_decrypt(national_id, current_setting('app.encryption_key')) as national_id,
            first_name,
            last_name,
            email,
            phone,
            user_type,
            password_hash,
            status,
            created_at,
            updated_at
         FROM users 
         WHERE pgp_sym_decrypt(national_id, current_setting('app.encryption_key')) = $1 
         AND status = 'active'"
    )
    .bind(&payload.national_id)
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "success": false,
                    "error": "ไม่พบผู้ใช้หรือบัญชีถูกระงับ"
                })),
            )
                .into_response();
        }
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการเข้าสู่ระบบ"
                })),
            )
                .into_response();
        }
    };

    // Verify password
    let is_valid = bcrypt::verify(&payload.password, &user.password_hash).unwrap_or(false);

    if !is_valid {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "success": false,
                "error": "รหัสผ่านไม่ถูกต้อง"
            })),
        )
            .into_response();
    }

    // Generate JWT token
    let token = match JwtService::generate_token(
        &user.id.to_string(),
        user.national_id.as_deref().unwrap_or(""),
        &user.user_type,
    ) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("❌ Failed to generate token: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": "ไม่สามารถสร้าง token ได้"
                })),
            )
                .into_response();
        }
    };

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
    .ok()
    .flatten();

    // Create user response with primary role name
    let mut user_response = UserResponse::from(user);
    user_response.primary_role_name = primary_role_name;

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

    (
        StatusCode::OK,
        Json(LoginResponse {
            success: true,
            message: "เข้าสู่ระบบสำเร็จ".to_string(),
            user: user_response,
        }),
    )
        .into_response()
}

/// Logout handler
pub async fn logout(cookies: Cookies) -> Response {
    // Remove auth token cookie
    let mut cookie = Cookie::new("auth_token", "");
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::seconds(0)); // Expire immediately
    
    cookies.add(cookie);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "ออกจากระบบสำเร็จ"
        })),
    )
        .into_response()
}

/// Get current user handler (protected)
pub async fn me(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: Request,
) -> Response {
    // Extract subdomain from Origin header (secure)
    let subdomain = match extract_subdomain_from_request(&headers) {
        Ok(s) => s,
        Err(response) => return response,
    };

    // Extract claims from middleware
    let claims = match req.extensions().get::<Claims>() {
        Some(c) => c.clone(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "ไม่พบข้อมูลผู้ใช้"
                })),
            )
                .into_response();
        }
    };

    // Get school database URL
    let db_url = match get_school_database_url(&state.admin_pool, &subdomain).await {
        Ok(url) => url,
        Err(e) => {
            eprintln!("❌ Failed to get school database: {}", e);
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
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
                Json(serde_json::json!({
                    "error": "เกิดข้อผิดพลาด"
                })),
            )
                .into_response();
        }
    };

    // Fetch user from database
    let user = match sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(uuid::Uuid::parse_str(&claims.sub).unwrap())
        .fetch_optional(&pool)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "ไม่พบผู้ใช้"
                })),
            )
                .into_response();
        }
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "เกิดข้อผิดพลาด"
                })),
            )
                .into_response();
        }
    };

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
    .ok()
    .flatten();

    // Create response with primary role name
    let mut user_response = UserResponse::from(user);
    user_response.primary_role_name = primary_role_name;

    (StatusCode::OK, Json(user_response)).into_response()
}

/// Get full profile handler (GET /me/profile)
/// Returns complete user profile with all fields
pub async fn get_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    req: Request,
) -> Response {
    // Extract subdomain from Origin header (secure)
    let subdomain = match extract_subdomain_from_request(&headers) {
        Ok(s) => s,
        Err(response) => return response,
    };

    // Extract claims from middleware
    let claims = match req.extensions().get::<Claims>() {
        Some(c) => c.clone(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "ไม่พบข้อมูลผู้ใช้"
                })),
            )
                .into_response();
        }
    };

    // Get school database URL
    let db_url = match get_school_database_url(&state.admin_pool, &subdomain).await {
        Ok(url) => url,
        Err(e) => {
            eprintln!("❌ Failed to get school database: {}", e);
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
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
                Json(serde_json::json!({
                    "error": "เกิดข้อผิดพลาด"
                })),
            )
                .into_response();
        }
    };

    // Fetch user from database
    let user = match sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(uuid::Uuid::parse_str(&claims.sub).unwrap())
        .fetch_optional(&pool)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "ไม่พบผู้ใช้"
                })),
            )
                .into_response();
        }
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "เกิดข้อผิดพลาด"
                })),
            )
                .into_response();
        }
    };

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
    .ok()
    .flatten();

    // Create full profile response with primary role name
    let mut profile_response = ProfileResponse::from(user);
    profile_response.primary_role_name = primary_role_name;

    (StatusCode::OK, Json(profile_response)).into_response()
}

/// Update profile handler (PUT /me/profile)
/// Updates user's editable fields only
pub async fn update_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateProfileRequest>,
) -> Response {
    // Extract subdomain from Origin header (secure)
    let subdomain = match extract_subdomain_from_request(&headers) {
        Ok(s) => s,
        Err(response) => return response,
    };

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
        });

    let token = match token {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "Missing authentication token"
                })),
            )
                .into_response();
        }
    };

    // Validate JWT
    let claims = match JwtService::verify_token(token) {
        Ok(claims) => claims,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "Invalid or expired token"
                })),
            )
                .into_response();
        }
    };

    // Get school database URL
    let db_url = match get_school_database_url(&state.admin_pool, &subdomain).await {
        Ok(url) => url,
        Err(e) => {
            eprintln!("❌ Failed to get school database: {}", e);
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
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
                Json(serde_json::json!({
                    "error": "เกิดข้อผิดพลาด"
                })),
            )
                .into_response();
        }
    };

    let user_id = match uuid::Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("❌ Invalid user ID: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Invalid user ID"
                })),
            )
                .into_response();
        }
    };

    // Parse date_of_birth if provided
    let date_of_birth = payload.date_of_birth.as_ref().and_then(|dob| {
        chrono::NaiveDate::parse_from_str(dob, "%Y-%m-%d").ok()
    });

    // Update user profile
    let result = sqlx::query(
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
             updated_at = NOW()
         WHERE id = $10"
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
    .bind(user_id)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            // Fetch updated user
            let user = match sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
                .bind(user_id)
                .fetch_optional(&pool)
                .await
            {
                Ok(Some(user)) => user,
                Ok(None) => {
                    return (
                        StatusCode::NOT_FOUND,
                        Json(serde_json::json!({
                            "error": "ไม่พบผู้ใช้"
                        })),
                    )
                        .into_response();
                }
                Err(e) => {
                    eprintln!("❌ Database error: {}", e);
                    return (
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(serde_json::json!({
                            "error": "เกิดข้อผิดพลาด"
                        })),
                    )
                        .into_response();
                }
            };

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
            .ok()
            .flatten();

            // Return updated profile
            let mut profile_response = ProfileResponse::from(user);
            profile_response.primary_role_name = primary_role_name;

            (StatusCode::OK, Json(profile_response)).into_response()
        }
        Err(e) => {
            eprintln!("❌ Failed to update profile: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "ไม่สามารถอัพเดทข้อมูลได้"
                })),
            )
                .into_response()
        }
    }
}

/// Change password handler (POST /me/change-password)
/// Changes user's password after verifying current password
pub async fn change_password(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ChangePasswordRequest>,
) -> Response {
    // Extract subdomain from Origin header (secure)
    let subdomain = match extract_subdomain_from_request(&headers) {
        Ok(s) => s,
        Err(response) => return response,
    };

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
        });

    let token = match token {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "Missing authentication token"
                })),
            )
                .into_response();
        }
    };

    // Validate JWT
    let claims = match JwtService::verify_token(token) {
        Ok(claims) => claims,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "Invalid or expired token"
                })),
            )
                .into_response();
        }
    };

    // Get school database URL
    let db_url = match get_school_database_url(&state.admin_pool, &subdomain).await {
        Ok(url) => url,
        Err(e) => {
            eprintln!("❌ Failed to get school database: {}", e);
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
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
                Json(serde_json::json!({
                    "error": "เกิดข้อผิดพลาด"
                })),
            )
                .into_response();
        }
    };

    let user_id = match uuid::Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(e) => {
            eprintln!("❌ Invalid user ID: {}", e);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "Invalid user ID"
                })),
            )
                .into_response();
        }
    };

    // Fetch user from database
    let user = match sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(&pool)
        .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({
                    "error": "ไม่พบผู้ใช้"
                })),
            )
                .into_response();
        }
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "เกิดข้อผิดพลาด"
                })),
            )
                .into_response();
        }
    };

    // Verify current password
    let is_valid = bcrypt::verify(&payload.current_password, &user.password_hash).unwrap_or(false);

    if !is_valid {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "รหัสผ่านปัจจุบันไม่ถูกต้อง"
            })),
        )
            .into_response();
    }

    // Hash new password
    let new_password_hash = match bcrypt::hash(&payload.new_password, bcrypt::DEFAULT_COST) {
        Ok(hash) => hash,
        Err(e) => {
            eprintln!("❌ Failed to hash password: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "ไม่สามารถเข้ารหัสรหัสผ่านได้"
                })),
            )
                .into_response();
        }
    };

    // Update password in database
    let result = sqlx::query(
        "UPDATE users 
         SET password_hash = $1,
             updated_at = NOW()
         WHERE id = $2"
    )
    .bind(&new_password_hash)
    .bind(user_id)
    .execute(&pool)
    .await;

    match result {
        Ok(_) => {
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "success": true,
                    "message": "เปลี่ยนรหัสผ่านสำเร็จ"
                })),
            )
                .into_response()
        }
        Err(e) => {
            eprintln!("❌ Failed to update password: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "ไม่สามารถเปลี่ยนรหัสผ่านได้"
                })),
            )
                .into_response()
        }
    }
}
