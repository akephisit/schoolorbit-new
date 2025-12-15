use crate::models::auth::{Claims, LoginRequest, LoginResponse, User, UserResponse};
use crate::utils::jwt::JwtService;
use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use sqlx::PgPool;
use tower_cookies::{Cookie, Cookies};

/// Login handler
pub async fn login(
    State(pool): State<PgPool>,
    cookies: Cookies,
    Json(payload): Json<LoginRequest>,
) -> Response {
    println!("🔐 Login attempt for national ID: {}", payload.national_id);

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

    // Find user by national_id
    let user = match sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE national_id = $1 AND status = 'active'"
    )
    .bind(&payload.national_id)
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(user)) => user,
        Ok(None) => {
            println!("❌ User not found: {}", payload.national_id);
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "success": false,
                    "error": "เลขบัตรประชาชนหรือรหัสผ่านไม่ถูกต้อง"
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
    let password_valid = bcrypt::verify(&payload.password, &user.password_hash)
        .unwrap_or(false);

    if !password_valid {
        println!("❌ Invalid password for: {}", payload.national_id);
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "success": false,
                "error": "เลขบัตรประชาชนหรือรหัสผ่านไม่ถูกต้อง"
            })),
        )
            .into_response();
    }

    // Generate JWT token
    let token = match JwtService::generate_token(
        &user.id.to_string(),
        &payload.national_id,
        &user.role,
    ) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("❌ Token generation failed: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการสร้าง token"
                })),
            )
                .into_response();
        }
    };

    // Set HTTP-only cookie
    let mut cookie = Cookie::new("auth_token", token);
    cookie.set_http_only(true);
    cookie.set_path("/");
    cookie.set_secure(false); // Set to true in production with HTTPS
    
    // Set expiry based on remember_me
    if payload.remember_me.unwrap_or(false) {
        cookie.set_max_age(time::Duration::days(7));
    } else {
        cookie.set_max_age(time::Duration::days(1));
    }
    
    cookies.add(cookie);

    println!("✅ Login successful: {} ({})", user.first_name, user.role);

    let response = LoginResponse {
        success: true,
        message: "เข้าสู่ระบบสำเร็จ".to_string(),
        user: UserResponse::from(user),
    };

    (StatusCode::OK, Json(response)).into_response()
}

/// Logout handler
pub async fn logout(cookies: Cookies) -> Response {
    println!("🚪 Logout request");

    // Remove auth cookie
    let mut cookie = Cookie::new("auth_token", "");
    cookie.set_http_only(true);
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
    State(pool): State<PgPool>,
    req: Request,
) -> Response {
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

    (StatusCode::OK, Json(UserResponse::from(user))).into_response()
}
