# Backend School Reliability Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make tenant pool reuse concurrency-safe, bound backend-admin calls with safe retry policy, and add strict deployment readiness to backend-school.

**Architecture:** Keep the existing `PoolManager`, `AdminClient`, and `AppState` boundaries. Add a per-tenant single-flight lock around pool creation, centralize retryable GET transport inside `AdminClient`, and move operational health handlers into the system module. Compose, smoke tests, and deploy workflows consume `/ready`; tenant databases and API business contracts remain unchanged.

**Tech Stack:** Rust 2021, Tokio, Axum 0.8, reqwest 0.12, sqlx 0.8, DashMap, Docker/Podman Compose, GitHub Actions shell steps.

## Global Constraints

- Do not add or edit a database migration.
- Do not change frontend behavior or generated OpenAPI operations; health/readiness stay outside OpenAPI.
- Keep backend-school single-replica assumptions; do not add a realtime backplane or distributed lock.
- Do not add a circuit-breaker framework in this iteration.
- Never log database URLs, credentials, internal headers/secrets, or raw response bodies.
- GET retries are limited to connection/timeout errors, `429`, and `5xx`; non-idempotent migration-status PUT is never retried.
- `/health` stays dependency-free; `/ready` checks backend-admin `/ready` and never opens or pings tenant databases.
- Use structured `tracing` logging and propagate errors without new `unwrap()` or `expect()` calls in runtime code.
- Follow red-green-refactor for every production behavior change.

---

### Task 1: Make tenant pool caching single-flight with sliding TTL

**Files:**
- Modify: `backend-school/src/db/pool_manager.rs`

**Interfaces:**
- Consumes: existing `MigrationTracker`, `PgPoolOptions`, and `PoolManager::get_pool(database_url, subdomain)` callers.
- Produces: unchanged public `get_pool()` API; private `get_or_create_pool_with()` test seam; per-subdomain creation locks; deterministic `PoolEntry` freshness/touch helpers.

- [ ] **Step 1: Write failing PoolEntry and concurrency tests**

Append a `#[cfg(test)]` module to `pool_manager.rs`. Use lazy sqlx pools so focused tests do not need PostgreSQL:

```rust
#[cfg(test)]
mod tests {
    use super::{PoolEntry, PoolManager};
    use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use std::time::{Duration, Instant};
    use tokio::sync::Barrier;

    fn lazy_pool(database: &str) -> sqlx::PgPool {
        PgPoolOptions::new().connect_lazy_with(
            PgConnectOptions::new()
                .host("127.0.0.1")
                .username("postgres")
                .database(database),
        )
    }

    #[test]
    fn cache_entry_freshness_uses_the_latest_touch() {
        let started = Instant::now();
        let mut entry = PoolEntry {
            pool: lazy_pool("touch_test"),
            last_used: started,
        };
        let ttl = Duration::from_secs(30);

        assert!(entry.is_fresh_at(started + Duration::from_secs(29), ttl));
        entry.touch(started + Duration::from_secs(20));
        assert!(entry.is_fresh_at(started + Duration::from_secs(49), ttl));
        assert!(!entry.is_fresh_at(started + Duration::from_secs(50), ttl));
    }

    #[tokio::test]
    async fn concurrent_misses_for_one_tenant_create_one_pool() {
        let manager = Arc::new(PoolManager::new());
        let creations = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();

        for _ in 0..20 {
            let manager = Arc::clone(&manager);
            let creations = Arc::clone(&creations);
            tasks.push(tokio::spawn(async move {
                manager
                    .get_or_create_pool_with("postgres://tenant-one", "tenant-one", || async move {
                        creations.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(20)).await;
                        Ok(lazy_pool("tenant_one"))
                    })
                    .await
            }));
        }

        for task in tasks {
            assert!(task.await.expect("pool task must join").is_ok());
        }
        assert_eq!(creations.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn different_tenants_create_pools_concurrently() {
        let manager = Arc::new(PoolManager::new());
        let entered = Arc::new(Barrier::new(2));

        let make_task = |subdomain: &'static str, database_url: &'static str| {
            let manager = Arc::clone(&manager);
            let entered = Arc::clone(&entered);
            tokio::spawn(async move {
                manager
                    .get_or_create_pool_with(database_url, subdomain, || async move {
                        entered.wait().await;
                        Ok(lazy_pool(subdomain))
                    })
                    .await
            })
        };

        let both = async {
            let first = make_task("tenant-a", "postgres://tenant-a");
            let second = make_task("tenant-b", "postgres://tenant-b");
            assert!(first.await.expect("first task must join").is_ok());
            assert!(second.await.expect("second task must join").is_ok());
        };

        tokio::time::timeout(Duration::from_millis(250), both)
            .await
            .expect("different tenants must not share a global creation lock");
    }

    #[tokio::test]
    async fn failed_creation_is_not_cached() {
        let manager = PoolManager::new();
        let creations = AtomicUsize::new(0);

        let first = manager
            .get_or_create_pool_with("postgres://retry", "retry", || async {
                creations.fetch_add(1, Ordering::SeqCst);
                Err("first creation failed".to_string())
            })
            .await;
        assert_eq!(first.expect_err("first creation must fail"), "first creation failed");

        let second = manager
            .get_or_create_pool_with("postgres://retry", "retry", || async {
                creations.fetch_add(1, Ordering::SeqCst);
                Ok(lazy_pool("retry"))
            })
            .await;
        assert!(second.is_ok());
        assert_eq!(creations.load(Ordering::SeqCst), 2);
    }
}
```

