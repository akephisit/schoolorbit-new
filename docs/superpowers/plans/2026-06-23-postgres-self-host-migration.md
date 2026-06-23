# PostgreSQL Self-Host Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move SchoolOrbit database provisioning and deployment from Neon-only PostgreSQL to PostgreSQL 18 self-hosted with Databasus backup support.

**Architecture:** Add a provider boundary inside `backend-admin` so `SchoolService` can create/delete tenant databases through either Neon or a self-hosted PostgreSQL cluster. Keep `backend-school` tenant runtime mostly unchanged because it already consumes tenant database URLs and runs clean-baseline migrations. Deploy PostgreSQL 18 and Databasus side-by-side first, then use self-hosted provisioning for the sandbox tenant before production cutover.

**Tech Stack:** Rust, Axum, SQLx PostgreSQL, PostgreSQL 18, Podman Compose, Databasus, existing tenant migration scripts.

---

## File Structure

- Create `backend-admin/src/services/database_provider.rs`: provider config, provider enum, Neon/self-host create/delete/connectable operations, SQL identifier quoting helpers, and unit tests.
- Modify `backend-admin/src/services/mod.rs`: export the new provider module.
- Modify `backend-admin/src/models/school.rs`: add provider metadata fields to `SchoolConfig` while keeping existing JSON compatibility.
- Modify `backend-admin/src/services/school_service.rs`: replace direct `NeonClient` calls with `DatabaseProvider`, update rollback cleanup, provisioning progress labels, delete path, and tests.
- Modify `backend-admin/src/clients/mod.rs`: keep `neon_client` exported while provider still supports rollback.
- Modify `backend-admin/src/main.rs`: update startup log from Neon-specific wording.
- Modify `podman-compose.yml`: add PostgreSQL 18 and Databasus services, wire `backend-admin` to self-host provider env vars.
- Modify `docker-compose.yml`: update local Postgres to PostgreSQL 18 and set local self-host provider defaults.
- Modify `.env.example`, `backend-admin/.env.example`, and `backend-school/.env.portainer.example`: document self-host provider variables and make Neon variables optional.
- Create `docs/POSTGRES_SELF_HOST_MIGRATION.md`: operator runbook for roles, admin DB migration, sandbox provisioning, tenant cutover, Databasus backup setup, and rollback.

## Scope Notes

- Do not edit old SQLx migration files.
- Do not add logical replication.
- Do not change tenant isolation. Keep one database per school.
- Do not expose database URLs or passwords in logs.
- Do not move Cloudflare R2 file storage.

---

### Task 1: Add Provider Config and Metadata Model

**Files:**
- Create: `backend-admin/src/services/database_provider.rs`
- Modify: `backend-admin/src/services/mod.rs`
- Modify: `backend-admin/src/models/school.rs`

- [ ] **Step 1: Write provider config tests**

Create `backend-admin/src/services/database_provider.rs` with the tests first:

```rust
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DatabaseProviderKind {
    Neon,
    SelfHostedPostgres,
}

impl DatabaseProviderKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Neon => "neon",
            Self::SelfHostedPostgres => "self_hosted_postgres",
        }
    }

    pub fn from_env_value(value: &str) -> Result<Self, String> {
        match value.trim() {
            "" | "neon" => Ok(Self::Neon),
            "self_hosted_postgres" => Ok(Self::SelfHostedPostgres),
            other => Err(format!(
                "DATABASE_PROVIDER must be 'neon' or 'self_hosted_postgres', got '{}'",
                other
            )),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProvisionedDatabase {
    pub provider: DatabaseProviderKind,
    pub database_name: String,
    pub connection_string: String,
    pub external_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfHostedPostgresConfig {
    pub admin_url: String,
    pub app_host: String,
    pub app_port: u16,
    pub tenant_user: String,
    pub tenant_password: String,
    pub sslmode: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatabaseProviderConfig {
    Neon,
    SelfHostedPostgres(SelfHostedPostgresConfig),
}

#[derive(Debug, Clone)]
pub struct DatabaseProvider {
    config: DatabaseProviderConfig,
}

impl DatabaseProvider {
    pub fn new(config: DatabaseProviderConfig) -> Self {
        Self { config }
    }

    pub fn kind(&self) -> DatabaseProviderKind {
        match self.config {
            DatabaseProviderConfig::Neon => DatabaseProviderKind::Neon,
            DatabaseProviderConfig::SelfHostedPostgres(_) => {
                DatabaseProviderKind::SelfHostedPostgres
            }
        }
    }
}

fn required_env(key: &str) -> Result<String, String> {
    env::var(key)
        .map(|value| value.trim().to_string())
        .ok()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{} must be set", key))
}

fn optional_env_or(key: &str, default: &str) -> String {
    env::var(key)
        .map(|value| value.trim().to_string())
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_string())
}

fn parse_port(value: &str) -> Result<u16, String> {
    value
        .parse::<u16>()
        .map_err(|_| format!("SELF_HOSTED_POSTGRES_APP_PORT must be a valid u16, got '{}'", value))
}

fn quote_pg_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn provider_kind_accepts_supported_values() {
        assert_eq!(
            DatabaseProviderKind::from_env_value("neon").unwrap(),
            DatabaseProviderKind::Neon
        );
        assert_eq!(
            DatabaseProviderKind::from_env_value("self_hosted_postgres").unwrap(),
            DatabaseProviderKind::SelfHostedPostgres
        );
        assert_eq!(
            DatabaseProviderKind::from_env_value("").unwrap(),
            DatabaseProviderKind::Neon
        );
    }

    #[test]
    fn provider_kind_rejects_unknown_values() {
        let error = DatabaseProviderKind::from_env_value("postgres").unwrap_err();

        assert!(error.contains("DATABASE_PROVIDER"));
        assert!(error.contains("self_hosted_postgres"));
    }

    #[test]
    fn parse_port_accepts_u16_values() {
        assert_eq!(parse_port("5432").unwrap(), 5432);
    }

    #[test]
    fn parse_port_rejects_invalid_values() {
        assert!(parse_port("not-a-port").unwrap_err().contains("u16"));
        assert!(parse_port("70000").unwrap_err().contains("u16"));
    }

    #[test]
    fn quote_pg_identifier_escapes_double_quotes() {
        assert_eq!(quote_pg_identifier("schoolorbit_sandbox"), "\"schoolorbit_sandbox\"");
        assert_eq!(quote_pg_identifier("school\"name"), "\"school\"\"name\"");
    }

    #[test]
    fn database_provider_reports_kind() {
        let provider = DatabaseProvider::new(DatabaseProviderConfig::Neon);

        assert_eq!(provider.kind(), DatabaseProviderKind::Neon);
    }
}
```

