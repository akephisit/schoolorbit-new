use crate::db::school_mapping::get_school_database_url;
use crate::models::auth::{Claims, LoginRequest, LoginResponse, User, UserResponse, ProfileResponse};
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

    // Fetch user from database
    let user = match sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE national_id = $1 AND status = 'active'"
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