- [ ] **Step 2: Run the focused test and confirm RED**

Run:

```bash
cd backend-school
cargo test db::pool_manager::tests --bin backend-school
```

Expected: compilation fails because `PoolEntry::is_fresh_at`, `PoolEntry::touch`, and `PoolManager::get_or_create_pool_with` do not exist.

- [ ] **Step 3: Implement entry freshness, cache touch, and per-tenant single flight**

Change imports and structures in `pool_manager.rs`:

```rust
use dashmap::DashMap;
use std::future::Future;
use tokio::sync::{Mutex, RwLock};

struct PoolEntry {
    pool: PgPool,
    last_used: Instant,
}

impl PoolEntry {
    fn is_fresh_at(&self, now: Instant, ttl: Duration) -> bool {
        now.duration_since(self.last_used) < ttl
    }

    fn touch(&mut self, now: Instant) {
        self.last_used = now;
    }
}

pub struct PoolManager {
    pools: Arc<RwLock<HashMap<String, PoolEntry>>>,
    creation_locks: Arc<DashMap<String, Arc<Mutex<()>>>>,
    migration_tracker: MigrationTracker,
    max_connections_per_school: u32,
    pool_ttl: Duration,
}
```

Initialize `creation_locks` in `new()`, then add these private helpers:

```rust
async fn cached_pool_at(&self, key: &str, now: Instant) -> Option<PgPool> {
    let mut pools = self.pools.write().await;
    match pools.get_mut(key) {
        Some(entry) if entry.is_fresh_at(now, self.pool_ttl) => {
            entry.touch(now);
            Some(entry.pool.clone())
        }
        Some(_) => {
            pools.remove(key);
            None
        }
        None => None,
    }
}

async fn get_or_create_pool_with<F, Fut>(
    &self,
    database_url: &str,
    subdomain: &str,
    create_pool: F,
) -> Result<PgPool, String>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<PgPool, String>>,
{
    if let Some(pool) = self.cached_pool_at(database_url, Instant::now()).await {
        tracing::debug!(subdomain, "Using cached tenant database pool");
        return Ok(pool);
    }

    let creation_lock = self
        .creation_locks
        .entry(subdomain.to_string())
        .or_insert_with(|| Arc::new(Mutex::new(())))
        .clone();
    let _guard = creation_lock.lock().await;

    if let Some(pool) = self.cached_pool_at(database_url, Instant::now()).await {
        tracing::debug!(subdomain, "Using tenant pool created by a concurrent request");
        return Ok(pool);
    }

    let pool = create_pool().await?;
    self.pools.write().await.insert(
        database_url.to_string(),
        PoolEntry {
            pool: pool.clone(),
            last_used: Instant::now(),
        },
    );
    Ok(pool)
}
```