- [ ] **Step 2: Run provider tests and verify they compile-fail only because module is not exported**

Run:

```bash
cd backend-admin
cargo test services::database_provider::tests --lib
```

Expected: FAIL with a module resolution error if `database_provider` is not exported yet.

- [ ] **Step 3: Export the provider module**

Modify `backend-admin/src/services/mod.rs`:

```rust
pub mod auth_service;
pub mod database_provider;
pub mod school_service;

pub use auth_service::AuthService;
pub use database_provider::{
    DatabaseProvider, DatabaseProviderConfig, DatabaseProviderKind, ProvisionedDatabase,
    SelfHostedPostgresConfig,
};
pub use school_service::SchoolService;
```

- [ ] **Step 4: Extend `SchoolConfig` metadata**

Modify `backend-admin/src/models/school.rs`:

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SchoolConfig {
    pub db_id: Option<i64>,
    pub dns_record_id: Option<String>,
    pub deployment_url: Option<String>,
    #[serde(default)]
    pub database_provider: Option<String>,
    #[serde(default)]
    pub database_name: Option<String>,
    #[serde(default)]
    pub external_database_id: Option<String>,
}
```

Update `school_config_serializes_expected_keys()`:

```rust
#[test]
fn school_config_serializes_expected_keys() {
    let config = SchoolConfig {
        db_id: Some(42),
        dns_record_id: Some(String::new()),
        deployment_url: Some("https://sandbox.schoolorbit.app".to_string()),
        database_provider: Some("self_hosted_postgres".to_string()),
        database_name: Some("schoolorbit_sandbox".to_string()),
        external_database_id: None,
    };

    let value = serde_json::to_value(config).unwrap();

    assert_eq!(value["db_id"], 42);
    assert_eq!(value["dns_record_id"], "");
    assert_eq!(value["deployment_url"], "https://sandbox.schoolorbit.app");
    assert_eq!(value["database_provider"], "self_hosted_postgres");
    assert_eq!(value["database_name"], "schoolorbit_sandbox");
    assert_eq!(value["external_database_id"], serde_json::Value::Null);
}
```

Update `sqlx_json_school_config_serializes_as_plain_object()`:

```rust
#[test]
fn sqlx_json_school_config_serializes_as_plain_object() {
    let config = Json(SchoolConfig {
        db_id: Some(42),
        dns_record_id: Some("record_123".to_string()),
        deployment_url: Some("https://sandbox.schoolorbit.app".to_string()),
        database_provider: Some("neon".to_string()),
        database_name: Some("schoolorbit_sandbox".to_string()),
        external_database_id: Some("42".to_string()),
    });

    let value = serde_json::to_value(config).unwrap();

    assert_eq!(value["db_id"], 42);
    assert_eq!(value["dns_record_id"], "record_123");
    assert_eq!(value["deployment_url"], "https://sandbox.schoolorbit.app");
    assert_eq!(value["database_provider"], "neon");
    assert_eq!(value["database_name"], "schoolorbit_sandbox");
    assert_eq!(value["external_database_id"], "42");
}
```

- [ ] **Step 5: Run model and provider tests**

Run:

```bash
cd backend-admin
cargo test models::school::tests services::database_provider::tests --lib
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add backend-admin/src/services/database_provider.rs backend-admin/src/services/mod.rs backend-admin/src/models/school.rs
git commit -m "feat(admin): add database provider metadata"
```

---

### Task 2: Implement Neon and Self-Hosted Provider Operations

**Files:**
- Modify: `backend-admin/src/services/database_provider.rs`

- [ ] **Step 1: Add failing tests for self-host connection strings and provider env parsing**

Append these tests to `backend-admin/src/services/database_provider.rs`:

```rust
#[cfg(test)]
mod provider_config_tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn clear_provider_env() {
        for key in [
            "DATABASE_PROVIDER",
            "SELF_HOSTED_POSTGRES_ADMIN_URL",
            "SELF_HOSTED_POSTGRES_APP_HOST",
            "SELF_HOSTED_POSTGRES_APP_PORT",
            "SELF_HOSTED_POSTGRES_TENANT_USER",
            "SELF_HOSTED_POSTGRES_TENANT_PASSWORD",
            "SELF_HOSTED_POSTGRES_SSLMODE",
        ] {
            env::remove_var(key);
        }
    }

    #[test]
    fn from_env_defaults_to_neon() {
        let _guard = env_lock();
        clear_provider_env();

        let provider = DatabaseProvider::from_env().unwrap();

        assert_eq!(provider.kind(), DatabaseProviderKind::Neon);
    }

    #[test]
    fn from_env_loads_self_hosted_config() {
        let _guard = env_lock();
        clear_provider_env();
        env::set_var("DATABASE_PROVIDER", "self_hosted_postgres");
        env::set_var(
            "SELF_HOSTED_POSTGRES_ADMIN_URL",
            "postgresql://provisioner:secret@postgres:5432/postgres",
        );
        env::set_var("SELF_HOSTED_POSTGRES_APP_HOST", "postgres.internal");
        env::set_var("SELF_HOSTED_POSTGRES_APP_PORT", "5433");
        env::set_var("SELF_HOSTED_POSTGRES_TENANT_USER", "schoolorbit_tenant_owner");
        env::set_var("SELF_HOSTED_POSTGRES_TENANT_PASSWORD", "tenant-secret");
        env::set_var("SELF_HOSTED_POSTGRES_SSLMODE", "require");

        let provider = DatabaseProvider::from_env().unwrap();

        assert_eq!(provider.kind(), DatabaseProviderKind::SelfHostedPostgres);
        match provider.config {
            DatabaseProviderConfig::SelfHostedPostgres(config) => {
                assert_eq!(config.app_host, "postgres.internal");
                assert_eq!(config.app_port, 5433);
                assert_eq!(config.tenant_user, "schoolorbit_tenant_owner");
                assert_eq!(config.sslmode, "require");
            }
            DatabaseProviderConfig::Neon => panic!("expected self-hosted config"),
        }
    }

    #[test]
    fn self_hosted_connection_string_uses_configured_host_port_and_sslmode() {
        let config = SelfHostedPostgresConfig {
            admin_url: "postgresql://provisioner:secret@postgres:5432/postgres".to_string(),
            app_host: "postgres.internal".to_string(),
            app_port: 5433,
            tenant_user: "schoolorbit_tenant_owner".to_string(),
            tenant_password: "tenant-secret".to_string(),
            sslmode: "require".to_string(),
        };

        let url = config.connection_string_for_database("schoolorbit_sandbox");

        assert_eq!(
            url,
            "postgresql://schoolorbit_tenant_owner:tenant-secret@postgres.internal:5433/schoolorbit_sandbox?sslmode=require"
        );
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cd backend-admin
cargo test services::database_provider --lib
```

Expected: FAIL because `DatabaseProvider::from_env()` and `connection_string_for_database()` do not exist.

- [ ] **Step 3: Implement env parsing and provider operations**

Replace the implementation section in `backend-admin/src/services/database_provider.rs` with this code, keeping all tests:

```rust
impl SelfHostedPostgresConfig {
    pub fn connection_string_for_database(&self, database_name: &str) -> String {
        format!(
            "postgresql://{}:{}@{}:{}/{}?sslmode={}",
            self.tenant_user,
            self.tenant_password,
            self.app_host,
            self.app_port,
            database_name,
            self.sslmode
        )
    }
}

impl DatabaseProvider {
    pub fn from_env() -> Result<Self, String> {
        let provider = DatabaseProviderKind::from_env_value(
            &env::var("DATABASE_PROVIDER").unwrap_or_else(|_| "neon".to_string()),
        )?;

        match provider {
            DatabaseProviderKind::Neon => Ok(Self::new(DatabaseProviderConfig::Neon)),
            DatabaseProviderKind::SelfHostedPostgres => {
                let port = parse_port(&optional_env_or("SELF_HOSTED_POSTGRES_APP_PORT", "5432"))?;
                Ok(Self::new(DatabaseProviderConfig::SelfHostedPostgres(
                    SelfHostedPostgresConfig {
                        admin_url: required_env("SELF_HOSTED_POSTGRES_ADMIN_URL")?,
                        app_host: optional_env_or("SELF_HOSTED_POSTGRES_APP_HOST", "postgres"),
                        app_port: port,
                        tenant_user: required_env("SELF_HOSTED_POSTGRES_TENANT_USER")?,
                        tenant_password: required_env("SELF_HOSTED_POSTGRES_TENANT_PASSWORD")?,
                        sslmode: optional_env_or("SELF_HOSTED_POSTGRES_SSLMODE", "disable"),
                    },
                )))
            }
        }
    }

    pub async fn create_database(
        &self,
        database_name: &str,
    ) -> Result<ProvisionedDatabase, String> {
        match &self.config {
            DatabaseProviderConfig::Neon => self.create_neon_database(database_name).await,
            DatabaseProviderConfig::SelfHostedPostgres(config) => {
                self.create_self_hosted_database(config, database_name).await
            }
        }
    }

    pub async fn delete_database_by_name(&self, database_name: &str) -> Result<(), String> {
        match &self.config {
            DatabaseProviderConfig::Neon => {
                let neon_client = crate::clients::neon_client::NeonClient::new()?;
                neon_client.delete_database_by_name(database_name).await
            }
            DatabaseProviderConfig::SelfHostedPostgres(config) => {
                self.delete_self_hosted_database(config, database_name).await
            }
        }
    }

    async fn create_neon_database(
        &self,
        database_name: &str,
    ) -> Result<ProvisionedDatabase, String> {
        let neon_client = crate::clients::neon_client::NeonClient::new()?;
        let database_id = neon_client
            .create_database(database_name, "neondb_owner")
            .await?;
        let password = required_env("NEON_DB_PASSWORD")?;
        let connection_string =
            neon_client.get_connection_string(database_name, "neondb_owner", &password);

        neon_client.wait_for_database_ready(database_name).await?;
        neon_client
            .wait_for_database_connectable(&connection_string)
            .await?;

        Ok(ProvisionedDatabase {
            provider: DatabaseProviderKind::Neon,
            database_name: database_name.to_string(),
            connection_string,
            external_id: Some(database_id.to_string()),
        })
    }

    async fn create_self_hosted_database(
        &self,
        config: &SelfHostedPostgresConfig,
        database_name: &str,
    ) -> Result<ProvisionedDatabase, String> {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(30))
            .connect(&config.admin_url)
            .await
            .map_err(|error| format!("Failed to connect to self-hosted PostgreSQL admin URL: {}", error))?;

        let owner = quote_pg_identifier(&config.tenant_user);
        let database = quote_pg_identifier(database_name);
        let create_sql = format!("CREATE DATABASE {} OWNER {}", database, owner);

        let create_result = sqlx::query(&create_sql).execute(&pool).await;
        pool.close().await;

        match create_result {
            Ok(_) => {}
            Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("42P04") => {
                return Err(format!("Tenant database '{}' already exists", database_name));
            }
            Err(error) => {
                return Err(format!(
                    "Failed to create self-hosted tenant database '{}': {}",
                    database_name, error
                ));
            }
        }

        let connection_string = config.connection_string_for_database(database_name);
        self.wait_for_database_connectable(&connection_string).await?;

        Ok(ProvisionedDatabase {
            provider: DatabaseProviderKind::SelfHostedPostgres,
            database_name: database_name.to_string(),
            connection_string,
            external_id: None,
        })
    }

    async fn delete_self_hosted_database(
        &self,
        config: &SelfHostedPostgresConfig,
        database_name: &str,
    ) -> Result<(), String> {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(30))
            .connect(&config.admin_url)
            .await
            .map_err(|error| format!("Failed to connect to self-hosted PostgreSQL admin URL: {}", error))?;

        let terminate_result = sqlx::query(
            "SELECT pg_terminate_backend(pid)
             FROM pg_stat_activity
             WHERE datname = $1
               AND pid <> pg_backend_pid()",
        )
        .bind(database_name)
        .execute(&pool)
        .await;

        if let Err(error) = terminate_result {
            pool.close().await;
            return Err(format!(
                "Failed to terminate connections for '{}': {}",
                database_name, error
            ));
        }

        let drop_sql = format!("DROP DATABASE IF EXISTS {} WITH (FORCE)", quote_pg_identifier(database_name));
        let drop_result = sqlx::query(&drop_sql).execute(&pool).await;
        pool.close().await;

        drop_result
            .map(|_| ())
            .map_err(|error| format!("Failed to drop self-hosted database '{}': {}", database_name, error))
    }

    async fn wait_for_database_connectable(&self, database_url: &str) -> Result<(), String> {
        let max_attempts = 30;

        for attempt in 1..=max_attempts {
            match PgPoolOptions::new()
                .max_connections(1)
                .acquire_timeout(Duration::from_secs(5))
                .connect(database_url)
                .await
            {
                Ok(pool) => {
                    pool.close().await;
                    return Ok(());
                }
                Err(error) if attempt == max_attempts => {
                    return Err(format!(
                        "Timeout waiting for PostgreSQL database connection: {}",
                        error
                    ));
                }
                Err(_) => {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            }
        }

        Err("Timeout waiting for PostgreSQL database connection".to_string())
    }
}
```

- [ ] **Step 4: Run provider tests**

Run:

```bash
cd backend-admin
cargo test services::database_provider --lib
```

Expected: PASS.

- [ ] **Step 5: Run cargo check**

Run:

```bash
cd backend-admin
cargo check
```

Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add backend-admin/src/services/database_provider.rs
git commit -m "feat(admin): implement database providers"
```

