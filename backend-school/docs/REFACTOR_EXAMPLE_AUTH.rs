use crate::db::school_mapping::get_school_database_url;
use crate::models::auth::{Claims, LoginRequest, LoginResponse, LoginUser, User, UserResponse, ProfileResponse, UpdateProfileRequest, ChangePasswordRequest};
use crate::utils::jwt::JwtService;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::utils::field_encryption;
use crate::AppState;
use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use tower_cookies::{Cookie, Cookies};

/// Login handler - authenticates user and returns JWT token
pub async fn login(
    State(state): State<AppState>,
    req: Request,
    cookies: Cookies,
    Json(payload): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, Response> {
    // Extract subdomain
    let subdomain = extract_subdomain_from_request(&req)
        .ok_or_else(|| {
            (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "success": false,
                    "error": "ไม่สามารถระบุโรงเรียนได้"
                })),
            )
                .into_response()
        })?;

    tracing::info!("🔐 Login attempt for school: {}, national ID: {}", subdomain, payload.national_id);

    // Get school database URL
    let database_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            tracing::error!("❌ Failed to get school database URL: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": "ไม่สามารถเชื่อมต่อกับฐานข้อมูลโรงเรียนได้"
                })),
            )
                .into_response()
        })?;

    // Get connection pool
    let pool = state
        .pool_manager
        .get_pool(&subdomain, &database_url)
        .await
        .map_err(|e| {
            tracing::error!("❌ Failed to get pool: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": "ไม่สามารถเชื่อมต่อกับฐานข้อมูลได้"
                })),
            )
                .into_response()
        })?;

    // Encrypt national_id for comparison
    let encrypted_national_id = field_encryption::encrypt(&payload.national_id)
        .map_err(|e| {
            tracing::error!("❌ Encryption failed: {}",  e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการประมวลผล"
                })),
            )
                .into_response()
        })?;

    // Query user by encrypted national_id
    let user_opt: Option<LoginUser> = sqlx::query_as(
        "SELECT 
            id, national_id, email, password_hash, 
            first_name, last_name, user_type, status, date_of_birth
         FROM users 
         WHERE national_id = $1"
    )
    .bind(&encrypted_national_id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| {
        tracing::error!("❌ Database error: {}", e);
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "success": false,
                "error": "เกิดข้อผิดพลาดในการเข้าสู่ระบบ"
            })),
        )
            .into_response()
    })?;

    let user = user_opt.ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "success": false,
                "error": "เลขบัตรประชาชนหรือรหัสผ่านไม่ถูกต้อง"
            })),
        )
            .into_response()
    })?;

    // Verify password
    let is_valid = bcrypt::verify(&payload.password, &user.password_hash)
        .map_err(|e| {
            tracing::error!("❌ Password verification error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการตรวจสอบรหัสผ่าน"
                })),
            )
                .into_response()
        })?;

    if !is_valid {
        return Err((
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "success": false,
                "error": "เลขบัตรประชาชนหรือรหัสผ่านไม่ถูกต้อง"
            })),
        )
            .into_response());
    }

    // Check if user is active
    if user.status != "active" {
        return Err((
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({
                "success": false,
                "error": "บัญชีผู้ใช้ถูกระงับ กรุณาติดต่อผู้ดูแลระบบ"
            })),
        )
            .into_response());
    }

    // Generate JWT token (use plaintext national_id)
    let claims = Claims {
        sub: user.id.to_string(),
        national_id: payload.national_id.clone(), // Use plaintext for JWT
        user_type: user.user_type.clone(),
        exp: (chrono::Utc::now() + chrono::Duration::hours(24)).timestamp() as usize,
    };

    let token = JwtService::generate_token(&claims)
        .map_err(|e| {
            tracing::error!("❌ Token generation error: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": "ไม่สามารถสร้าง token ได้"
                })),
            )
                .into_response()
        })?;

    // Set cookie
    let mut cookie = Cookie::new("auth_token", token.clone());
    cookie.set_http_only(true);
    cookie.set_secure(true);
    cookie.set_same_site(cookie::SameSite::Lax);
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::days(1));
    cookies.add(cookie);

    // Build user response
    let user_response = UserResponse {
        id: user.id,
        national_id: payload.national_id, // Return plaintext
        email: user.email,
        first_name: user.first_name,
        last_name: user.last_name,
        user_type: user.user_type,
        phone: None,
        date_of_birth: user.date_of_birth,
        address: None,
        profile_image_url: None,
    };

    Ok(Json(LoginResponse {
        success: true,
        token,
        user: user_response,
    }))
}

// ... rest of auth.rs handlers will be in next file
// This is just the login function as an example
//
// Key changes:
// 1. Import field_encryption
// 2. Encrypt national_id before query
// 3. Query uses direct comparison: WHERE national_id = $1
// 4. Use plaintext national_id in JWT and responses