Refactor `get_pool()` so its pool-selection portion calls `get_or_create_pool_with()` and its factory contains the existing `PgConnectOptions`/`PgPoolOptions` configuration. Keep migration and permission synchronization after the helper returns:

```rust
let pool = self
    .get_or_create_pool_with(database_url, subdomain, || async {
        tracing::info!(subdomain, "Creating tenant database pool");
        let connect_options = PgConnectOptions::from_str(database_url)
            .map_err(|error| format!("Invalid database configuration for {subdomain}: {error}"))?
            .statement_cache_capacity(0);

        PgPoolOptions::new()
            .min_connections(0)
            .max_connections(self.max_connections_per_school)
            .acquire_timeout(Duration::from_secs(20))
            .idle_timeout(Duration::from_secs(300))
            .test_before_acquire(true)
            .connect_with(connect_options)
            .await
            .map_err(|error| format!("Failed to connect to database for {subdomain}: {error}"))
    })
    .await?;
```

Change `cleanup_expired()` to compute one `now` and use `entry.is_fresh_at(now, self.pool_ttl)` so cleanup and lookup share the same expiry rule.

- [ ] **Step 4: Run focused and migration tracker tests and confirm GREEN**

Run:

```bash
cd backend-school
cargo test db::pool_manager::tests --bin backend-school
cargo test db::migration::tests --bin backend-school
cargo fmt --all -- --check
```

Expected: all focused tests pass and formatting reports no diff.

- [ ] **Step 5: Commit the pool change**

```bash
git add backend-school/src/db/pool_manager.rs
git commit -m "fix(db): make tenant pool creation single flight"
```

---

### Task 2: Add bounded timeout and safe GET retries to AdminClient

**Files:**
- Modify: `backend-school/src/db/admin_client.rs`
- Modify: `backend-school/src/main.rs`

**Interfaces:**
- Consumes: `BACKEND_ADMIN_URL`, `INTERNAL_API_SECRET`, and existing AdminClient callers.
- Produces: `AdminClientConfig::from_env() -> Result<Self, String>`, `AdminClient::new(base_url, secret, config)`, retryable internal GET transport, and `AdminClient::check_readiness() -> Result<(), String>`.

- [ ] **Step 1: Add failing retry-policy, HTTP behavior, and config parser tests**

Add tests inside `admin_client.rs`. The local server helper must bind an ephemeral loopback port and shut down when its Tokio task is aborted:

```rust
#[cfg(test)]
mod tests {
    use super::{is_retryable_status, AdminClient, AdminClientConfig};
    use axum::{
        extract::State,
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
        let address = listener.local_addr().expect("listener must have an address");
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
    }

    #[tokio::test]
    async fn transient_get_is_retried_then_succeeds() {
        let attempts = Arc::new(AtomicUsize::new(0));
        let router = Router::new()
            .route(
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

        assert_eq!(client.get_db_url("sandbox").await.unwrap(), "postgres://tenant");
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
```

- [ ] **Step 2: Run the focused test and confirm RED**

Run:

```bash
cd backend-school
cargo test db::admin_client::tests --bin backend-school
```

Expected: compilation fails because `AdminClientConfig`, retry helpers, the three-argument constructor, and `check_readiness()` do not exist.

- [ ] **Step 3: Implement validated config and central retry transport**

Add configuration and policy near the top of `admin_client.rs`:

```rust
use reqwest::{Client, Response, StatusCode};
use std::time::Duration;

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
            std::env::var("BACKEND_ADMIN_REQUEST_TIMEOUT_MS").ok().as_deref(),
            std::env::var("BACKEND_ADMIN_RETRY_MAX_ATTEMPTS").ok().as_deref(),
            std::env::var("BACKEND_ADMIN_RETRY_BASE_DELAY_MS").ok().as_deref(),
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
    fn for_tests(request_timeout: Duration, max_attempts: usize, retry_base_delay: Duration) -> Self {
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
```

