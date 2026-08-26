use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use std::env;
use tokio::sync::OnceCell;
use tracing::{debug, info, warn};

const NEON_API_BASE_URL: &str = "https://console.neon.tech/api/v2";

#[derive(Debug, Serialize)]
struct CreateDatabaseRequest {
    database: DatabaseConfig,
}

#[derive(Debug, Serialize)]
struct DatabaseConfig {
    name: String,
    owner_name: String,
}

#[derive(Debug, Deserialize)]
struct CreateDatabaseResponse {
    database: DatabaseInfo,
}

#[derive(Debug, Deserialize)]
struct DatabaseInfo {
    id: i64, // Only field we actually use
}

#[derive(Debug, Deserialize)]
struct ListBranchesResponse {
    branches: Vec<BranchInfo>,
}

#[derive(Debug, Deserialize)]
struct BranchInfo {
    id: String,
    name: String,
}

#[derive(Debug, Default, Deserialize)]
struct NeonApiErrorBody {
    request_id: Option<String>,
    code: Option<String>,
    message: Option<String>,
}

struct NeonApiFailure {
    status: StatusCode,
    body: NeonApiErrorBody,
}

impl NeonApiFailure {
    fn is_branch_not_found(&self) -> bool {
        self.status == StatusCode::NOT_FOUND
            && self
                .body
                .message
                .as_deref()
                .is_some_and(|message| message.eq_ignore_ascii_case("branch not found"))
    }

    fn is_locked(&self) -> bool {
        self.status == StatusCode::LOCKED
            || self.body.message.as_deref().is_some_and(|message| {
                message.contains("conflicting operations") || message.contains("Locked")
            })
    }

    fn user_message(&self, action: &str) -> String {
        if self.is_branch_not_found() {
            return "ไม่พบ Neon branch ที่กำหนด กรุณาตรวจสอบ NEON_PROJECT_ID และ NEON_BRANCH_ID"
                .to_string();
        }

        format!(
            "Neon API ไม่สามารถ{}ได้ (HTTP {}) กรุณาลองอีกครั้ง",
            action,
            self.status.as_u16()
        )
    }
}

fn sanitize_provider_field(value: Option<&str>) -> Option<String> {
    value.and_then(|value| {
        let sanitized = value
            .chars()
            .take(256)
            .map(|character| {
                if character.is_control() {
                    ' '
                } else {
                    character
                }
            })
            .collect::<String>();
        (!sanitized.trim().is_empty()).then_some(sanitized)
    })
}

pub struct NeonClient {
    client: Client,
    api_key: String,
    project_id: String,
    branch_reference: String,
    resolved_branch_id: OnceCell<String>,
    api_base_url: String,
}

impl NeonClient {
    pub fn new() -> Result<Self, String> {
        let api_key = env::var("NEON_API_KEY").map_err(|_| "NEON_API_KEY not set".to_string())?;
        let project_id =
            env::var("NEON_PROJECT_ID").map_err(|_| "NEON_PROJECT_ID not set".to_string())?;
        let branch_reference =
            env::var("NEON_BRANCH_ID").map_err(|_| "NEON_BRANCH_ID not set".to_string())?;

        Self::from_config(
            Client::new(),
            api_key,
            project_id,
            branch_reference,
            NEON_API_BASE_URL.to_string(),
        )
    }

    fn from_config(
        client: Client,
        api_key: String,
        project_id: String,
        branch_reference: String,
        api_base_url: String,
    ) -> Result<Self, String> {
        if api_key.trim().is_empty() {
            return Err("NEON_API_KEY is empty".to_string());
        }
        if project_id.trim().is_empty() {
            return Err("NEON_PROJECT_ID is empty".to_string());
        }
        if branch_reference.trim().is_empty() {
            return Err("NEON_BRANCH_ID is empty".to_string());
        }

        Ok(Self {
            client,
            api_key,
            project_id,
            branch_reference,
            resolved_branch_id: OnceCell::new(),
            api_base_url: api_base_url.trim_end_matches('/').to_string(),
        })
    }