---

### Task 3: Wire Provider into School Provisioning

**Files:**
- Modify: `backend-admin/src/services/school_service.rs`

- [ ] **Step 1: Update tests for provider-aware config**

Modify the test imports in `backend-admin/src/services/school_service.rs`:

```rust
use super::{
    build_active_school_config, build_provisioning_failure_message, build_school_database_name,
    validate_school_subdomain, ProvisioningRunOptions,
};
use crate::services::{DatabaseProviderKind, ProvisionedDatabase};
```

Replace `active_school_config_records_database_and_deployment_url()`:

```rust
#[test]
fn active_school_config_records_database_provider_metadata() {
    let database = ProvisionedDatabase {
        provider: DatabaseProviderKind::SelfHostedPostgres,
        database_name: "schoolorbit_sandbox".to_string(),
        connection_string: "postgresql://redacted".to_string(),
        external_id: None,
    };

    let config = build_active_school_config(&database, "", "https://sandbox.schoolorbit.app");

    assert_eq!(config.db_id, None);
    assert_eq!(config.dns_record_id.as_deref(), Some(""));
    assert_eq!(
        config.deployment_url.as_deref(),
        Some("https://sandbox.schoolorbit.app")
    );
    assert_eq!(
        config.database_provider.as_deref(),
        Some("self_hosted_postgres")
    );
    assert_eq!(config.database_name.as_deref(), Some("schoolorbit_sandbox"));
    assert_eq!(config.external_database_id, None);
}

#[test]
fn active_school_config_keeps_neon_numeric_db_id_for_legacy_ui() {
    let database = ProvisionedDatabase {
        provider: DatabaseProviderKind::Neon,
        database_name: "schoolorbit_sandbox".to_string(),
        connection_string: "postgresql://redacted".to_string(),
        external_id: Some("42".to_string()),
    };

    let config = build_active_school_config(&database, "", "https://sandbox.schoolorbit.app");

    assert_eq!(config.db_id, Some(42));
    assert_eq!(config.external_database_id.as_deref(), Some("42"));
    assert_eq!(config.database_provider.as_deref(), Some("neon"));
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cd backend-admin
cargo test services::school_service::tests --lib
```