Add `config: AdminClientConfig` to `AdminClient`, change its constructor to trim trailing `/`, and centralize GET requests:

```rust
pub fn new(base_url: String, secret: String, config: AdminClientConfig) -> Self {
    Self {
        client: Client::new(),
        base_url: base_url.trim_end_matches('/').to_string(),
        secret,
        config,
    }
}

async fn get_with_retry(&self, path: &str, operation: &'static str) -> Result<Response, String> {
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
                tracing::warn!(operation, attempt, max_attempts = self.config.max_attempts, status = %response.status(), "Retrying transient backend-admin response");
            }
            Ok(response) => return Ok(response),
            Err(error)
                if (error.is_timeout() || error.is_connect())
                    && attempt < self.config.max_attempts =>
            {
                tracing::warn!(operation, attempt, max_attempts = self.config.max_attempts, timeout = error.is_timeout(), "Retrying transient backend-admin transport failure");
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
```

Replace the three GET call sites with `get_with_retry()` while preserving their response parsing and status-specific errors. Add:

```rust
pub async fn check_readiness(&self) -> Result<(), String> {
    let response = self.get_with_retry("/ready", "backend-admin readiness").await?;
    if response.status().is_success() {
        Ok(())
    } else {
        Err(format!("backend-admin readiness returned {}", response.status()))
    }
}
```

Add `.timeout(self.config.request_timeout)` to the migration-status PUT and do not route PUT through `get_with_retry()`.

In `main.rs`, construct config before the client and fail startup with a structured error and nonzero exit rather than a new panic:

```rust
let admin_client_config = match AdminClientConfig::from_env() {
    Ok(config) => config,
    Err(error) => {
        tracing::error!(error = %error, "Invalid backend-admin client configuration");
        std::process::exit(1);
    }
};
let admin_client = Arc::new(AdminClient::new(
    backend_admin_url,
    internal_secret,
    admin_client_config,
));
```

Update the import to `use db::admin_client::{AdminClient, AdminClientConfig};`.

- [ ] **Step 4: Run focused tests and confirm GREEN**

Run:

```bash
cd backend-school
cargo test db::admin_client::tests --bin backend-school
cargo test db::pool_manager::tests --bin backend-school
cargo fmt --all -- --check
```

Expected: all focused tests pass with no formatting diff.

- [ ] **Step 5: Commit the client reliability change**

```bash
git add backend-school/src/db/admin_client.rs backend-school/src/main.rs
git commit -m "feat(internal): bound backend-admin requests"
```

---

### Task 3: Add strict backend-school readiness

**Files:**
- Create: `backend-school/src/modules/system/handlers/health.rs`
- Modify: `backend-school/src/modules/system/handlers.rs`
- Modify: `backend-school/src/main.rs`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: `AppState.admin_client` and `AdminClient::check_readiness()` from Task 2.
- Produces: public `GET /health` liveness and `GET /ready` readiness; typed operational responses with camelCase `controlPlane`.

- [ ] **Step 1: Write failing handler-mapping and route-registration tests**

Export the test target from `modules/system/handlers.rs` first:

```rust
pub mod health;
```

Then create `modules/system/handlers/health.rs` with tests first and declarations that intentionally do not yet have implementations:

```rust
#[cfg(test)]
mod tests {
    use super::{liveness_response, readiness_response};
    use axum::http::StatusCode;

    #[test]
    fn liveness_is_dependency_free() {
        let (status, response) = liveness_response("2026-07-23T00:00:00Z".to_string());
        assert_eq!(status, StatusCode::OK);
        assert_eq!(response.status, "healthy");
    }

    #[test]
    fn available_control_plane_is_ready() {
        let (status, response) =
            readiness_response("2026-07-23T00:00:00Z".to_string(), Ok(()));
        assert_eq!(status, StatusCode::OK);
        assert_eq!(response.status, "ready");
        assert_eq!(response.control_plane, "connected");
    }

    #[test]
    fn unavailable_control_plane_fails_closed_without_internal_error() {
        let (status, response) = readiness_response(
            "2026-07-23T00:00:00Z".to_string(),
            Err("secret internal detail".to_string()),
        );
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.status, "not_ready");
        assert_eq!(response.control_plane, "unavailable");
        let json = serde_json::to_value(response).expect("readiness response must serialize");
        assert!(json.get("error").is_none());
        assert!(json.get("controlPlane").is_some());
    }
}
```

