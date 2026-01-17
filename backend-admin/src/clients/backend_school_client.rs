use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionRequest {
    pub school_id: String,
    pub db_connection_string: String,
    pub subdomain: String,
    pub admin_username: Option<String>,
    pub admin_password: String,
    pub admin_title: String,
    pub admin_first_name: String,
    pub admin_last_name: String,
}

#[derive(Debug, Deserialize)]
pub struct ProvisionResponse {
    pub success: bool,
    pub message: String,
    pub school_id: String,
}

pub struct BackendSchoolClient {
    client: Client,
    base_url: String,
    internal_secret: String,
}

impl BackendSchoolClient {
    pub fn new() -> Result<Self, String> {
        let base_url = env::var("BACKEND_SCHOOL_URL")
            .unwrap_or_else(|_| "http://backend-school:8081".to_string());
        let internal_secret = env::var("INTERNAL_API_SECRET")
            .map_err(|_| "INTERNAL_API_SECRET not set".to_string())?;

        Ok(Self {
            client: Client::new(),
            base_url,
            internal_secret,
        })
    }

    /// Call backend-school to provision tenant database
    pub async fn provision_tenant(
        &self,
        school_id: &str,
        db_connection_string: &str,
        subdomain: &str,
        admin_username: Option<&str>,
        admin_password: &str,
        admin_title: &str,
        admin_first_name: &str,
        admin_last_name: &str,
    ) -> Result<ProvisionResponse, String> {
        let url = format!("{}/internal/provision", self.base_url);

        let request_body = ProvisionRequest {
            school_id: school_id.to_string(),
            db_connection_string: db_connection_string.to_string(),
            subdomain: subdomain.to_string(),
            admin_username: admin_username.map(|s| s.to_string()),
            admin_password: admin_password.to_string(),
            admin_title: admin_title.to_string(),
            admin_first_name: admin_first_name.to_string(),
            admin_last_name: admin_last_name.to_string(),
        };

        let response = self
            .client
            .post(&url)
            .header("X-Internal-Secret", &self.internal_secret)
            .header("Content-Type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("Failed to call backend-school: {}", e))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            return Err(format!(
                "Backend-school provisioning failed ({}): {}",
                status, error_text
            ));
        }

        let response_data: ProvisionResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse response: {}", e))?;

        Ok(response_data)
    }

    /// Health check for backend-school
    pub async fn health_check(&self) -> bool {
        let url = format!("{}/health", self.base_url);

        match self.client.get(&url).send().await {
            Ok(response) => response.status().is_success(),
            Err(_) => false,
        }
    }
}