Expected: FAIL because `build_active_school_config()` still takes `i64`.

- [ ] **Step 3: Update imports and config builder**

Modify the top imports in `backend-admin/src/services/school_service.rs`:

```rust
use crate::error::AppError;
use crate::models::{CreateSchool, School, SchoolConfig, UpdateSchool};
use crate::services::{DatabaseProvider, ProvisionedDatabase};
use sqlx::{types::Json, PgPool};
use tracing::{error, info, warn};
use uuid::Uuid;
```

Replace `build_active_school_config()`:

```rust
fn build_active_school_config(
    database: &ProvisionedDatabase,
    dns_record_id: &str,
    deployment_url: &str,
) -> SchoolConfig {
    SchoolConfig {
        db_id: database
            .external_id
            .as_ref()
            .and_then(|value| value.parse::<i64>().ok()),
        dns_record_id: Some(dns_record_id.to_string()),
        deployment_url: Some(deployment_url.to_string()),
        database_provider: Some(database.provider.as_str().to_string()),
        database_name: Some(database.database_name.clone()),
        external_database_id: database.external_id.clone(),
    }
}
```

- [ ] **Step 4: Replace Neon creation in `provision_school()`**

In `backend-admin/src/services/school_service.rs`, replace the block from `// Step 1: Create database in Neon` through the connectable wait with:

```rust
        options
            .progress(2, TOTAL_STEPS, "Creating tenant database...")
            .await;

        let database_provider = DatabaseProvider::from_env()
            .map_err(|e| AppError::ExternalServiceError(format!("Database provider error: {}", e)))?;

        let provisioned_database = match database_provider.create_database(&db_name).await {
            Ok(database) => database,
            Err(e) => {
                return Err(AppError::ExternalServiceError(format!(
                    "Failed to create tenant database: {}",
                    e
                )));
            }
        };

        options
            .success(&format!(
                "✅ Tenant database created using {}",
                provisioned_database.provider.as_str()
            ))
            .await;
```

Replace `.bind(&db_connection_string)` in the school insert with:

```rust
        .bind(&provisioned_database.connection_string)
```

Replace calls to `rollback_failed_provisioning(..., &neon_client, &db_name, ...)` in the provisioning flow with:

```rust
self.rollback_failed_provisioning(Some(school_id), &database_provider, &db_name, reason).await
```

For the school record insert error before `school_id` exists, call:

```rust
self.rollback_failed_provisioning(None, &database_provider, &db_name, reason).await
```

Replace the call to `provision_tenant_database()` so it passes the provisioned connection string:

```rust
            .provision_tenant_database(
                &backend_school_client,
                school_id,
                &provisioned_database.connection_string,
                &data,
            )
```