Add this failing guard to `static_architecture.rs`:

```rust
#[test]
fn backend_school_registers_separate_liveness_and_readiness_routes() {
    let main = read_source(repo_root().join("backend-school/src/main.rs"));
    let health = read_source(
        repo_root().join("backend-school/src/modules/system/handlers/health.rs"),
    );
    let health_route = Regex::new(r#"\.route\(\s*\"/health\""#).expect("valid health regex");
    let ready_route = Regex::new(r#"\.route\(\s*\"/ready\""#).expect("valid ready regex");

    assert!(health_route.is_match(&main));
    assert!(ready_route.is_match(&main));
    assert!(main.contains("handlers::health::health_check"));
    assert!(main.contains("handlers::health::readiness_check"));
    assert!(health.contains("check_readiness().await"));
    assert!(!health.contains("get_pool("));
    assert!(!health.contains("PgPool"));
}
```

- [ ] **Step 2: Run focused tests and confirm RED**

Run:

```bash
cd backend-school
cargo test modules::system::handlers::health::tests --bin backend-school
cargo test backend_school_registers_separate_liveness_and_readiness_routes --test static_architecture
```

Expected: the binary unit-test command fails to compile because `liveness_response()` and `readiness_response()` are missing. The separate integration-test command builds the normal binary without the `#[cfg(test)]` block and fails its route assertion because `/ready` is not registered.

- [ ] **Step 3: Implement typed liveness/readiness handlers and route registration**

Implement `health.rs` above its tests:

```rust
use crate::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct LivenessResponse {
    status: &'static str,
    timestamp: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadinessResponse {
    status: &'static str,
    control_plane: &'static str,
    timestamp: String,
}

fn liveness_response(timestamp: String) -> (StatusCode, LivenessResponse) {
    (
        StatusCode::OK,
        LivenessResponse {
            status: "healthy",
            timestamp,
        },
    )
}

fn readiness_response(
    timestamp: String,
    control_plane_result: Result<(), String>,
) -> (StatusCode, ReadinessResponse) {
    match control_plane_result {
        Ok(()) => (
            StatusCode::OK,
            ReadinessResponse {
                status: "ready",
                control_plane: "connected",
                timestamp,
            },
        ),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            ReadinessResponse {
                status: "not_ready",
                control_plane: "unavailable",
                timestamp,
            },
        ),
    }
}

pub async fn health_check() -> impl IntoResponse {
    let (status, response) = liveness_response(chrono::Utc::now().to_rfc3339());
    (status, Json(response))
}

pub async fn readiness_check(State(state): State<AppState>) -> impl IntoResponse {
    let result = state.admin_client.check_readiness().await;
    if let Err(error) = &result {
        tracing::warn!(error = %error, "Backend-school readiness check failed");
    }
    let (status, response) = readiness_response(chrono::Utc::now().to_rfc3339(), result);
    (status, Json(response))
}
```

Keep the `pub mod health;` export added during RED.

In `main.rs`, replace the old route and add readiness:

```rust
.route(
    "/health",
    get(modules::system::handlers::health::health_check),
)
.route(
    "/ready",
    get(modules::system::handlers::health::readiness_check),
)
```

Delete the local `health_check()` from `main.rs`. Keep `root_handler()` unchanged.

- [ ] **Step 4: Run handler, architecture, contract, and formatting tests**

Run:

```bash
cd backend-school
cargo test modules::system::handlers::health::tests --bin backend-school
cargo test backend_school_registers_separate_liveness_and_readiness_routes --test static_architecture
cargo test api_contract::tests --bin backend-school
cargo fmt --all -- --check
```

