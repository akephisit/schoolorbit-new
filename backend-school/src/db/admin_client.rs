use reqwest::{Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;

const INTERNAL_CALLER: &str = "backend-school";
const INTERNAL_CALLER_HEADER: &str = "X-Internal-Caller";
const INTERNAL_SECRET_HEADER: &str = "X-Internal-Secret";
const DEFAULT_REQUEST_TIMEOUT_MS: u64 = 5_000;
const DEFAULT_MAX_ATTEMPTS: usize = 3;
const DEFAULT_RETRY_BASE_DELAY_MS: u64 = 100;

#[derive(Clone, Debug)]
pub struct AdminClientConfig {
    request_timeout: Duration,
    max_attempts: usize,
    retry_base_delay: Duration,
}

impl AdminClientConfig {
    pub fn from_env() -> Result<Self, String> {
        Self::from_values(
            std::env::var("BACKEND_ADMIN_REQUEST_TIMEOUT_MS")
                .ok()
                .as_deref(),
            std::env::var("BACKEND_ADMIN_RETRY_MAX_ATTEMPTS")
                .ok()
                .as_deref(),
            std::env::var("BACKEND_ADMIN_RETRY_BASE_DELAY_MS")
                .ok()
                .as_deref(),
        )
    }

    fn from_values(
        timeout: Option<&str>,
        attempts: Option<&str>,
        base_delay: Option<&str>,
    ) -> Result<Self, String> {
        Ok(Self {
            request_timeout: Duration::from_millis(parse_bounded(
                "BACKEND_ADMIN_REQUEST_TIMEOUT_MS",
                timeout,
                DEFAULT_REQUEST_TIMEOUT_MS,
                100,
                30_000,
            )?),
            max_attempts: parse_bounded(
                "BACKEND_ADMIN_RETRY_MAX_ATTEMPTS",
                attempts,
                DEFAULT_MAX_ATTEMPTS,
                1,
                5,
            )?,
            retry_base_delay: Duration::from_millis(parse_bounded(
                "BACKEND_ADMIN_RETRY_BASE_DELAY_MS",
                base_delay,
                DEFAULT_RETRY_BASE_DELAY_MS,
                1,
                5_000,
            )?),
        })
    }

    #[cfg(test)]
    fn for_tests(
        request_timeout: Duration,
        max_attempts: usize,
        retry_base_delay: Duration,
    ) -> Self {
        Self {
            request_timeout,
            max_attempts,
            retry_base_delay,
        }
    }
}

fn parse_bounded<T>(
    name: &str,
    value: Option<&str>,
    default: T,
    minimum: T,
    maximum: T,
) -> Result<T, String>
where
    T: Copy + Ord + std::str::FromStr,
{
    let parsed = match value {
        Some(raw) => raw
            .parse::<T>()
            .map_err(|_| format!("{name} must be a valid integer"))?,
        None => default,
    };
    if parsed < minimum || parsed > maximum {
        return Err(format!("{name} is outside the supported range"));
    }
    Ok(parsed)
}

fn is_retryable_status(status: StatusCode) -> bool {
    status == StatusCode::TOO_MANY_REQUESTS || status.is_server_error()
}

/// HTTP client for communicating with backend-admin's internal API.
/// Replaces direct admin database access — backend-school no longer needs ADMIN_DATABASE_URL.
#[derive(Clone)]
pub struct AdminClient {
    client: Client,
    base_url: String,
    secret: String,
    config: AdminClientConfig,
}

#[derive(Debug, Deserialize)]
struct SchoolDbInfo {
    db_connection_string: Option<String>,
    name: Option<String>,
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
    pub fn new(base_url: String, secret: String, config: AdminClientConfig) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.trim_end_matches('/').to_string(),
            secret,
            config,
        }
    }

    async fn get_with_retry(
        &self,
        path: &str,
        operation: &'static str,
    ) -> Result<Response, String> {
        let url = format!("{}{}", self.base_url, path);

        for attempt in 1..=self.config.max_attempts {
            let result = self
                .client
                .get(&url)
                .header(INTERNAL_SECRET_HEADER, &self.secret)
                .header(INTERNAL_CALLER_HEADER, INTERNAL_CALLER)
                .timeout(self.config.request_timeout)
                .send()
                .await;

            match result {
                Ok(response)
                    if is_retryable_status(response.status())
                        && attempt < self.config.max_attempts =>
                {
                    tracing::warn!(
                        operation,
                        attempt,
                        max_attempts = self.config.max_attempts,
                        status = %response.status(),
                        "Retrying transient backend-admin response"
                    );
                }
                Ok(response) => return Ok(response),
                Err(error)
                    if (error.is_timeout() || error.is_connect())
                        && attempt < self.config.max_attempts =>
                {
                    tracing::warn!(
                        operation,
                        attempt,
                        max_attempts = self.config.max_attempts,
                        timeout = error.is_timeout(),
                        "Retrying transient backend-admin transport failure"
                    );
                }
                Err(error) if error.is_timeout() => {
                    return Err(format!("{operation} timed out"));
                }
                Err(_) => return Err(format!("{operation} could not reach backend-admin")),
            }

            let multiplier = 1_u32 << (attempt.saturating_sub(1) as u32);
            tokio::time::sleep(self.config.retry_base_delay.saturating_mul(multiplier)).await;
        }

        Err(format!("{operation} exhausted its retry attempts"))
    }

    /// Fetch the tenant database URL for a given subdomain.
    /// Called on every cold-start of a tenant pool (cached 30 min by PoolManager).
    pub async fn get_db_url(&self, subdomain: &str) -> Result<String, String> {
        let resp = self
            .get_with_retry(
                &format!("/internal/schools/{subdomain}"),
                "school database lookup",
            )
            .await?;

        if resp.status().as_u16() == 404 {
            return Err(format!("School '{}' not found or inactive", subdomain));
        }
        if !resp.status().is_success() {
            return Err(format!("Admin service returned error: {}", resp.status()));
        }

        let info: SchoolDbInfo = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse admin response: {}", e))?;

        info.db_connection_string
            .ok_or_else(|| format!("School '{}' has no database configured", subdomain))
    }

    /// Fetch the school name from backend-admin for a given subdomain.
    pub async fn get_school_name(&self, subdomain: &str) -> Result<String, String> {
        let resp = self
            .get_with_retry(
                &format!("/internal/schools/{subdomain}"),
                "school name lookup",
            )
            .await?;

        if !resp.status().is_success() {
            return Err(format!("Admin service returned error: {}", resp.status()));
        }

        let info: SchoolDbInfo = resp
            .json()
            .await
            .map_err(|e| format!("Failed to parse admin response: {}", e))?;

        info.name
            .ok_or_else(|| "School name not available".to_string())
    }

    /// Fetch all active schools with their db_connection_string and migration metadata.
    /// Used by the cleanup job and migrate-all handler.
    pub async fn list_active_schools(&self) -> Result<Vec<ActiveSchool>, String> {
        let resp = self
            .get_with_retry("/internal/schools?status=active", "active school listing")
            .await?;

        if !resp.status().is_success() {
            return Err(format!("Admin service returned error: {}", resp.status()));
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
            .header(INTERNAL_SECRET_HEADER, &self.secret)
            .header(INTERNAL_CALLER_HEADER, INTERNAL_CALLER)
            .json(&UpdateMigrationStatusPayload {
                migration_version: version,
                migration_status: status.to_string(),
                migration_error: error.map(|e| e.to_string()),
            })
            .timeout(self.config.request_timeout)
            .send()
            .await
            .map_err(|error| {
                if error.is_timeout() {
                    "migration status update timed out".to_string()
                } else {
                    "migration status update could not reach backend-admin".to_string()
                }
            })?;

        if !resp.status().is_success() {
            return Err(format!("Admin service returned error: {}", resp.status()));
        }

        Ok(())
    }

    pub async fn check_readiness(&self) -> Result<(), String> {
        let response = self
            .get_with_retry("/ready", "backend-admin readiness")
            .await?;
        if response.status().is_success() {
            Ok(())
        } else {
            Err(format!(
                "backend-admin readiness returned {}",
                response.status()
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{is_retryable_status, AdminClient, AdminClientConfig};
    use axum::{
        http::StatusCode,
        routing::{get, put},
        Json, Router,
    };
    use serde_json::json;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use std::time::Duration;

    fn test_config(timeout: Duration, attempts: usize) -> AdminClientConfig {
        AdminClientConfig::for_tests(timeout, attempts, Duration::from_millis(1))
    }

    async fn spawn_server(router: Router) -> (String, tokio::task::JoinHandle<()>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test listener must bind");
        let address = listener
            .local_addr()
            .expect("listener must have an address");
        let task = tokio::spawn(async move {
            axum::serve(listener, router)
                .await
                .expect("test server must run");
        });
        (format!("http://{address}"), task)
    }

    #[test]
    fn retryable_statuses_are_limited_to_rate_limit_and_server_errors() {
        assert!(is_retryable_status(StatusCode::TOO_MANY_REQUESTS));
        assert!(is_retryable_status(StatusCode::INTERNAL_SERVER_ERROR));
        assert!(is_retryable_status(StatusCode::SERVICE_UNAVAILABLE));
        assert!(!is_retryable_status(StatusCode::BAD_REQUEST));
        assert!(!is_retryable_status(StatusCode::UNAUTHORIZED));
        assert!(!is_retryable_status(StatusCode::NOT_FOUND));
    }

    #[test]
    fn environment_values_are_bounded() {
        assert!(AdminClientConfig::from_values(Some("5000"), Some("3"), Some("100")).is_ok());
        assert!(AdminClientConfig::from_values(Some("0"), Some("3"), Some("100")).is_err());
        assert!(AdminClientConfig::from_values(Some("30001"), Some("3"), Some("100")).is_err());
        assert!(AdminClientConfig::from_values(Some("5000"), Some("0"), Some("100")).is_err());
        assert!(AdminClientConfig::from_values(Some("5000"), Some("6"), Some("100")).is_err());
        assert!(AdminClientConfig::from_values(Some("5000"), Some("3"), Some("5001")).is_err());
        assert!(AdminClientConfig::from_values(Some("invalid"), None, None).is_err());
    }

    #[tokio::test]
    async fn transient_get_is_retried_then_succeeds() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let router = Router::new().route(
            "/internal/schools/sandbox",
            get({
                let attempts = Arc::clone(&attempts);
                move || async move {
                    if attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                        return (StatusCode::SERVICE_UNAVAILABLE, Json(json!({})));
                    }
                    (
                        StatusCode::OK,
                        Json(json!({"db_connection_string": "postgres://tenant"})),
                    )
                }
            }),
        );
        let (base_url, server) = spawn_server(router).await;
        let client = AdminClient::new(
            base_url,
            "test-secret".to_string(),
            test_config(Duration::from_millis(100), 3),
        );

        assert_eq!(
            client
                .get_db_url("sandbox")
                .await
                .expect("transient GET must recover"),
            "postgres://tenant"
        );
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        server.abort();
    }

    #[tokio::test]
    async fn not_found_get_is_not_retried() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let router = Router::new().route(
            "/internal/schools/missing",
            get({
                let attempts = Arc::clone(&attempts);
                move || async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    StatusCode::NOT_FOUND
                }
            }),
        );
        let (base_url, server) = spawn_server(router).await;
        let client = AdminClient::new(
            base_url,
            "test-secret".to_string(),
            test_config(Duration::from_millis(100), 3),
        );

        assert!(client.get_db_url("missing").await.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        server.abort();
    }

    #[tokio::test]
    async fn timed_out_get_stops_at_the_attempt_limit() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let router = Router::new().route(
            "/ready",
            get({
                let attempts = Arc::clone(&attempts);
                move || async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(40)).await;
                    StatusCode::OK
                }
            }),
        );
        let (base_url, server) = spawn_server(router).await;
        let client = AdminClient::new(
            base_url,
            "test-secret".to_string(),
            test_config(Duration::from_millis(5), 2),
        );

        assert!(client.check_readiness().await.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 2);
        server.abort();
    }

    #[tokio::test]
    async fn repeated_transient_status_stops_at_the_attempt_limit() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let router = Router::new().route(
            "/ready",
            get({
                let attempts = Arc::clone(&attempts);
                move || async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    StatusCode::SERVICE_UNAVAILABLE
                }
            }),
        );
        let (base_url, server) = spawn_server(router).await;
        let client = AdminClient::new(
            base_url,
            "test-secret".to_string(),
            test_config(Duration::from_millis(100), 3),
        );

        assert!(client.check_readiness().await.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 3);
        server.abort();
    }

    #[tokio::test]
    async fn successful_readiness_is_accepted_without_retry() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let router = Router::new().route(
            "/ready",
            get({
                let attempts = Arc::clone(&attempts);
                move || async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    StatusCode::OK
                }
            }),
        );
        let (base_url, server) = spawn_server(router).await;
        let client = AdminClient::new(
            base_url,
            "test-secret".to_string(),
            test_config(Duration::from_millis(100), 3),
        );

        assert!(client.check_readiness().await.is_ok());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        server.abort();
    }

    #[tokio::test]
    async fn successful_invalid_json_is_not_retried() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let router = Router::new().route(
            "/internal/schools/broken",
            get({
                let attempts = Arc::clone(&attempts);
                move || async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    (StatusCode::OK, "not-json")
                }
            }),
        );
        let (base_url, server) = spawn_server(router).await;
        let client = AdminClient::new(
            base_url,
            "test-secret".to_string(),
            test_config(Duration::from_millis(100), 3),
        );

        assert!(client.get_db_url("broken").await.is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        server.abort();
    }

    #[tokio::test]
    async fn migration_status_put_is_never_retried() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let router = Router::new().route(
            "/internal/schools/sandbox/migration-status",
            put({
                let attempts = Arc::clone(&attempts);
                move || async move {
                    attempts.fetch_add(1, Ordering::SeqCst);
                    StatusCode::SERVICE_UNAVAILABLE
                }
            }),
        );
        let (base_url, server) = spawn_server(router).await;
        let client = AdminClient::new(
            base_url,
            "test-secret".to_string(),
            test_config(Duration::from_millis(100), 3),
        );

        assert!(client
            .update_migration_status("sandbox", 28, "completed", None)
            .await
            .is_err());
        assert_eq!(attempts.load(Ordering::SeqCst), 1);
        server.abort();
    }
}
