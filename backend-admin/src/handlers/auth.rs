use crate::services::AuthService;
use crate::models::LoginRequest;
use shared::types::ApiResponse;
use std::sync::{Arc, OnceLock};
use serde::Serialize;
use sqlx::PgPool;
use ohkami::prelude::*;
use ohkami::claw::Json;
use cookie::{Cookie, SameSite};

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
            let cookie = Cookie::build(("auth_token", token))
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