Expected: all commands pass; API operation count remains unchanged because operational endpoints stay outside OpenAPI.

- [ ] **Step 5: Commit readiness**

```bash
git add backend-school/src/modules/system/handlers/health.rs \
  backend-school/src/modules/system/handlers.rs \
  backend-school/src/main.rs \
  backend-school/tests/static_architecture.rs
git commit -m "feat(ops): add backend-school readiness"
```

---

### Task 4: Wire readiness into configuration, smoke tests, and deployment

**Files:**
- Modify: `.env.example`
- Modify: `backend-school/.env.example`
- Modify: `backend-school/.env.portainer.example`
- Modify: `docker-compose.yml`
- Modify: `podman-compose.yml`
- Modify: `.github/workflows/deploy-backend-admin.yml`
- Modify: `.github/workflows/deploy-backend-school.yml`
- Modify: `scripts/smoke_test.sh`
- Modify: `docs/TESTING.md`
- Modify: `IMPROVEMENT_PLAN.md`
- Modify: `backend-school/tests/static_architecture.rs`

**Interfaces:**
- Consumes: `/ready` from Task 3 and existing backend-admin `/ready`.
- Produces: documented env defaults, container readiness probes, bounded 60-second post-deploy waits, and sandbox smoke coverage for liveness plus readiness.

- [ ] **Step 1: Add failing repository configuration guards**

Add a focused static test to `backend-school/tests/static_architecture.rs`:

```rust
#[test]
fn deployment_and_smoke_checks_use_backend_readiness() {
    let docker_compose = read_source(repo_root().join("docker-compose.yml"));
    let podman_compose = read_source(repo_root().join("podman-compose.yml"));
    let school_deploy =
        read_source(repo_root().join(".github/workflows/deploy-backend-school.yml"));
    let admin_deploy =
        read_source(repo_root().join(".github/workflows/deploy-backend-admin.yml"));
    let smoke = read_source(repo_root().join("scripts/smoke_test.sh"));

    for compose in [&docker_compose, &podman_compose] {
        assert!(compose.contains("http://localhost:8080/ready"));
        assert!(compose.contains("http://localhost:8081/ready"));
        assert!(compose.contains("BACKEND_ADMIN_REQUEST_TIMEOUT_MS"));
        assert!(compose.contains("BACKEND_ADMIN_RETRY_MAX_ATTEMPTS"));
        assert!(compose.contains("BACKEND_ADMIN_RETRY_BASE_DELAY_MS"));
    }
    assert!(school_deploy.contains("http://127.0.0.1:8081/ready"));
    assert!(admin_deploy.contains("http://127.0.0.1:8080/ready"));
    assert!(school_deploy.contains("seq 1 12"));
    assert!(admin_deploy.contains("seq 1 12"));
    assert!(smoke.contains("$SMOKE_ADMIN_API_URL/ready"));
    assert!(smoke.contains("$SMOKE_API_URL/ready"));
}
```

- [ ] **Step 2: Run the guard and confirm RED**

Run:

```bash
cd backend-school
cargo test deployment_and_smoke_checks_use_backend_readiness --test static_architecture
```

Expected: failure because Compose, deploy workflows, and smoke checks still use only `/health` and do not expose AdminClient retry configuration.

- [ ] **Step 3: Add environment defaults and Compose readiness probes**

Add these documented settings beside `BACKEND_ADMIN_URL` in all three environment examples:

```dotenv
# Backend-admin request reliability (optional; bounds enforced at startup)
BACKEND_ADMIN_REQUEST_TIMEOUT_MS=5000
BACKEND_ADMIN_RETRY_MAX_ATTEMPTS=3
BACKEND_ADMIN_RETRY_BASE_DELAY_MS=100
```

Pass the same values into backend-school in both Compose files using existing syntax and defaults.

Add backend-admin and backend-school health checks to `docker-compose.yml`, and change the existing Podman probes to:

```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:8081/ready"]
  interval: 30s
  timeout: 20s
  retries: 3
  start_period: 40s
```

Use port `8080` for backend-admin. Preserve `depends_on: backend-admin: condition: service_healthy` for backend-school.