    async fn branch_id(&self) -> Result<&str, String> {
        let branch_id = self
            .resolved_branch_id
            .get_or_try_init(|| async {
                if self.branch_reference.starts_with("br-") {
                    return Ok(self.branch_reference.clone());
                }

                let url = format!(
                    "{}/projects/{}/branches",
                    self.api_base_url, self.project_id
                );
                let response = self
                    .client
                    .get(&url)
                    .header("Authorization", format!("Bearer {}", self.api_key))
                    .header("Accept", "application/json")
                    .query(&[("search", self.branch_reference.as_str())])
                    .send()
                    .await
                    .map_err(|error| {
                        warn!(error = %error, "failed to resolve Neon branch");
                        "ไม่สามารถติดต่อ Neon API เพื่อตรวจสอบ branch ได้".to_string()
                    })?;

                if !response.status().is_success() {
                    let failure = Self::read_api_failure(response, "resolve branch").await;
                    return Err(failure.user_message("ตรวจสอบ branch"));
                }

                let branches: ListBranchesResponse = response.json().await.map_err(|error| {
                    warn!(error = %error, "failed to parse Neon branch list");
                    "Neon API ส่งข้อมูล branch ที่ไม่ถูกต้อง".to_string()
                })?;

                branches
                    .branches
                    .into_iter()
                    .find(|branch| branch.name == self.branch_reference)
                    .map(|branch| branch.id)
                    .ok_or_else(|| {
                        "ไม่พบ Neon branch ที่กำหนด กรุณาตรวจสอบ NEON_PROJECT_ID และ NEON_BRANCH_ID"
                            .to_string()
                    })
            })
            .await?;

        Ok(branch_id)
    }

    async fn read_api_failure(response: Response, operation: &str) -> NeonApiFailure {
        let status = response.status();
        let body = match response.json::<NeonApiErrorBody>().await {
            Ok(body) => body,
            Err(error) => {
                warn!(%status, operation, error = %error, "failed to parse Neon API error response");
                NeonApiErrorBody::default()
            }
        };
        let request_id = sanitize_provider_field(body.request_id.as_deref());
        let code = sanitize_provider_field(body.code.as_deref());
        let message = sanitize_provider_field(body.message.as_deref());
        warn!(
            %status,
            operation,
            request_id = request_id.as_deref().unwrap_or("unavailable"),
            code = code.as_deref().unwrap_or("unavailable"),
            provider_message = message.as_deref().unwrap_or("unavailable"),
            "Neon API request failed"
        );

        NeonApiFailure { status, body }
    }

