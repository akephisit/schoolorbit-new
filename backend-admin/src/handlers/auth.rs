use crate::services::AuthService;
use crate::models::LoginRequest;
use shared::types::ApiResponse;
use std::sync::Arc;
use serde::Serialize;

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
                        "email": admin.email,
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