- [ ] **Step 4: Add bounded post-start readiness waits to both deploy workflows**

Immediately after each `podman-compose up -d ...`, add the service-specific form of this loop:

```bash
for attempt in $(seq 1 12); do
  if curl -fsS http://127.0.0.1:8081/ready >/dev/null; then
    break
  fi
  if [ "$attempt" -eq 12 ]; then
    podman logs --tail 100 schoolorbit-backend-school
    exit 1
  fi
  sleep 5
done
```

Use port `8080` and container `schoolorbit-backend-admin` in the admin workflow. Keep image cleanup and Nginx reload after readiness succeeds.

- [ ] **Step 5: Extend sandbox smoke checks and documentation**

In `scripts/smoke_test.sh`, keep both liveness calls and add:

```bash
admin_ready_headers="$tmp_dir/admin-ready.headers"
admin_ready_body="$tmp_dir/admin-ready.body"
status="$(request "admin readiness" GET "$SMOKE_ADMIN_API_URL/ready" "$admin_ready_headers" "$admin_ready_body")"
expect_status "admin readiness" "$status" "200"
expect_body_contains "admin readiness" "$admin_ready_body" '"status":"ready"'

ready_headers="$tmp_dir/ready.headers"
ready_body="$tmp_dir/ready.body"
status="$(request "school API readiness" GET "$SMOKE_API_URL/ready" "$ready_headers" "$ready_body")"
expect_status "school API readiness" "$status" "200"
expect_body_contains "school API readiness" "$ready_body" '"status":"ready"'
expect_body_contains "school API readiness" "$ready_body" '"controlPlane":"connected"'
```

Update `docs/TESTING.md` to list both liveness and readiness calls and explain that backend-school readiness checks only backend-admin control-plane readiness, not every tenant database.

Update `IMPROVEMENT_PLAN.md`:

- leave L-1 open, but record that bounded timeout and exponential GET retry are complete groundwork while circuit breaker remains deferred;
- rewrite L-2 so it accurately states backend-admin checks its database and backend-school checks backend-admin control-plane readiness without waking tenant databases.

- [ ] **Step 6: Run focused configuration verification and confirm GREEN**

Run:

```bash
cd backend-school
cargo test deployment_and_smoke_checks_use_backend_readiness --test static_architecture
cd ..
bash -n scripts/smoke_test.sh
git diff --check
```

If Docker Compose is available, also run:

```bash
docker compose config --quiet
```

Expected: focused guard and shell syntax pass; Compose validates when the local Docker Compose plugin is available.

- [ ] **Step 7: Commit deployment and documentation changes**

```bash
git add .env.example \
  backend-school/.env.example \
  backend-school/.env.portainer.example \
  docker-compose.yml \
  podman-compose.yml \
  .github/workflows/deploy-backend-admin.yml \
  .github/workflows/deploy-backend-school.yml \
  scripts/smoke_test.sh \
  docs/TESTING.md \
  IMPROVEMENT_PLAN.md \
  backend-school/tests/static_architecture.rs
git commit -m "ops: enforce backend readiness checks"
```

---

### Task 5: Full verification, review, and integration readiness

**Files:**
- No expected source changes; fix only defects exposed by verification in focused commits.

**Interfaces:**
- Consumes: Tasks 1-4.
- Produces: a reviewed, clean feature branch with fresh evidence for unit, integration, static, config, and documentation behavior.

- [ ] **Step 1: Run strict backend checks**

Run:

```bash
cd backend-school
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
```

Expected: both commands exit zero with no warnings.

- [ ] **Step 2: Run the complete backend suite against supported PostgreSQL**

Start a disposable PostgreSQL 17 database, create `uuid-ossp` and `pg_trgm` in `public`, export `TEST_DATABASE_URL`, and run the full suite with automatic cleanup:

