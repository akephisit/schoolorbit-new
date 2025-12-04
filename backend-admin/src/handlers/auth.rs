use crate::services::AuthService;
use crate::models::LoginRequest;
use shared::types::ApiResponse;
use std::sync::{Arc, OnceLock};
use serde::Serialize;
use sqlx::PgPool;
use ohkami::claw::Json;

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
    token: String,
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
            Ok((admin, token)) => {
                let response = ApiResponse::success(LoginResponse {
                    token,
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

// Standalone handler function for Ohkami routing
pub async fn login_handler(Json(req): Json<LoginRequest>) -> String {
    let pool = match DB_POOL.get() {
        Some(pool) => pool.clone(),
        None => return format!("{{\"error\":\"Database not initialized\"}}"),
    };

    let auth_service = AuthService::new(pool);
    match auth_service.login(req).await {
        Ok((admin, token)) => {
            let response = ApiResponse::success(LoginResponse {
                token,
                user: serde_json::json!({
                    "id": admin.id,
                    "nationalId": admin.national_id,
                    "name": admin.name,
                    "role": admin.role,
                }),
            });
            serde_json::to_string(&response).unwrap()
        }
        Err(e) => format!("{{\"error\":\"{}\"}}", e),
    }
}