Replace final config creation:

```rust
        let config = build_active_school_config(&provisioned_database, &dns_record_id, &subdomain_url);
```

- [ ] **Step 5: Update rollback helper signature**

Replace `rollback_failed_provisioning()` signature and delete logic:

```rust
    async fn rollback_failed_provisioning(
        &self,
        school_id: Option<Uuid>,
        database_provider: &DatabaseProvider,
        db_name: &str,
        reason: String,
    ) -> AppError {
        let mut cleanup_errors = Vec::new();

        if let Some(school_id) = school_id {
            if let Err(e) = self
                .mark_school_failed(school_id, "provision_failed", &reason)
                .await
            {
                cleanup_errors.push(format!(
                    "failed to mark school {} as provision_failed: {}",
                    school_id, e
                ));
            }
        }

        let database_deleted = match database_provider.delete_database_by_name(db_name).await {
            Ok(_) => true,
            Err(e) => {
                cleanup_errors.push(format!(
                    "failed to delete tenant database '{}': {}",
                    db_name, e
                ));
                false
            }
        };

        if database_deleted {
            if let Some(school_id) = school_id {
                if let Err(e) = self.delete_school_record(school_id).await {
                    cleanup_errors.push(format!(
                        "failed to delete school record {}: {}",
                        school_id, e
                    ));
                }
            }
        }

        AppError::ExternalServiceError(build_provisioning_failure_message(&reason, &cleanup_errors))
    }
```

- [ ] **Step 6: Run school service tests**

Run:

```bash
cd backend-admin
cargo test services::school_service::tests --lib
```

Expected: PASS.

- [ ] **Step 7: Run cargo check**

Run:

```bash
cd backend-admin
cargo check
```

Expected: PASS.

- [ ] **Step 8: Commit**

```bash
git add backend-admin/src/services/school_service.rs
git commit -m "feat(admin): use provider for school provisioning"
```

---

### Task 4: Wire Provider into School Deletion

**Files:**
- Modify: `backend-admin/src/services/school_service.rs`

- [ ] **Step 1: Add database name helper**

Add this helper near `build_school_database_name()`:

```rust
fn database_name_for_school(school: &School) -> String {
    school
        .config
        .database_name
        .clone()
        .unwrap_or_else(|| build_school_database_name(&school.subdomain))
}
```

Add this test:

```rust
#[test]
fn database_name_for_school_uses_config_when_present() {
    let school = School {
        id: Uuid::nil(),
        name: "Sandbox".to_string(),
        subdomain: "sandbox".to_string(),
        db_name: "legacy_name".to_string(),
        db_connection_string: None,
        status: "active".to_string(),
        config: Json(SchoolConfig {
            database_name: Some("schoolorbit_from_config".to_string()),
            ..SchoolConfig::default()
        }),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    assert_eq!(database_name_for_school(&school), "schoolorbit_from_config");
}
```

Update test imports:

```rust
use crate::models::{School, SchoolConfig};
use sqlx::types::Json;
use uuid::Uuid;
```

- [ ] **Step 2: Run test to verify helper compiles**

Run:

```bash
cd backend-admin
cargo test services::school_service::tests::database_name_for_school_uses_config_when_present --lib
```

Expected: PASS.

- [ ] **Step 3: Replace direct Neon deletion in `delete_school()`**

In `delete_school()`, remove:

```rust
        let db_id = school.config.db_id;
```

Replace the `// Step 3: Delete Neon database` block with:

```rust
        info!("deleting tenant database");

        let db_name = database_name_for_school(&school);
        let database_provider = DatabaseProvider::from_env()
            .map_err(|e| AppError::ExternalServiceError(format!("Database provider error: {}", e)))?;

        match database_provider.delete_database_by_name(&db_name).await {
            Ok(_) => info!(db_name, "tenant database deleted"),
            Err(e) => {
                warn!(error = %e, db_name, "failed to delete tenant database; manual cleanup may be needed");
            }
        }
```

- [ ] **Step 4: Replace direct Neon deletion in `delete_school_stream()`**

In `delete_school_stream()`, replace the `// Step 3: Delete Neon database` block with:

```rust
        logger.progress(3, 4, "Deleting tenant database...").await;

        let db_name = database_name_for_school(&school);
        logger.info(&format!("Database name: {}", db_name)).await;

        match DatabaseProvider::from_env() {
            Ok(database_provider) => match database_provider.delete_database_by_name(&db_name).await {
                Ok(_) => {
                    logger
                        .success(&format!("✅ Database deleted: {}", db_name))
                        .await
                }
                Err(e) => {
                    logger
                        .warning(&format!("⚠️  Database deletion failed: {}", e))
                        .await
                }
            },
            Err(e) => {
                logger
                    .warning(&format!("⚠️  Database provider unavailable: {}", e))
                    .await
            }
        }
```

- [ ] **Step 5: Remove stale Neon wording in deletion logs**

Search:

```bash
rg -n "Delete Neon|Neon database|deleting Neon|neon_client" backend-admin/src/services/school_service.rs
```

Expected after edits: no results in `school_service.rs`.

- [ ] **Step 6: Run tests and cargo check**

Run:

```bash
cd backend-admin
cargo test services::school_service::tests --lib
cargo check
```

Expected: both PASS.

- [ ] **Step 7: Commit**

```bash
git add backend-admin/src/services/school_service.rs
git commit -m "feat(admin): use provider for school deletion"
```

---

### Task 5: Update Startup Logs and Environment Examples

**Files:**
- Modify: `backend-admin/src/main.rs`
- Modify: `.env.example`
- Modify: `backend-admin/.env.example`
- Modify: `backend-school/.env.portainer.example`

- [ ] **Step 1: Replace Neon-specific admin startup log**

In `backend-admin/src/main.rs`, replace:

```rust
    info!("connected to Neon PostgreSQL");
```

with:

```rust
    info!("connected to admin PostgreSQL database");
```

- [ ] **Step 2: Update root env example**

In `.env.example`, replace the Neon database block with:

```dotenv
# Database provider for backend-admin tenant provisioning.
# Use self_hosted_postgres for the PostgreSQL 18 self-host migration.
DATABASE_PROVIDER=self_hosted_postgres

# Admin/platform database used by backend-admin.
DATABASE_URL=postgresql://schoolorbit_admin_app:password@postgres:5432/schoolorbit_admin?sslmode=disable

# Self-hosted PostgreSQL tenant provisioning.
SELF_HOSTED_POSTGRES_ADMIN_URL=postgresql://schoolorbit_provisioner:password@postgres:5432/postgres?sslmode=disable
SELF_HOSTED_POSTGRES_APP_HOST=postgres
SELF_HOSTED_POSTGRES_APP_PORT=5432
SELF_HOSTED_POSTGRES_TENANT_USER=schoolorbit_tenant_owner
SELF_HOSTED_POSTGRES_TENANT_PASSWORD=change-this-tenant-password
SELF_HOSTED_POSTGRES_SSLMODE=disable

# Neon fallback. Required only when DATABASE_PROVIDER=neon.
NEON_API_KEY=
NEON_PROJECT_ID=
NEON_BRANCH_ID=main
NEON_HOST=
NEON_DB_PASSWORD=
```

- [ ] **Step 3: Update `backend-admin/.env.example`**

Ensure `backend-admin/.env.example` contains this block:

```dotenv
DATABASE_PROVIDER=self_hosted_postgres
DATABASE_URL=postgresql://schoolorbit_admin_app:password@postgres:5432/schoolorbit_admin?sslmode=disable
SELF_HOSTED_POSTGRES_ADMIN_URL=postgresql://schoolorbit_provisioner:password@postgres:5432/postgres?sslmode=disable
SELF_HOSTED_POSTGRES_APP_HOST=postgres
SELF_HOSTED_POSTGRES_APP_PORT=5432
SELF_HOSTED_POSTGRES_TENANT_USER=schoolorbit_tenant_owner
SELF_HOSTED_POSTGRES_TENANT_PASSWORD=change-this-tenant-password
SELF_HOSTED_POSTGRES_SSLMODE=disable
NEON_API_KEY=
NEON_PROJECT_ID=
NEON_BRANCH_ID=main
NEON_HOST=
NEON_DB_PASSWORD=
```

- [ ] **Step 4: Update Portainer env example**

In `backend-school/.env.portainer.example`, add this note near database variables:

```dotenv
# Tenant database URLs are resolved through backend-admin.
# After the self-host migration, backend-school still does not need DATABASE_URL.
# backend-admin stores PostgreSQL 18 self-host tenant URLs in schools.db_connection_string.
```

- [ ] **Step 5: Verify no required Neon wording remains in env examples**

Run:

```bash
rg -n "required.*Neon|Neon \\(tenant database provisioning\\)|Admin database \\(Neon\\)" .env.example backend-admin/.env.example backend-school/.env.portainer.example podman-compose.yml docker-compose.yml
```

Expected: no results. Mentions that explicitly say Neon fallback is optional are acceptable.

- [ ] **Step 6: Run cargo check**

Run:

```bash
cd backend-admin
cargo check
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add backend-admin/src/main.rs .env.example backend-admin/.env.example backend-school/.env.portainer.example
git commit -m "chore: document self-host database provider env"
```

---

### Task 6: Add PostgreSQL 18 and Databasus to Compose

**Files:**
- Modify: `podman-compose.yml`
- Modify: `docker-compose.yml`

- [ ] **Step 1: Update production Podman compose**

In `podman-compose.yml`, add this service before `backend-admin`:

```yaml
  postgres:
    image: docker.io/library/postgres:18-alpine
    container_name: schoolorbit-postgres
    restart: unless-stopped
    environment:
      POSTGRES_USER: ${POSTGRES_USER:-schoolorbit_provisioner}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
      POSTGRES_DB: postgres
    volumes:
      - postgres_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U ${POSTGRES_USER:-schoolorbit_provisioner} -d postgres"]
      interval: 10s
      timeout: 5s
      retries: 5
      start_period: 30s
    networks:
      - schoolorbit-net

  databasus:
    image: docker.io/databasus/databasus:latest
    container_name: schoolorbit-databasus
    restart: unless-stopped
    ports:
      - "4005:4005"
    volumes:
      - databasus_data:/databasus-data
    depends_on:
      postgres:
        condition: service_healthy
    networks:
      - schoolorbit-net
```

In `backend-admin.environment`, replace Neon-required env with:

```yaml
      # Database provider
      DATABASE_PROVIDER: ${DATABASE_PROVIDER:-self_hosted_postgres}
      DATABASE_URL: ${DATABASE_URL}
      SELF_HOSTED_POSTGRES_ADMIN_URL: ${SELF_HOSTED_POSTGRES_ADMIN_URL}
      SELF_HOSTED_POSTGRES_APP_HOST: ${SELF_HOSTED_POSTGRES_APP_HOST:-postgres}
      SELF_HOSTED_POSTGRES_APP_PORT: ${SELF_HOSTED_POSTGRES_APP_PORT:-5432}
      SELF_HOSTED_POSTGRES_TENANT_USER: ${SELF_HOSTED_POSTGRES_TENANT_USER}
      SELF_HOSTED_POSTGRES_TENANT_PASSWORD: ${SELF_HOSTED_POSTGRES_TENANT_PASSWORD}
      SELF_HOSTED_POSTGRES_SSLMODE: ${SELF_HOSTED_POSTGRES_SSLMODE:-disable}

      # Neon fallback, only used when DATABASE_PROVIDER=neon
      NEON_API_KEY: ${NEON_API_KEY:-}
      NEON_PROJECT_ID: ${NEON_PROJECT_ID:-}
      NEON_BRANCH_ID: ${NEON_BRANCH_ID:-main}
      NEON_HOST: ${NEON_HOST:-}
      NEON_DB_PASSWORD: ${NEON_DB_PASSWORD:-}
```

Add `depends_on` for `backend-admin`:

```yaml
    depends_on:
      postgres:
        condition: service_healthy
```

Add volumes at the bottom:

```yaml
volumes:
  postgres_data:
    driver: local
  databasus_data:
    driver: local
```

- [ ] **Step 2: Update local Docker compose**

In `docker-compose.yml`, change the Postgres image:

```yaml
    image: postgres:18-alpine
```

In `backend-admin.environment`, set:

```yaml
      - DATABASE_PROVIDER=${DATABASE_PROVIDER:-self_hosted_postgres}
      - DATABASE_URL=${DATABASE_URL:-postgresql://admin_user:password@postgres:5432/schoolorbit_admin?sslmode=disable}
      - SELF_HOSTED_POSTGRES_ADMIN_URL=${SELF_HOSTED_POSTGRES_ADMIN_URL:-postgresql://admin_user:password@postgres:5432/postgres?sslmode=disable}
      - SELF_HOSTED_POSTGRES_APP_HOST=${SELF_HOSTED_POSTGRES_APP_HOST:-postgres}
      - SELF_HOSTED_POSTGRES_APP_PORT=${SELF_HOSTED_POSTGRES_APP_PORT:-5432}
      - SELF_HOSTED_POSTGRES_TENANT_USER=${SELF_HOSTED_POSTGRES_TENANT_USER:-admin_user}
      - SELF_HOSTED_POSTGRES_TENANT_PASSWORD=${SELF_HOSTED_POSTGRES_TENANT_PASSWORD:-password}
      - SELF_HOSTED_POSTGRES_SSLMODE=${SELF_HOSTED_POSTGRES_SSLMODE:-disable}
```

Add local Databasus service:

```yaml
  databasus:
    image: databasus/databasus:latest
    container_name: schoolorbit-databasus
    ports:
      - "4005:4005"
    volumes:
      - databasus_data:/databasus-data
    depends_on:
      postgres:
        condition: service_healthy
    networks:
      - schoolorbit-network
    restart: unless-stopped
```

Add local volume:

```yaml
  databasus_data:
    driver: local
```

- [ ] **Step 3: Validate compose syntax**

Run:

```bash
docker compose -f docker-compose.yml config >/tmp/schoolorbit-compose.out
podman-compose -f podman-compose.yml config >/tmp/schoolorbit-podman-compose.out
```

Expected: both commands exit 0. If `podman-compose` is unavailable locally, run `docker compose -f podman-compose.yml config` and record that Podman validation must run on the server.

- [ ] **Step 4: Commit**

```bash
git add podman-compose.yml docker-compose.yml
git commit -m "chore: add self-host postgres and databasus compose"
```

---

### Task 7: Add Migration Runbook

**Files:**
- Create: `docs/POSTGRES_SELF_HOST_MIGRATION.md`

- [ ] **Step 1: Write the runbook**

Create `docs/POSTGRES_SELF_HOST_MIGRATION.md`:

```markdown
# PostgreSQL 18 Self-Host Migration Runbook

This runbook moves SchoolOrbit from Neon to PostgreSQL 18 self-hosted using clean tenant migration.

## 1. Required Environment

Set these values in the production `.env` used by `podman-compose.yml`:

```dotenv
POSTGRES_USER=schoolorbit_provisioner
POSTGRES_PASSWORD=<strong provisioner password>
DATABASE_PROVIDER=self_hosted_postgres
DATABASE_URL=postgresql://schoolorbit_admin_app:<admin app password>@postgres:5432/schoolorbit_admin?sslmode=disable
SELF_HOSTED_POSTGRES_ADMIN_URL=postgresql://schoolorbit_provisioner:<provisioner password>@postgres:5432/postgres?sslmode=disable
SELF_HOSTED_POSTGRES_APP_HOST=postgres
SELF_HOSTED_POSTGRES_APP_PORT=5432
SELF_HOSTED_POSTGRES_TENANT_USER=schoolorbit_tenant_owner
SELF_HOSTED_POSTGRES_TENANT_PASSWORD=<tenant owner password>
SELF_HOSTED_POSTGRES_SSLMODE=disable
```

## 2. Create Roles and Admin Database

Connect as the provisioner:

```bash
podman exec -it schoolorbit-postgres psql -U "$POSTGRES_USER" -d postgres
```

Run:

```sql
CREATE ROLE schoolorbit_admin_app LOGIN PASSWORD '<admin app password>';
CREATE ROLE schoolorbit_tenant_owner LOGIN PASSWORD '<tenant owner password>';
CREATE DATABASE schoolorbit_admin OWNER schoolorbit_admin_app;
GRANT CREATE ON DATABASE postgres TO schoolorbit_provisioner;
```

If the roles already exist, verify ownership instead:

```sql
\du
\l
```

## 3. Migrate backend-admin Data

Export from Neon:

```bash
pg_dump "$NEON_ADMIN_DATABASE_URL" \
  --format=custom \
  --no-owner \
  --no-privileges \
  --file=/tmp/schoolorbit_admin.dump
```

Restore into self-hosted PostgreSQL:

```bash
pg_restore \
  --dbname "$DATABASE_URL" \
  --no-owner \
  --no-privileges \
  --clean \
  --if-exists \
  /tmp/schoolorbit_admin.dump
```

Start `backend-admin` and confirm its migrations run.

## 4. Provision Sandbox Tenant

Use the admin UI or API to create a sandbox school. Then verify the tenant database:

```bash
MIGRATION_AUDIT_DATABASE_URL='postgresql://schoolorbit_tenant_owner:<tenant owner password>@postgres:5432/schoolorbit_sandbox?sslmode=disable' \
  ./scripts/check_migration_rebaseline_ready.sh
```

Run smoke tests with sandbox credentials:

```bash
SMOKE_TENANT_URL=https://sandbox.schoolorbit.app \
SMOKE_API_URL=https://school-api.schoolorbit.app \
SMOKE_USERNAME="$SMOKE_USERNAME" \
SMOKE_PASSWORD="$SMOKE_PASSWORD" \
  ./scripts/smoke_test.sh
```

## 5. Configure Databasus

Open Databasus at `http://<server-ip>:4005` or behind the configured reverse proxy.

Create backup jobs for:

- `schoolorbit_admin`
- each active tenant database

Use read-only PostgreSQL credentials for logical backups. Store backups in R2 or S3-compatible storage. Enable notifications for failed backups.

After the first backup, restore it into a disposable database and verify a simple query:

```sql
SELECT COUNT(*) FROM schools;
```

For tenant restores, verify:

```sql
SELECT COUNT(*) FROM _sqlx_migrations WHERE success;
SELECT COUNT(*) FROM permissions;
```

## 6. Copy Important Neon Tenants

Prepare a clean target tenant database first:

```bash
PREPARE_CLEAN_TENANT_DATABASE_URL='postgresql://schoolorbit_tenant_owner:<tenant owner password>@postgres:5432/schoolorbit_target?sslmode=disable' \
PREPARE_CLEAN_TENANT_CONFIRM=public \
PREPARE_CLEAN_TENANT_ALLOW_NON_TEST=1 \
  ./scripts/prepare_clean_tenant_db.sh
```