```bash
verification_container="schoolorbit-reliability-verification-$$"
cleanup_verification_db() {
  docker rm -f "$verification_container" >/dev/null 2>&1 || true
}
trap cleanup_verification_db EXIT

docker run -d --name "$verification_container" \
  -e POSTGRES_PASSWORD=postgres \
  -e POSTGRES_DB=schoolorbit_test \
  -p 127.0.0.1::5432 \
  postgres:17-alpine >/dev/null

for attempt in $(seq 1 30); do
  if docker exec "$verification_container" \
    pg_isready -U postgres -d schoolorbit_test >/dev/null 2>&1; then
    break
  fi
  if [ "$attempt" -eq 30 ]; then
    exit 1
  fi
  sleep 1
done

docker exec "$verification_container" psql -v ON_ERROR_STOP=1 \
  -U postgres -d schoolorbit_test \
  -c 'CREATE EXTENSION IF NOT EXISTS "uuid-ossp" WITH SCHEMA public; CREATE EXTENSION IF NOT EXISTS pg_trgm WITH SCHEMA public;'

verification_port=$(docker port "$verification_container" 5432/tcp | sed 's/.*://')
TEST_DATABASE_URL="postgres://postgres:postgres@127.0.0.1:${verification_port}/schoolorbit_test" \
  cargo test --all-targets --all-features -- --test-threads=1
```

Expected current suite shape: the main binary tests, OpenAPI exporter tests, logging tests, and static architecture tests all pass; exact counts may increase with Tasks 1-4.

- [ ] **Step 3: Run repository/configuration checks**

From the repository root:

```bash
bash -n scripts/smoke_test.sh
docker compose config --quiet
git diff --check
git status --short --branch
```

Expected: syntax/config/diff checks pass and status contains only intentional committed feature history.

- [ ] **Step 4: Audit the requirements directly**

Run:

```bash
rg -n 'creation_locks|get_or_create_pool_with|entry\.touch' backend-school/src/db/pool_manager.rs
rg -n 'BACKEND_ADMIN_REQUEST_TIMEOUT_MS|BACKEND_ADMIN_RETRY_MAX_ATTEMPTS|BACKEND_ADMIN_RETRY_BASE_DELAY_MS|check_readiness|get_with_retry' backend-school/src/db/admin_client.rs
rg -n 'route\("/health"|route\("/ready"|handlers::health' backend-school/src/main.rs
rg -n 'localhost:808[01]/ready|127\.0\.0\.1:808[01]/ready' docker-compose.yml podman-compose.yml .github/workflows/deploy-backend-*.yml
git diff --name-only 4a734312..HEAD -- backend-school/migrations frontend-school
```

Expected: reliability symbols and readiness consumers are present; the final command prints no migration or frontend path.

- [ ] **Step 5: Request final code review and address only verified findings**

Review against the approved design with special attention to:

- deadlocks or lock scope spanning pool creation;
- retries accidentally applied to PUT;
- secret/database URL leakage;
- worst-case readiness duration versus probe timeout;
- tenant DB access from `/ready`;
- deployment loops that can report success before readiness.

Apply the receiving-code-review workflow to any findings, reproduce each issue, add or strengthen a failing test first, and commit each focused correction.

- [ ] **Step 6: Re-run all verification after review fixes**

Repeat Steps 1-4 from the final reviewed commit. Do not claim completion, merge, or push based on earlier runs.

- [ ] **Step 7: Finish the branch**

Use `superpowers:finishing-a-development-branch` after all final commands pass. Present merge/push choices unless the user has already explicitly authorized integration for this branch.

## Plan self-review

- **Spec coverage:** Tasks 1-4 map directly to single-flight/sliding TTL, AdminClient timeout/retry, strict readiness, and deployment integration; Task 5 covers every success criterion.
- **Scope:** No migration, frontend feature, circuit breaker, distributed system, or OpenAPI expansion is included.
- **Type consistency:** `AdminClientConfig`, `AdminClient::check_readiness`, `PoolManager::get_or_create_pool_with`, `readiness_response`, and camelCase `controlPlane` use the same names in producing and consuming tasks.
- **Failure boundaries:** pool factory errors are not cached; GET retry classes are explicit; PUT is single-attempt; readiness hides internal error details and never resolves tenant pools.
