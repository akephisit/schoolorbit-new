use crate::services::AuthService;
use crate::models::LoginRequest;
use shared::types::ApiResponse;
use std::sync::{Arc, OnceLock};
use serde::Serialize;
use sqlx::PgPool;
use ohkami::prelude::*;
use ohkami::claw::Json;
use cookie::{Cookie as CookieBuilder, SameSite};
use ohkami::claw::header::Cookie as CookieHeader;
use shared::auth::validate_token;
use uuid::Uuid;

static DB_POOL: OnceLock<PgPool> = OnceLock::new();

pub fn init_db_pool(pool: PgPool) {
    DB_POOL.set(pool).ok();
}

pub struct AuthHandler {
    service: Arc<AuthService>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginResponse {
    // Token is now sent via HttpOnly cookie, not in response body
    user: serde_json::Value,
}

impl AuthHandler {
    pub fn new(service: Arc<AuthService>) -> Self {
        Self { service }
    }

    pub async fn login(&self, body: String) -> Result<String, String> {
        let data: LoginRequest = serde_json::from_str(&body)
            .map_err(|e| format!("Invalid request body: {}", e))?;

        match self.service.login(data).await {
            Ok((admin, _token)) => {
                let response = ApiResponse::success(LoginResponse {
                    user: serde_json::json!({
                        "id": admin.id,
                        "nationalId": admin.national_id,
                        "name": admin.name,
                        "role": admin.role,
                    }),
                });
                Ok(serde_json::to_string(&response).unwrap())
            }
            Err(e) => Err(format!("{}", e)),
        }
    }
}

// Standalone handler function for Ohkami routing with HttpOnly cookie
pub async fn login_handler(Json(req): Json<LoginRequest>) -> Response {
    let pool = match DB_POOL.get() {
        Some(pool) => pool.clone(),
        None => {
            return Response::InternalServerError()
                .with_text(r#"{"error":"Database not initialized"}"#);
        }
    };

    let auth_service = AuthService::new(pool);
    match auth_service.login(req).await {
        Ok((admin, token)) => {
            let response_data = ApiResponse::success(LoginResponse {
                user: serde_json::json!({
                    "id": admin.id,
                    "nationalId": admin.national_id,
                    "name": admin.name,
                    "role": admin.role,
                }),
            });

            // Create secure HttpOnly cookie using cookie crate
            let cookie = CookieBuilder::build(("auth_token", token))
                .http_only(true)
                .secure(true)  // HTTPS only
                .same_site(SameSite::Lax)
                .path("/")
                .max_age(cookie::time::Duration::days(1))  // 24 hours
                .build();

            // Return response with Set-Cookie header
            let cookie_str = cookie.to_string();
            let mut res = Response::OK().with_json(&response_data);
            
            // Use .x() for custom headers (like Set-Cookie if typed method is missing)
            res.headers.set().x("Set-Cookie", cookie_str);
            
            res
        }
        Err(e) => {
            Response::Unauthorized()
                .with_text(format!(r#"{{"error":"{}"}}"#, e))
        }
    }
}

pub async fn logout_handler() -> Response {
    // Create cookie with empty value and max-age 0 to clear it
    let cookie = CookieBuilder::build(("auth_token", ""))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Lax)
        .path("/")
        .max_age(cookie::time::Duration::seconds(0)) // Expire immediately
        .expires(cookie::time::OffsetDateTime::now_utc() - cookie::time::Duration::days(1))
        .build();

    let cookie_str = cookie.to_string();
    let mut res = Response::OK().with_text(r#"{"success":true}"#);
    
    // Set the clearing cookie
    res.headers.set().x("Set-Cookie", cookie_str);
    
    res
}

pub async fn me_handler(cookie: CookieHeader<&str>) -> Response {
    let pool = match DB_POOL.get() {
        Some(pool) => pool.clone(),
        None => return Response::InternalServerError().with_text(r#"{"error":"Database not initialized"}"#),
    };

    // cookie.0 is the raw cookie string: "auth_token=xyz; other=abc"
    let cookies_str = cookie.0;
    
    // Parse to find auth_token
    let token = cookies_str.split(';')
        .find_map(|s| {
            let s = s.trim();
            if s.starts_with("auth_token=") {
                Some(&s[11..])
            } else {
                None
            }
        });

    let token = match token {
        Some(t) => t,
        None => return Response::Unauthorized().with_text(r#"{"error":"No auth token in cookie"}"#),
    };

    // Validate token
    let claims = match validate_token(token) {
        Ok(c) => c,
        Err(_) => return Response::Unauthorized().with_text(r#"{"error":"Invalid token"}"#),
    };

    // Get user from database
    let auth_service = AuthService::new(pool);
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => return Response::Unauthorized().with_text(r#"{"error":"Invalid user ID in token"}"#),
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
            Response::OK().with_json(&response)
        }
        Err(_) => Response::Unauthorized().with_text(r#"{"error":"User not found"}"#),
    }
}
