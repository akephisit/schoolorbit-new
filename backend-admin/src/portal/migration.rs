//! Operator-only migration transport. Customer-facing Workers never hold this credential.
use super::PortalError;
use axum::Json;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct SchoolStatus {
    pub subdomain: String,
    pub migration_version: i64,
    pub migration_status: String,
    pub last_migrated_at: Option<String>,
    pub migration_error: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub struct Status {
    pub total_schools: i64,
    pub migrated: i64,
    pub pending: i64,
    pub failed: i64,
    pub outdated: i64,
    pub active_pools: i64,
    pub latest_version: i64,
    pub schools: Vec<SchoolStatus>,
}
#[derive(Serialize, Deserialize)]
pub struct ResultItem {
    pub subdomain: String,
    pub status: String,
    pub version: Option<i64>,
    pub error: Option<String>,
}
#[derive(Serialize, Deserialize)]
pub struct ResultBody {
    pub total: i64,
    pub success: i64,
    pub failed: i64,
    pub latest_version: i64,
    pub results: Vec<ResultItem>,
}
async fn call<T: serde::de::DeserializeOwned>(
    endpoint: &str,
    method: reqwest::Method,
) -> Result<Json<T>, PortalError> {
    let base = std::env::var("BACKEND_SCHOOL_URL").map_err(|_| PortalError::unavailable())?;
    let secret = std::env::var("INTERNAL_API_SECRET").map_err(|_| PortalError::unavailable())?;
    let url = reqwest::Url::parse(&base)
        .and_then(|u| u.join(endpoint))
        .map_err(|_| PortalError::unavailable())?;
    let response = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|_| PortalError::unavailable())?
        .request(method, url)
        .header("X-Internal-Secret", secret)
        .header("X-Internal-Caller", "backend-admin")
        .send()
        .await
        .map_err(|_| PortalError::unavailable())?;
    if !response.status().is_success() {
        return Err(PortalError::unavailable());
    }
    Ok(Json(
        response
            .json()
            .await
            .map_err(|_| PortalError::unavailable())?,
    ))
}
pub async fn status() -> Result<Json<Status>, PortalError> {
    call("/internal/migration-status", reqwest::Method::GET).await
}
pub async fn migrate_all() -> Result<Json<ResultBody>, PortalError> {
    call("/internal/migrate-all", reqwest::Method::POST).await
}