Dry-run data copy:

```bash
CUTOVER_SOURCE_DATABASE_URL="$NEON_TENANT_DATABASE_URL" \
CUTOVER_TARGET_DATABASE_URL='postgresql://schoolorbit_tenant_owner:<tenant owner password>@postgres:5432/schoolorbit_target?sslmode=disable' \
  ./scripts/cutover_tenant_data.sh
```

Apply data copy:

```bash
CUTOVER_MODE=apply \
CUTOVER_SOURCE_DATABASE_URL="$NEON_TENANT_DATABASE_URL" \
CUTOVER_TARGET_DATABASE_URL='postgresql://schoolorbit_tenant_owner:<tenant owner password>@postgres:5432/schoolorbit_target?sslmode=disable' \
CUTOVER_TARGET_SCHEMA=public \
CUTOVER_ALLOW_PUBLIC_TARGET=1 \
CUTOVER_ALLOW_NON_TEST_TARGET=1 \
CUTOVER_CONFIRM_TARGET_TRUNCATE=public \
  ./scripts/cutover_tenant_data.sh
```

Update `backend-admin.schools.db_connection_string` for that tenant only after dry-run and apply checks pass.

## 7. Rollback

While Neon is retained:

1. set `DATABASE_PROVIDER=neon`;
2. restore the previous `schools.db_connection_string` value for the affected tenant;
3. redeploy `backend-admin` and `backend-school`;
4. run smoke tests against the tenant.

Keep Neon databases untouched for 7-14 days after successful self-host production validation.
```

- [ ] **Step 2: Check markdown for incomplete markers**

Run:

```bash
rg -n "FIX""ME|UNRESOLVED|REPLACE_ME" docs/POSTGRES_SELF_HOST_MIGRATION.md
```

Expected: no output. The string `<strong provisioner password>` is an operator secret marker and is acceptable in this runbook.

- [ ] **Step 3: Commit**

```bash
git add docs/POSTGRES_SELF_HOST_MIGRATION.md
git commit -m "docs: add postgres self-host migration runbook"
```

---

### Task 8: Full Backend Verification

**Files:**
- No code changes expected.

- [ ] **Step 1: Run backend-admin tests**

Run:

```bash
cd backend-admin
cargo test
```

Expected: PASS.

- [ ] **Step 2: Run backend-admin check**

Run:

```bash
cd backend-admin
cargo check
```

Expected: PASS.

- [ ] **Step 3: Run backend-school static migration tests**

Run:

```bash
cd backend-school
cargo test db::migration::tests --bin backend-school
```

Expected: PASS.

- [ ] **Step 4: Run repository diff checks**

Run:

```bash
git diff --check
git status --short
```

Expected: `git diff --check` exits 0. `git status --short` should show no uncommitted implementation changes.

- [ ] **Step 5: Commit only if verification changed docs or scripts**

If no files changed, do not create an empty commit. If command notes were added to docs during verification, run:

```bash
git add docs/POSTGRES_SELF_HOST_MIGRATION.md
git commit -m "docs: record postgres self-host verification"
```

---

### Task 9: Sandbox Deployment Verification

**Files:**
- No repository changes expected unless environment docs need correction.

- [ ] **Step 1: Start self-host stack**

Run on the deployment host:

```bash
cd /opt/stack
podman-compose -f podman-compose.yml up -d postgres databasus backend-admin backend-school
```

Expected:

```text
schoolorbit-postgres      running/healthy
schoolorbit-databasus     running
schoolorbit-backend-admin running/healthy
schoolorbit-backend-school running/healthy
```

- [ ] **Step 2: Verify admin health**

Run:

```bash
curl -fsS http://localhost:8080/health
```

Expected: HTTP 200.

- [ ] **Step 3: Verify school backend health**

Run:

```bash
curl -fsS http://localhost:8081/health
```

Expected: HTTP 200.

- [ ] **Step 4: Provision sandbox tenant**

Create a sandbox school from the admin UI or the existing school creation API. Use subdomain `sandbox` if it is not already active on the self-hosted admin database.

Expected:

- backend-admin logs include `Tenant database created using self_hosted_postgres`;
- `schools.db_connection_string` points to the self-hosted PostgreSQL host;
- backend-school provisioning completes.

- [ ] **Step 5: Verify tenant migration baseline**

Run:

```bash
MIGRATION_AUDIT_DATABASE_URL='postgresql://schoolorbit_tenant_owner:<tenant owner password>@postgres:5432/schoolorbit_sandbox?sslmode=disable' \
  ./scripts/check_migration_rebaseline_ready.sh
```

Expected: PASS with one successful SQLx migration at version `1`.

- [ ] **Step 6: Run smoke test**

Run:

```bash
SMOKE_TENANT_URL=https://sandbox.schoolorbit.app \
SMOKE_API_URL=https://school-api.schoolorbit.app \
SMOKE_USERNAME="$SMOKE_USERNAME" \
SMOKE_PASSWORD="$SMOKE_PASSWORD" \
  ./scripts/smoke_test.sh
```

Expected: PASS for public endpoints, CORS, login, and `/api/auth/me`.

- [ ] **Step 7: Verify Databasus backup and restore**

In Databasus:

1. create a logical backup job for `schoolorbit_sandbox`;
2. run the backup immediately;
3. restore into `schoolorbit_sandbox_restore_check`;
4. connect with `psql`;
5. run:

```sql
SELECT COUNT(*) FROM _sqlx_migrations WHERE success;
SELECT COUNT(*) FROM permissions;
```

Expected: first query returns `1`; second query returns the current permission count for the clean baseline.

- [ ] **Step 8: Record deployment result**

If sandbox succeeds, add a short dated note to `docs/POSTGRES_SELF_HOST_MIGRATION.md` under a new `## Sandbox Verification Log` heading:

```markdown
## Sandbox Verification Log

- 2026-06-23: Sandbox tenant provisioned on PostgreSQL 18 self-hosted. Smoke test passed. Databasus backup and restore check passed.
```

Commit:

```bash
git add docs/POSTGRES_SELF_HOST_MIGRATION.md
git commit -m "docs: record self-host sandbox verification"
```
