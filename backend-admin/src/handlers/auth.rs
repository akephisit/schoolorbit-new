use crate::models::LoginRequest;
use crate::services::AuthService;
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::{Serialize, Deserialize};
use shared::auth::validate_token;
use shared::types::ApiResponse;
use sqlx::PgPool;
use std::sync::OnceLock;
use tower_cookies::{Cookie, Cookies};
use uuid::Uuid;

static DB_POOL: OnceLock<PgPool> = OnceLock::new();

pub fn init_pool(pool: PgPool) {
    DB_POOL.set(pool).ok();
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LoginResponse {
    pub user: serde_json::Value,
}

pub async fn login_handler(
    cookies: Cookies,
    Json(credentials): Json<LoginRequest>,
) -> Response {
    let pool = match DB_POOL.get() {
        Some(pool) => pool.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Database not initialized"
                })),
            )
                .into_response();
        }
    };

    let auth_service = AuthService::new(pool);

    match auth_service.login(credentials).await {
        Ok((admin, token)) => {
            // Set cookie using tower-cookies with proper configuration
            let mut cookie = Cookie::new("auth_token", token.clone());
            cookie.set_path("/");
            cookie.set_http_only(true);
            cookie.set_secure(true);
            cookie.set_same_site(tower_cookies::cookie::SameSite::Lax);
            cookie.set_max_age(tower_cookies::cookie::time::Duration::days(1));
            
            cookies.add(cookie);

            let response_data = ApiResponse::success(LoginResponse {
                user: serde_json::json!({
                    "id": admin.id,
                    "nationalId": admin.national_id,
                    "name": admin.name,
                    "role": admin.role,
                }),
            });

            (StatusCode::OK, Json(response_data)).into_response()
        }
        Err(e) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": e.to_string()
            })),
        )
            .into_response(),
    }
}

pub async fn logout_handler(cookies: Cookies) -> Response {
    // Remove cookie by setting Max-Age to 0
    // Must match the same path/domain as when cookie was created
    let mut cookie = Cookie::new("auth_token", "");
    cookie.set_path("/");
    cookie.set_http_only(true);
    cookie.set_max_age(tower_cookies::cookie::time::Duration::seconds(0));
    
    cookies.add(cookie);

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true
        })),
    )
        .into_response()
}

pub async fn me_handler(cookies: Cookies) -> Response {
    let pool = match DB_POOL.get() {
        Some(pool) => pool.clone(),
        None => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Database not initialized"
                })),
            )
                .into_response();
        }
    };

    // Get auth_token from cookies
    let token = match cookies.get("auth_token") {
        Some(cookie) => cookie.value().to_string(),
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "No auth token in cookie"
                })),
            )
                .into_response();
        }
    };

    // Validate token
    let claims = match validate_token(&token) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "Invalid token"
                })),
            )
                .into_response();
        }
    };

    // Get user from database
    let auth_service = AuthService::new(pool);
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "Invalid user ID in token"
                })),
            )
                .into_response();
        }
    };

    match auth_service.get_admin_by_id(user_id).await {
        Ok(admin) => {
            let response = ApiResponse::success(LoginResponse {
                user: serde_json::json!({
                    "id": admin.id,
                    "nationalId": admin.national_id,
                    "name": admin.name,
                    "role": admin.role,
                }),
            });
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(_) => (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": "User not found"
            })),
        )
            .into_response(),
    }
}