    /// Create a new database in Neon
    pub async fn create_database(&self, db_name: &str, owner_name: &str) -> Result<i64, String> {
        let branch_id = self.branch_id().await?;
        let url = format!(
            "{}/projects/{}/branches/{}/databases",
            self.api_base_url, self.project_id, branch_id
        );

        let request_body = CreateDatabaseRequest {
            database: DatabaseConfig {
                name: db_name.to_string(),
                owner_name: owner_name.to_string(),
            },
        };

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Accept", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|error| {
                warn!(error = %error, "failed to create Neon database");
                "ไม่สามารถติดต่อ Neon API เพื่อสร้างฐานข้อมูลได้".to_string()
            })?;

        if !response.status().is_success() {
            let failure = Self::read_api_failure(response, "create database").await;
            return Err(failure.user_message("สร้างฐานข้อมูล"));
        }

        // Get response text first for debugging
        let response_text = response.text().await.map_err(|error| {
            warn!(error = %error, "failed to read Neon create database response");
            "ไม่สามารถอ่านผลการสร้างฐานข้อมูลจาก Neon API ได้".to_string()
        })?;

        // Try to parse the response
        let response_data: CreateDatabaseResponse =
            serde_json::from_str(&response_text).map_err(|error| {
                warn!(error = %error, "failed to parse Neon create database response");
                "Neon API ส่งข้อมูลการสร้างฐานข้อมูลที่ไม่ถูกต้อง".to_string()
            })?;
        info!(
            database_id = response_data.database.id,
            db_name, "Neon database created"
        );

        Ok(response_data.database.id)
    }

    /// Get connection string for a database
    pub fn get_connection_string(&self, db_name: &str, db_user: &str, db_password: &str) -> String {
        // Neon connection string format
        // postgres://user:password@host/dbname?sslmode=require
        let host =
            env::var("NEON_HOST").unwrap_or_else(|_| format!("{}.neon.tech", self.project_id));

        format!(
            "postgresql://{}:{}@{}/{}?sslmode=require",
            db_user, db_password, host, db_name
        )
    }

    /// Delete a database by name (not ID!)
    /// Neon API requires database name, not ID
    pub async fn delete_database_by_name(&self, db_name: &str) -> Result<(), String> {
        let branch_id = self.branch_id().await?;
        let url = format!(
            "{}/projects/{}/branches/{}/databases/{}",
            self.api_base_url, self.project_id, branch_id, db_name
        );

        let max_attempts = 12;

        for attempt in 1..=max_attempts {
            info!(
                db_name,
                attempt, max_attempts, "deleting Neon database by name"
            );

            let response = self
                .client
                .delete(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .send()
                .await
                .map_err(|error| {
                    warn!(error = %error, "failed to delete Neon database");
                    "ไม่สามารถติดต่อ Neon API เพื่อลบฐานข้อมูลได้".to_string()
                })?;

            let status = response.status();
            debug!(%status, "Neon delete database response");

            if status.is_success() {
                return Ok(());
            }

            let failure = Self::read_api_failure(response, "delete database").await;

            if failure.is_locked() && attempt < max_attempts {
                warn!(
                    db_name,
                    attempt, "Neon project locked by another operation; retrying"
                );
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                continue;
            }

            return Err(failure.user_message("ลบฐานข้อมูล"));
        }

        Err("ไม่สามารถลบฐานข้อมูลจาก Neon ได้หลังจากลองซ้ำ".to_string())
    }

    /// Delete a database by ID (deprecated - use delete_database_by_name)
    /// Kept for backward compatibility
    pub async fn delete_database(&self, db_id: i64) -> Result<(), String> {
        let branch_id = self.branch_id().await?;
        let url = format!(
            "{}/projects/{}/branches/{}/databases/{}",
            self.api_base_url, self.project_id, branch_id, db_id
        );

        info!(db_id, "deleting Neon database by ID");

        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .send()
            .await
            .map_err(|error| {
                warn!(error = %error, "failed to delete Neon database by ID");
                "ไม่สามารถติดต่อ Neon API เพื่อลบฐานข้อมูลได้".to_string()
            })?;

        let status = response.status();
        debug!(%status, "Neon delete database response");

        if !status.is_success() {
            let failure = Self::read_api_failure(response, "delete database by ID").await;
            return Err(failure.user_message("ลบฐานข้อมูล"));
        }

        Ok(())
    }

    /// Wait for database to be ready
    /// Neon creates databases asynchronously, so we need to wait for it to be ready
    pub async fn wait_for_database_ready(&self, db_name: &str) -> Result<(), String> {
        info!(db_name, "waiting for database to be ready");
        let branch_id = self.branch_id().await?;

        let max_attempts = 30; // 30 seconds max
        let mut attempts = 0;

        while attempts < max_attempts {
            attempts += 1;

            // Check if database exists and is ready
            let url = format!(
                "{}/projects/{}/branches/{}/databases",
                self.api_base_url, self.project_id, branch_id
            );

            let response = self
                .client
                .get(&url)
                .header("Authorization", format!("Bearer {}", self.api_key))
                .send()
                .await
                .map_err(|error| {
                    warn!(error = %error, "failed to check Neon database readiness");
                    "ไม่สามารถติดต่อ Neon API เพื่อตรวจสอบฐานข้อมูลได้".to_string()
                })?;

            if response.status().is_success() {
                let text = response.text().await.map_err(|error| {
                    warn!(error = %error, "failed to read Neon database readiness response");
                    "ไม่สามารถอ่านสถานะฐานข้อมูลจาก Neon API ได้".to_string()
                })?;

                // Check if our database is in the list
                if text.contains(db_name) {
                    info!(db_name, "database is listed in Neon API");
                    return Ok(());
                }
            }

            // Wait 1 second before retry
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

            if attempts % 5 == 0 {
                debug!(
                    db_name,
                    attempts, max_attempts, "still waiting for database readiness"
                );
            }
        }

        Err(format!(
            "Timeout waiting for database '{}' to be ready",
            db_name
        ))
    }

    /// Wait until PostgreSQL accepts connections to the newly created database.
    ///
    /// Neon can return the database in the API list while its create operation is
    /// still running. A real connection check prevents provisioning from racing
    /// ahead into a database that is listed but not usable yet.
    pub async fn wait_for_database_connectable(&self, database_url: &str) -> Result<(), String> {
        info!("waiting for database to accept PostgreSQL connections");

        let max_attempts = 60;

        for attempt in 1..=max_attempts {
            match PgPoolOptions::new()
                .max_connections(1)
                .acquire_timeout(std::time::Duration::from_secs(5))
                .connect(database_url)
                .await
            {
                Ok(pool) => {
                    pool.close().await;
                    info!("database accepts PostgreSQL connections");
                    return Ok(());
                }
                Err(e) => {
                    if attempt % 5 == 0 {
                        debug!(
                            attempt,
                            max_attempts,
                            error = %e,
                            "still waiting for database connection"
                        );
                    }
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }

        Err("Timeout waiting for database to accept PostgreSQL connections".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::NeonClient;
    use axum::{
        http::StatusCode,
        routing::{get, post},
        Json, Router,
    };
    use serde_json::json;

    async fn start_neon_api(app: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test listener should bind");
        let address = listener
            .local_addr()
            .expect("test listener should have an address");
        let task = tokio::spawn(async move {
            axum::serve(listener, app)
                .await
                .expect("test Neon API should serve");
        });

        (format!("http://{address}/api/v2"), task)
    }

    #[tokio::test]
    async fn named_branch_is_resolved_before_creating_a_database() {
        let app = Router::new()
            .route(
                "/api/v2/projects/quiet-test-123/branches",
                get(|| async {
                    Json(json!({
                        "branches": [{
                            "id": "br-primary-abc123",
                            "project_id": "quiet-test-123",
                            "parent_id": null,
                            "parent_lsn": null,
                            "parent_timestamp": null,
                            "name": "main",
                            "current_state": "ready",
                            "pending_state": null,
                            "state_changed_at": "2026-08-26T00:00:00Z",
                            "creation_source": "console",
                            "primary": true,
                            "default": true,
                            "protected": false,
                            "cpu_used_sec": 0,
                            "compute_time_seconds": 0,
                            "active_time_seconds": 0,
                            "written_data_bytes": 0,
                            "data_transfer_bytes": 0,
                            "created_at": "2026-08-26T00:00:00Z",
                            "updated_at": "2026-08-26T00:00:00Z",
                            "created_by": {"name": "SchoolOrbit", "image": ""},
                            "init_source": "parent-data"
                        }],
                        "pagination": {"sort_by": "updated_at", "sort_order": "DESC"}
                    }))
                }),
            )
            .route(
                "/api/v2/projects/quiet-test-123/branches/br-primary-abc123/databases",
                post(|| async {
                    (
                        StatusCode::CREATED,
                        Json(json!({
                            "database": {
                                "id": 42,
                                "branch_id": "br-primary-abc123",
                                "name": "schoolorbit_demo",
                                "owner_name": "neondb_owner",
                                "created_at": "2026-08-26T00:00:00Z",
                                "updated_at": "2026-08-26T00:00:00Z"
                            },
                            "operations": []
                        })),
                    )
                }),
            );
        let (api_base_url, server) = start_neon_api(app).await;
        let client = NeonClient::from_config(
            reqwest::Client::new(),
            "test-api-key".to_string(),
            "quiet-test-123".to_string(),
            "main".to_string(),
            api_base_url,
        )
        .expect("test Neon configuration should be valid");

        let database_id = client
            .create_database("schoolorbit_demo", "neondb_owner")
            .await
            .expect("named branch should resolve to its Neon ID");

        assert_eq!(database_id, 42);
        server.abort();
    }

    #[tokio::test]
    async fn branch_not_found_response_does_not_leak_provider_details() {
        let app = Router::new().route(
            "/api/v2/projects/quiet-test-123/branches/br-missing/databases",
            post(|| async {
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({
                        "request_id": "provider-request-secret-detail",
                        "code": "",
                        "message": "branch not found"
                    })),
                )
            }),
        );
        let (api_base_url, server) = start_neon_api(app).await;
        let client = NeonClient::from_config(
            reqwest::Client::new(),
            "test-api-key".to_string(),
            "quiet-test-123".to_string(),
            "br-missing".to_string(),
            api_base_url,
        )
        .expect("test Neon configuration should be valid");

        let error = client
            .create_database("schoolorbit_demo", "neondb_owner")
            .await
            .expect_err("missing branch should fail");
        let message = error.to_string();

        assert!(message.contains("ไม่พบ Neon branch"));
        assert!(!message.contains("provider-request-secret-detail"));
        assert!(!message.contains("branch not found"));
        server.abort();
    }
}
