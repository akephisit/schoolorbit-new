use reqwest::Client;
use serde::{Deserialize, Serialize};

/// HTTP client for communicating with backend-admin's internal API.
/// Replaces direct admin database access — backend-school no longer needs ADMIN_DATABASE_URL.
#[derive(Clone)]
pub struct AdminClient {
    client: Client,
    base_url: String,
    secret: String,
}

#[derive(Debug, Deserialize)]
struct SchoolDbInfo {
    db_connection_string: Option<String>,
}

/// School info returned by the list endpoint, includes migration metadata
#[derive(Debug, Deserialize)]
pub struct ActiveSchool {
    pub subdomain: String,
    pub db_connection_string: Option<String>,
    pub migration_version: Option<i32>,
    pub migration_status: Option<String>,
    pub last_migrated_at: Option<String>,
    pub migration_error: Option<String>,
}

#[derive(Deserialize)]
struct ListSchoolsResponse {
    schools: Vec<ActiveSchool>,
}

#[derive(Serialize)]
struct UpdateMigrationStatusPayload {
    migration_version: i32,
    migration_status: String,
    migration_error: Option<String>,
}

impl AdminClient {
    pub fn new(base_url: String, secret: String) -> Self {
        Self {
            client: Client::new(),
            base_url,
            secret,
        }
    }

    /// Fetch the tenant database URL for a given subdomain.
    /// Called on every cold-start of a tenant pool (cached 30 min by PoolManager).
    pub async fn get_db_url(&self, subdomain: &str) -> Result<String, String> {
        let url = format!("{}/internal/schools/{}", self.base_url, subdomain);

        let resp = self
            .client
            .get(&url)
            .header("X-Internal-Secret", &self.secret)
            .send()
            .await
            .map_err(|e| format!("Failed to reach admin service: {}", e))?;

        if resp.status().as_u16() == 404 {
            return Err(format!("School '{}' not found or inactive", subdomain));
        }
        if !resp.status().is_success() {
            return Err(format!(
                "Admin service returned error: {}",
                resp.status()
            ));
        }

        let info: SchoolDbInfo = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse admin response: {}", e))?;

        info.db_connection_string
            .ok_or_else(|| format!("School '{}' has no database configured", subdomain))
    }

    /// Fetch all active schools with their db_connection_string and migration metadata.
    /// Used by the cleanup job and migrate-all handler.
    pub async fn list_active_schools(&self) -> Result<Vec<ActiveSchool>, String> {
        let url = format!("{}/internal/schools?status=active", self.base_url);

        let resp = self
            .client
            .get(&url)
            .header("X-Internal-Secret", &self.secret)
            .send()
            .await
            .map_err(|e| format!("Failed to reach admin service: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!(
                "Admin service returned error: {}",
                resp.status()
            ));
        }

        let list: ListSchoolsResponse = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse schools list: {}", e))?;

        Ok(list.schools)
    }

    /// Write migration status back to backend-admin after migrating a tenant database.
    pub async fn update_migration_status(
        &self,
        subdomain: &str,
        version: i32,
        status: &str,
        error: Option<&str>,
    ) -> Result<(), String> {
        let url = format!(
            "{}/internal/schools/{}/migration-status",
            self.base_url, subdomain
        );

        let resp = self
            .client
            .put(&url)
            .header("X-Internal-Secret", &self.secret)
            .json(&UpdateMigrationStatusPayload {
                migration_version: version,
                migration_status: status.to_string(),
                migration_error: error.map(|e| e.to_string()),
            })
            .send()
            .await
            .map_err(|e| format!("Failed to reach admin service: {}", e))?;

        if !resp.status().is_success() {
            return Err(format!(
                "Admin service returned error: {}",
                resp.status()
            ));
        }

        Ok(())
    }
}
