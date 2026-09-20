use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;

const INTERNAL_CALLER: &str = "backend-admin";
const INTERNAL_CALLER_HEADER: &str = "X-Internal-Caller";
const INTERNAL_SECRET_HEADER: &str = "X-Internal-Secret";

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
#[serde(rename_all = "camelCase")]
pub struct ProvisionResponse {
    pub success: bool,
    pub data: ProvisionResponseData,
    pub message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProvisionResponseData {
    #[serde(alias = "school_id")]
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

    #[cfg(test)]
    fn from_config(client: Client, base_url: String, internal_secret: String) -> Self {
        Self {
            client,
            base_url,
            internal_secret,
        }
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
            .header(INTERNAL_SECRET_HEADER, &self.internal_secret)
            .header(INTERNAL_CALLER_HEADER, INTERNAL_CALLER)
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

#[cfg(test)]
mod tests {
    use super::{BackendSchoolClient, ProvisionResponse, INTERNAL_CALLER, INTERNAL_SECRET_HEADER};
    use axum::{http::HeaderMap, routing::post, Json, Router};
    use serde_json::{json, Value};

    async fn start_backend_school(app: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test listener should bind");
        let address = listener
            .local_addr()
            .expect("test listener should have an address");
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("test backend-school API should serve");
        });

        (format!("http://{address}"), task)
    }

    #[test]
    fn provision_response_deserializes_backend_school_envelope() {
        let response: ProvisionResponse = serde_json::from_str(
            r#"{
                "success": true,
                "data": {
                    "school_id": "c01ff666-f591-4a5e-ae6b-28f1a69054f0"
                },
                "message": "Tenant database provisioned successfully"
            }"#,
        )
        .expect("backend-school success envelope should deserialize");

        assert!(response.success);
        assert_eq!(
            response.data.school_id,
            "c01ff666-f591-4a5e-ae6b-28f1a69054f0"
        );
        assert_eq!(
            response.message.as_deref(),
            Some("Tenant database provisioned successfully")
        );
    }

    #[test]
    fn provision_response_accepts_camel_case_school_id() {
        let response: ProvisionResponse = serde_json::from_str(
            r#"{
                "success": true,
                "data": {
                    "schoolId": "b1365c77-5f09-48e9-b881-e713b3fe696f"
                }
            }"#,
        )
        .expect("camelCase success envelope should deserialize");

        assert_eq!(
            response.data.school_id,
            "b1365c77-5f09-48e9-b881-e713b3fe696f"
        );
        assert_eq!(response.message, None);
    }

    #[tokio::test]
    async fn provision_tenant_preserves_internal_headers_path_and_camel_case_body() {
        let internal_secret = uuid::Uuid::new_v4().to_string();
        let expected_secret = internal_secret.clone();
        let admin_password = uuid::Uuid::new_v4().to_string();
        let expected_password = admin_password.clone();
        let app = Router::new().route(
            "/internal/provision",
            post(move |headers: HeaderMap, Json(body): Json<Value>| {
                let expected_secret = expected_secret.clone();
                let expected_password = expected_password.clone();
                async move {
                    assert_eq!(
                        headers
                            .get("X-Internal-Caller")
                            .and_then(|value| value.to_str().ok()),
                        Some(INTERNAL_CALLER)
                    );
                    assert!(
                        headers
                            .get(INTERNAL_SECRET_HEADER)
                            .is_some_and(|value| value.as_bytes() == expected_secret.as_bytes()),
                        "internal secret header must contain the configured value"
                    );
                    assert_eq!(
                        body.get("schoolId").and_then(Value::as_str),
                        Some("c01ff666-f591-4a5e-ae6b-28f1a69054f0")
                    );
                    assert!(
                        body.get("dbConnectionString").is_some_and(|value| {
                            value.as_str() == Some("postgresql://fixture.invalid/school")
                        }),
                        "database connection string must use the camelCase field"
                    );
                    assert_eq!(
                        body.get("subdomain").and_then(Value::as_str),
                        Some("fixture-school")
                    );
                    assert_eq!(
                        body.get("adminUsername").and_then(Value::as_str),
                        Some("fixture-admin")
                    );
                    assert!(
                        body.get("adminPassword").is_some_and(|value| {
                            value.as_str() == Some(expected_password.as_str())
                        }),
                        "admin password must use the camelCase field"
                    );
                    assert_eq!(body.get("adminTitle").and_then(Value::as_str), Some("Mr"));
                    assert_eq!(
                        body.get("adminFirstName").and_then(Value::as_str),
                        Some("Fixture")
                    );
                    assert_eq!(
                        body.get("adminLastName").and_then(Value::as_str),
                        Some("Admin")
                    );
                    assert_eq!(body.as_object().map(|object| object.len()), Some(8));
                    assert!(body.get("school_id").is_none());
                    assert!(body.get("admin_password").is_none());

                    Json(json!({
                        "success": true,
                        "data": {
                            "schoolId": "c01ff666-f591-4a5e-ae6b-28f1a69054f0"
                        },
                        "message": "Tenant database provisioned successfully"
                    }))
                }
            }),
        );
        let (base_url, server) = start_backend_school(app).await;
        let client =
            BackendSchoolClient::from_config(reqwest::Client::new(), base_url, internal_secret);

        let response = client
            .provision_tenant(
                "c01ff666-f591-4a5e-ae6b-28f1a69054f0",
                "postgresql://fixture.invalid/school",
                "fixture-school",
                Some("fixture-admin"),
                &admin_password,
                "Mr",
                "Fixture",
                "Admin",
            )
            .await
            .expect("local provisioning request should succeed");

        assert!(response.success);
        assert_eq!(
            response.data.school_id,
            "c01ff666-f591-4a5e-ae6b-28f1a69054f0"
        );
        server.abort();
    }
}
