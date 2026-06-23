# PostgreSQL Self-Host Migration Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace Neon-only provisioning with PostgreSQL 18 self-hosted provisioning and native backup/restore scripts.

**Architecture:** `backend-admin` will use one `SelfHostedPostgresProvisioner` to create, verify, and drop tenant databases through a PostgreSQL admin connection. `backend-school` continues to consume tenant connection strings and run clean-baseline tenant migrations. Backups use `pg_dump --format=custom`, object storage upload, retention cleanup, and restore checks into disposable databases.

**Tech Stack:** Rust, Axum, SQLx PostgreSQL, PostgreSQL 18, Bash, `pg_dump`, `pg_restore`, S3-compatible object storage, existing tenant migration scripts.

---

## File Structure

- Create `backend-admin/src/services/self_hosted_postgres.rs`: self-host config, tenant connection string builder, create/drop database operations, connectability check, and focused unit tests.
- Modify `backend-admin/src/services/mod.rs`: export the self-host provisioner.
- Modify `backend-admin/src/services/school_service.rs`: remove direct Neon usage, use `SelfHostedPostgresProvisioner` in create/delete flows, update rollback cleanup and tests.
- Modify `backend-admin/src/main.rs`: remove Neon-specific startup wording.
- Delete `backend-admin/src/clients/neon_client.rs`: no runtime Neon path remains.
- Modify `backend-admin/src/clients/mod.rs`: remove `neon_client` export.
- Modify `podman-compose.yml` and `docker-compose.yml`: add PostgreSQL 18 service and self-host env vars; do not add Databasus.
- Modify `.env.example`, `backend-admin/.env.example`, and `backend-school/.env.portainer.example`: document self-host vars and remove required `NEON_*`.
- Create `scripts/backup_postgres.sh`: dump admin DB and tenant DBs, write local artifacts, optionally upload with `rclone`.
- Create `scripts/restore_check_postgres.sh`: restore one admin or tenant dump into a disposable DB and run sanity queries.
- Create `docs/POSTGRES_SELF_HOST_MIGRATION.md`: operator runbook for roles, admin DB migration, backup/restore, sandbox provisioning, tenant cutover, and Neon shutdown.

## Scope Notes

- Do not edit old SQLx migration files.
- Do not add Databasus in this migration.
- Do not keep a Neon code fallback.
- Do not add logical replication.
- Do not change tenant isolation. Keep one database per school.
- Do not expose database URLs, passwords, national IDs, or raw dump contents in logs.
- Do not move Cloudflare R2 file storage.

---

### Task 1: Add Self-Hosted PostgreSQL Provisioner

**Files:**
- Create: `backend-admin/src/services/self_hosted_postgres.rs`
- Modify: `backend-admin/src/services/mod.rs`

- [ ] **Step 1: Write self-host provisioner tests**

Create `backend-admin/src/services/self_hosted_postgres.rs`:

```rust
use sqlx::postgres::PgPoolOptions;
use std::env;
use std::time::Duration;

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
pub struct ProvisionedDatabase {
    pub database_name: String,
    pub connection_string: String,
}

#[derive(Debug, Clone)]
pub struct SelfHostedPostgresProvisioner {
    config: SelfHostedPostgresConfig,
}

impl SelfHostedPostgresProvisioner {
    pub fn new(config: SelfHostedPostgresConfig) -> Self {
        Self { config }
    }

    pub fn config(&self) -> &SelfHostedPostgresConfig {
        &self.config
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
    value.parse::<u16>().map_err(|_| {
        format!(
            "SELF_HOSTED_POSTGRES_APP_PORT must be a valid u16, got '{}'",
            value
        )
    })
}

fn quote_pg_identifier(identifier: &str) -> String {
    format!("\"{}\"", identifier.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn clear_self_host_env() {
        for key in [
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
        assert_eq!(
            quote_pg_identifier("schoolorbit_sandbox"),
            "\"schoolorbit_sandbox\""
        );
        assert_eq!(quote_pg_identifier("school\"name"), "\"school\"\"name\"");
    }

    #[test]
    fn connection_string_uses_configured_host_port_and_sslmode() {
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

    #[test]
    fn from_env_loads_self_hosted_config() {
        let _guard = env_lock();
        clear_self_host_env();
        env::set_var(
            "SELF_HOSTED_POSTGRES_ADMIN_URL",
            "postgresql://provisioner:secret@postgres:5432/postgres",
        );
        env::set_var("SELF_HOSTED_POSTGRES_APP_HOST", "postgres.internal");
        env::set_var("SELF_HOSTED_POSTGRES_APP_PORT", "5433");
        env::set_var("SELF_HOSTED_POSTGRES_TENANT_USER", "schoolorbit_tenant_owner");
        env::set_var("SELF_HOSTED_POSTGRES_TENANT_PASSWORD", "tenant-secret");
        env::set_var("SELF_HOSTED_POSTGRES_SSLMODE", "require");

        let provisioner = SelfHostedPostgresProvisioner::from_env().unwrap();

        assert_eq!(provisioner.config().app_host, "postgres.internal");
        assert_eq!(provisioner.config().app_port, 5433);
        assert_eq!(provisioner.config().tenant_user, "schoolorbit_tenant_owner");
        assert_eq!(provisioner.config().sslmode, "require");
    }
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cd backend-admin
cargo test services::self_hosted_postgres::tests --lib
```

Expected: FAIL because `self_hosted_postgres` is not exported and methods are missing.

- [ ] **Step 3: Export the module**

Modify `backend-admin/src/services/mod.rs`:

```rust
pub mod auth_service;
pub mod school_service;
pub mod self_hosted_postgres;

pub use auth_service::AuthService;
pub use school_service::SchoolService;
pub use self_hosted_postgres::{
    ProvisionedDatabase, SelfHostedPostgresConfig, SelfHostedPostgresProvisioner,
};
```

- [ ] **Step 4: Implement config methods and database operations**

Append below `quote_pg_identifier()` in `backend-admin/src/services/self_hosted_postgres.rs`:

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

impl SelfHostedPostgresProvisioner {
    pub fn from_env() -> Result<Self, String> {
        let port = parse_port(&optional_env_or("SELF_HOSTED_POSTGRES_APP_PORT", "5432"))?;

        Ok(Self::new(SelfHostedPostgresConfig {
            admin_url: required_env("SELF_HOSTED_POSTGRES_ADMIN_URL")?,
            app_host: optional_env_or("SELF_HOSTED_POSTGRES_APP_HOST", "postgres"),
            app_port: port,
            tenant_user: required_env("SELF_HOSTED_POSTGRES_TENANT_USER")?,
            tenant_password: required_env("SELF_HOSTED_POSTGRES_TENANT_PASSWORD")?,
            sslmode: optional_env_or("SELF_HOSTED_POSTGRES_SSLMODE", "disable"),
        }))
    }

    pub async fn create_database(&self, database_name: &str) -> Result<ProvisionedDatabase, String> {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(30))
            .connect(&self.config.admin_url)
            .await
            .map_err(|error| {
                format!(
                    "Failed to connect to self-hosted PostgreSQL admin URL: {}",
                    error
                )
            })?;

        let owner = quote_pg_identifier(&self.config.tenant_user);
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

        let connection_string = self.config.connection_string_for_database(database_name);
        self.wait_for_database_connectable(&connection_string).await?;

        Ok(ProvisionedDatabase {
            database_name: database_name.to_string(),
            connection_string,
        })
    }

    pub async fn drop_database_by_name(&self, database_name: &str) -> Result<(), String> {
        let pool = PgPoolOptions::new()
            .max_connections(1)
            .acquire_timeout(Duration::from_secs(30))
            .connect(&self.config.admin_url)
            .await
            .map_err(|error| {
                format!(
                    "Failed to connect to self-hosted PostgreSQL admin URL: {}",
                    error
                )
            })?;

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

        let drop_sql = format!(
            "DROP DATABASE IF EXISTS {} WITH (FORCE)",
            quote_pg_identifier(database_name)
        );
        let drop_result = sqlx::query(&drop_sql).execute(&pool).await;
        pool.close().await;

        drop_result.map(|_| ()).map_err(|error| {
            format!(
                "Failed to drop self-hosted database '{}': {}",
                database_name, error
            )
        })
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

- [ ] **Step 5: Run provisioner tests**

Run:

```bash
cd backend-admin
cargo test services::self_hosted_postgres::tests --lib
```

Expected: PASS.

- [ ] **Step 6: Run cargo check**

Run:

```bash
cd backend-admin
cargo check
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add backend-admin/src/services/self_hosted_postgres.rs backend-admin/src/services/mod.rs
git commit -m "feat(admin): add self-host postgres provisioner"
```

---

### Task 2: Replace Neon in School Provisioning

**Files:**
- Modify: `backend-admin/src/services/school_service.rs`

- [ ] **Step 1: Update school service tests**

Modify test imports in `backend-admin/src/services/school_service.rs`:

```rust
use super::{
    build_active_school_config, build_provisioning_failure_message, build_school_database_name,
    validate_school_subdomain, ProvisioningRunOptions,
};
```

Replace `active_school_config_records_database_and_deployment_url()`:

```rust
#[test]
fn active_school_config_records_deployment_url_without_neon_db_id() {
    let config = build_active_school_config("", "https://sandbox.schoolorbit.app");

    assert_eq!(config.db_id, None);
    assert_eq!(config.dns_record_id.as_deref(), Some(""));
    assert_eq!(
        config.deployment_url.as_deref(),
        Some("https://sandbox.schoolorbit.app")
    );
}
```

- [ ] **Step 2: Run tests to verify failure**

Run:

```bash
cd backend-admin
cargo test services::school_service::tests --lib
```

Expected: FAIL because `build_active_school_config()` still takes `db_id`.

- [ ] **Step 3: Update imports and config builder**

At the top of `backend-admin/src/services/school_service.rs`, use:

```rust
use crate::error::AppError;
use crate::models::{CreateSchool, School, SchoolConfig, UpdateSchool};
use crate::services::SelfHostedPostgresProvisioner;
use sqlx::{types::Json, PgPool};
use tracing::{error, info, warn};
use uuid::Uuid;
```

Replace `build_active_school_config()`:

```rust
fn build_active_school_config(dns_record_id: &str, deployment_url: &str) -> SchoolConfig {
    SchoolConfig {
        db_id: None,
        dns_record_id: Some(dns_record_id.to_string()),
        deployment_url: Some(deployment_url.to_string()),
    }
}
```

- [ ] **Step 4: Replace Neon creation in `provision_school()`**

Replace the block from `// Step 1: Create database in Neon` through the Neon connectability wait with:

```rust
        options
            .progress(2, TOTAL_STEPS, "Creating tenant database...")
            .await;

        let provisioner = SelfHostedPostgresProvisioner::from_env().map_err(|e| {
            AppError::ExternalServiceError(format!("Database provisioner error: {}", e))
        })?;

        let provisioned_database = match provisioner.create_database(&db_name).await {
            Ok(database) => database,
            Err(e) => {
                return Err(AppError::ExternalServiceError(format!(
                    "Failed to create tenant database: {}",
                    e
                )));
            }
        };

        options.success("✅ Tenant database created").await;
```

Replace `.bind(&db_connection_string)` in the school insert with:

```rust
        .bind(&provisioned_database.connection_string)
```

Replace rollback calls in the provisioning flow with:

```rust
self.rollback_failed_provisioning(Some(school_id), &provisioner, &db_name, reason).await
```

For the insert error before `school_id` exists, use:

```rust
self.rollback_failed_provisioning(None, &provisioner, &db_name, reason).await
```

Replace the call to `provision_tenant_database()`:

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
        let config = build_active_school_config(&dns_record_id, &subdomain_url);
```

- [ ] **Step 5: Update rollback helper signature**

Replace `rollback_failed_provisioning()`:

```rust
    async fn rollback_failed_provisioning(
        &self,
        school_id: Option<Uuid>,
        provisioner: &SelfHostedPostgresProvisioner,
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

        let database_deleted = match provisioner.drop_database_by_name(db_name).await {
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

- [ ] **Step 6: Update final activation config call**

Replace any remaining:

```rust
build_active_school_config(db_id, &dns_record_id, &subdomain_url)
```

with:

```rust
build_active_school_config(&dns_record_id, &subdomain_url)
```

- [ ] **Step 7: Verify Neon client is no longer referenced in school service**

Run:

```bash
rg -n "NeonClient|neon_client|NEON|Neon database|Creating database in Neon" backend-admin/src/services/school_service.rs
```

Expected: no output.

- [ ] **Step 8: Run tests and cargo check**

Run:

```bash
cd backend-admin
cargo test services::school_service::tests --lib
cargo check
```

Expected: both PASS.

- [ ] **Step 9: Commit**

```bash
git add backend-admin/src/services/school_service.rs
git commit -m "feat(admin): provision tenants on self-host postgres"
```

---

### Task 3: Replace Neon in School Deletion

**Files:**
- Modify: `backend-admin/src/services/school_service.rs`

- [ ] **Step 1: Add database-name helper test**

Add near existing tests in `backend-admin/src/services/school_service.rs`:

```rust
#[test]
fn school_database_name_uses_existing_db_name_column() {
    let school = School {
        id: Uuid::nil(),
        name: "Sandbox".to_string(),
        subdomain: "sandbox".to_string(),
        db_name: "schoolorbit_sandbox".to_string(),
        db_connection_string: None,
        status: "active".to_string(),
        config: Json(SchoolConfig::default()),
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
    };

    assert_eq!(database_name_for_school(&school), "schoolorbit_sandbox");
}
```

Add test imports:

```rust
use crate::models::{School, SchoolConfig};
use sqlx::types::Json;
use uuid::Uuid;
```

- [ ] **Step 2: Run test to verify failure**

Run:

```bash
cd backend-admin
cargo test services::school_service::tests::school_database_name_uses_existing_db_name_column --lib
```

Expected: FAIL because `database_name_for_school()` does not exist.

- [ ] **Step 3: Add helper**

Add near `build_school_database_name()`:

```rust
fn database_name_for_school(school: &School) -> &str {
    &school.db_name
}
```

- [ ] **Step 4: Replace direct Neon deletion in `delete_school()`**

Replace the `// Step 3: Delete Neon database` block with:

```rust
        info!("deleting tenant database");

        let db_name = database_name_for_school(&school);
        match SelfHostedPostgresProvisioner::from_env() {
            Ok(provisioner) => match provisioner.drop_database_by_name(db_name).await {
                Ok(_) => info!(db_name, "tenant database deleted"),
                Err(e) => {
                    warn!(error = %e, db_name, "failed to delete tenant database; manual cleanup may be needed");
                }
            },
            Err(e) => {
                warn!(error = %e, "database provisioner unavailable; manual tenant database cleanup may be needed");
            }
        }
```

Remove the stale `db_id` local variable if present.

- [ ] **Step 5: Replace direct Neon deletion in `delete_school_stream()`**

Replace the `// Step 3: Delete Neon database` block with:

```rust
        logger.progress(3, 4, "Deleting tenant database...").await;

        let db_name = database_name_for_school(&school);
        logger.info(&format!("Database name: {}", db_name)).await;

        match SelfHostedPostgresProvisioner::from_env() {
            Ok(provisioner) => match provisioner.drop_database_by_name(db_name).await {
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
                    .warning(&format!("⚠️  Database provisioner unavailable: {}", e))
                    .await
            }
        }
```

- [ ] **Step 6: Verify Neon wording is gone from service**

Run:

```bash
rg -n "Neon|neon_client|NEON|db_id" backend-admin/src/services/school_service.rs
```

Expected: no output, except tests or comments that still mention old migration history should be removed in this task.

- [ ] **Step 7: Run tests and cargo check**

Run:

```bash
cd backend-admin
cargo test services::school_service::tests --lib
cargo check
```

Expected: both PASS.

- [ ] **Step 8: Commit**

```bash
git add backend-admin/src/services/school_service.rs
git commit -m "feat(admin): delete self-host tenant databases"
```

---

### Task 4: Remove Neon Client and Required Neon Environment

**Files:**
- Delete: `backend-admin/src/clients/neon_client.rs`
- Modify: `backend-admin/src/clients/mod.rs`
- Modify: `backend-admin/src/main.rs`
- Modify: `.env.example`
- Modify: `backend-admin/.env.example`
- Modify: `backend-school/.env.portainer.example`

- [ ] **Step 1: Remove Neon client export**

Modify `backend-admin/src/clients/mod.rs`:

```rust
pub mod backend_school_client;
pub mod cloudflare_client;
```

- [ ] **Step 2: Delete Neon client file**

Run:

```bash
git rm backend-admin/src/clients/neon_client.rs
```

Expected: file removed from git.

- [ ] **Step 3: Update admin startup log**

In `backend-admin/src/main.rs`, replace:

```rust
    info!("connected to Neon PostgreSQL");
```

with:

```rust
    info!("connected to admin PostgreSQL database");
```

- [ ] **Step 4: Update env examples**

In `.env.example` and `backend-admin/.env.example`, ensure this self-host block exists and `NEON_*` variables are removed:

```dotenv
DATABASE_URL=postgresql://schoolorbit_admin_app:password@postgres:5432/schoolorbit_admin?sslmode=disable
SELF_HOSTED_POSTGRES_ADMIN_URL=postgresql://schoolorbit_provisioner:password@postgres:5432/postgres?sslmode=disable
SELF_HOSTED_POSTGRES_APP_HOST=postgres
SELF_HOSTED_POSTGRES_APP_PORT=5432
SELF_HOSTED_POSTGRES_TENANT_USER=schoolorbit_tenant_owner
SELF_HOSTED_POSTGRES_TENANT_PASSWORD=change-this-tenant-password
SELF_HOSTED_POSTGRES_SSLMODE=disable
BACKUP_ROOT=/opt/schoolorbit/backups
BACKUP_RETENTION_DAYS=14
BACKUP_RCLONE_REMOTE=
```

In `backend-school/.env.portainer.example`, add:

```dotenv
# Tenant database URLs are resolved through backend-admin.
# backend-school does not need DATABASE_URL or Neon credentials.
# backend-admin stores PostgreSQL 18 self-host tenant URLs in schools.db_connection_string.
```

- [ ] **Step 5: Verify no required Neon references remain**

Run:

```bash
rg -n "NEON_|Neon|neon_client" backend-admin/src .env.example backend-admin/.env.example backend-school/.env.portainer.example podman-compose.yml docker-compose.yml
```

Expected: no output.

- [ ] **Step 6: Run cargo check**

Run:

```bash
cd backend-admin
cargo check
```

Expected: PASS.

- [ ] **Step 7: Commit**

```bash
git add backend-admin/src/clients/mod.rs backend-admin/src/main.rs .env.example backend-admin/.env.example backend-school/.env.portainer.example
git add -u backend-admin/src/clients/neon_client.rs
git commit -m "chore: remove neon runtime configuration"
```

---

### Task 5: Add PostgreSQL 18 to Compose

**Files:**
- Modify: `podman-compose.yml`
- Modify: `docker-compose.yml`

- [ ] **Step 1: Update production Podman compose**

In `podman-compose.yml`, remove the comment that says production uses Neon. Add this service before `backend-admin`:

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
```

In `backend-admin.environment`, use:

```yaml
      DATABASE_URL: ${DATABASE_URL}
      SELF_HOSTED_POSTGRES_ADMIN_URL: ${SELF_HOSTED_POSTGRES_ADMIN_URL}
      SELF_HOSTED_POSTGRES_APP_HOST: ${SELF_HOSTED_POSTGRES_APP_HOST:-postgres}
      SELF_HOSTED_POSTGRES_APP_PORT: ${SELF_HOSTED_POSTGRES_APP_PORT:-5432}
      SELF_HOSTED_POSTGRES_TENANT_USER: ${SELF_HOSTED_POSTGRES_TENANT_USER}
      SELF_HOSTED_POSTGRES_TENANT_PASSWORD: ${SELF_HOSTED_POSTGRES_TENANT_PASSWORD}
      SELF_HOSTED_POSTGRES_SSLMODE: ${SELF_HOSTED_POSTGRES_SSLMODE:-disable}
```

Add `depends_on` for `backend-admin`:

```yaml
    depends_on:
      postgres:
        condition: service_healthy
```

Add volume:

```yaml
volumes:
  postgres_data:
    driver: local
```

- [ ] **Step 2: Update local Docker compose**

In `docker-compose.yml`, change the Postgres image:

```yaml
    image: postgres:18-alpine
```

In `backend-admin.environment`, set:

```yaml
      - DATABASE_URL=${DATABASE_URL:-postgresql://admin_user:password@postgres:5432/schoolorbit_admin?sslmode=disable}
      - SELF_HOSTED_POSTGRES_ADMIN_URL=${SELF_HOSTED_POSTGRES_ADMIN_URL:-postgresql://admin_user:password@postgres:5432/postgres?sslmode=disable}
      - SELF_HOSTED_POSTGRES_APP_HOST=${SELF_HOSTED_POSTGRES_APP_HOST:-postgres}
      - SELF_HOSTED_POSTGRES_APP_PORT=${SELF_HOSTED_POSTGRES_APP_PORT:-5432}
      - SELF_HOSTED_POSTGRES_TENANT_USER=${SELF_HOSTED_POSTGRES_TENANT_USER:-admin_user}
      - SELF_HOSTED_POSTGRES_TENANT_PASSWORD=${SELF_HOSTED_POSTGRES_TENANT_PASSWORD:-password}
      - SELF_HOSTED_POSTGRES_SSLMODE=${SELF_HOSTED_POSTGRES_SSLMODE:-disable}
```

- [ ] **Step 3: Validate compose syntax**

Run:

```bash
docker compose -f docker-compose.yml config >/tmp/schoolorbit-compose.out
docker compose -f podman-compose.yml config >/tmp/schoolorbit-podman-compose.out
```

Expected: both commands exit 0. If Docker Compose is unavailable locally, record that compose validation must run on the deployment host before production.

- [ ] **Step 4: Commit**

```bash
git add podman-compose.yml docker-compose.yml
git commit -m "chore: add postgres 18 self-host compose"
```

---

### Task 6: Add Native Backup Script

**Files:**
- Create: `scripts/backup_postgres.sh`

- [ ] **Step 1: Create backup script**

Create `scripts/backup_postgres.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

timestamp="$(date -u +%Y%m%dT%H%M%SZ)"
backup_root="${BACKUP_ROOT:-./backups/postgres}"
retention_days="${BACKUP_RETENTION_DAYS:-14}"
admin_database_url="${DATABASE_URL:-}"
admin_backup_name="schoolorbit_admin_${timestamp}.dump"
backup_dir="${backup_root}/${timestamp}"

if [[ -z "$admin_database_url" ]]; then
    printf 'Set DATABASE_URL to the backend-admin database URL.\n' >&2
    exit 1
fi

for command in pg_dump psql; do
    if ! command -v "$command" >/dev/null 2>&1; then
        printf '%s is required for PostgreSQL backups.\n' "$command" >&2
        exit 1
    fi
done

mkdir -p "$backup_dir"
chmod 700 "$backup_dir"

dump_database() {
    local database_url="$1"
    local output_path="$2"

    pg_dump "$database_url" \
        --format=custom \
        --no-owner \
        --no-privileges \
        --file="$output_path"
}

printf 'Backing up admin database...\n'
dump_database "$admin_database_url" "${backup_dir}/${admin_backup_name}"

tenant_list="${backup_dir}/tenants.tsv"
psql "$admin_database_url" -v ON_ERROR_STOP=1 -X -q -A -F $'\t' -t <<SQL > "$tenant_list"
SELECT subdomain, db_connection_string
FROM schools
WHERE status IN ('active', 'provisioning')
  AND db_connection_string IS NOT NULL
ORDER BY subdomain;
SQL

while IFS=$'\t' read -r subdomain database_url; do
    if [[ -z "$subdomain" || -z "$database_url" ]]; then
        continue
    fi

    safe_subdomain="${subdomain//[^A-Za-z0-9_-]/_}"
    output_path="${backup_dir}/tenant_${safe_subdomain}_${timestamp}.dump"
    printf 'Backing up tenant: %s\n' "$subdomain"
    dump_database "$database_url" "$output_path"
done < "$tenant_list"

manifest="${backup_dir}/manifest.txt"
{
    printf 'timestamp=%s\n' "$timestamp"
    printf 'admin_backup=%s\n' "$admin_backup_name"
    find "$backup_dir" -maxdepth 1 -name '*.dump' -printf '%f\n' | sort
} > "$manifest"

if [[ -n "${BACKUP_RCLONE_REMOTE:-}" ]]; then
    if ! command -v rclone >/dev/null 2>&1; then
        printf 'BACKUP_RCLONE_REMOTE is set but rclone is not installed.\n' >&2
        exit 1
    fi
    rclone copy "$backup_dir" "${BACKUP_RCLONE_REMOTE%/}/${timestamp}/"
fi

find "$backup_root" -mindepth 1 -maxdepth 1 -type d -mtime "+$retention_days" -print -exec rm -rf {} +

printf 'Backup completed: %s\n' "$backup_dir"
```

- [ ] **Step 2: Make script executable**

Run:

```bash
chmod +x scripts/backup_postgres.sh
```

- [ ] **Step 3: Syntax-check script**

Run:

```bash
bash -n scripts/backup_postgres.sh
```

Expected: exits 0.

- [ ] **Step 4: Commit**

```bash
git add scripts/backup_postgres.sh
git commit -m "feat(ops): add postgres backup script"
```

---

### Task 7: Add Restore Check Script

**Files:**
- Create: `scripts/restore_check_postgres.sh`

- [ ] **Step 1: Create restore check script**

Create `scripts/restore_check_postgres.sh`:

```bash
#!/usr/bin/env bash
set -euo pipefail

admin_url="${RESTORE_CHECK_ADMIN_URL:-${SELF_HOSTED_POSTGRES_ADMIN_URL:-}}"
dump_path="${RESTORE_CHECK_DUMP_PATH:-}"
database_kind="${RESTORE_CHECK_KIND:-tenant}"
database_name="restore_check_$(date -u +%Y%m%d%H%M%S)_$$"

if [[ -z "$admin_url" ]]; then
    printf 'Set RESTORE_CHECK_ADMIN_URL or SELF_HOSTED_POSTGRES_ADMIN_URL.\n' >&2
    exit 1
fi

if [[ -z "$dump_path" || ! -f "$dump_path" ]]; then
    printf 'Set RESTORE_CHECK_DUMP_PATH to an existing .dump file.\n' >&2
    exit 1
fi

for command in createdb dropdb pg_restore psql; do
    if ! command -v "$command" >/dev/null 2>&1; then
        printf '%s is required for restore checks.\n' "$command" >&2
        exit 1
    fi
done

cleanup() {
    dropdb "$admin_url" "$database_name" --if-exists >/dev/null 2>&1 || true
}
trap cleanup EXIT

createdb "$admin_url" "$database_name"

restore_url="${admin_url%/*}/${database_name}"
pg_restore \
    --dbname "$restore_url" \
    --no-owner \
    --no-privileges \
    "$dump_path"

case "$database_kind" in
    admin)
        psql "$restore_url" -v ON_ERROR_STOP=1 -X -q <<SQL
SELECT COUNT(*) FROM schools;
SQL
        ;;
    tenant)
        psql "$restore_url" -v ON_ERROR_STOP=1 -X -q <<SQL
SELECT COUNT(*) FROM _sqlx_migrations WHERE success;
SELECT COUNT(*) FROM permissions;
SQL
        ;;
    *)
        printf 'RESTORE_CHECK_KIND must be admin or tenant.\n' >&2
        exit 1
        ;;
esac

printf 'Restore check passed for %s dump: %s\n' "$database_kind" "$dump_path"
```

- [ ] **Step 2: Make script executable**

Run:

```bash
chmod +x scripts/restore_check_postgres.sh
```

- [ ] **Step 3: Syntax-check script**

Run:

```bash
bash -n scripts/restore_check_postgres.sh
```

Expected: exits 0.

- [ ] **Step 4: Commit**

```bash
git add scripts/restore_check_postgres.sh
git commit -m "feat(ops): add postgres restore check script"
```

---

### Task 8: Add Migration and Backup Runbook

**Files:**
- Create: `docs/POSTGRES_SELF_HOST_MIGRATION.md`

- [ ] **Step 1: Write runbook**

Create `docs/POSTGRES_SELF_HOST_MIGRATION.md`:

```markdown
# PostgreSQL 18 Self-Host Migration Runbook

This runbook moves SchoolOrbit from Neon to PostgreSQL 18 self-hosted. The app does not keep a Neon runtime path after this migration.

## 1. Required Environment

Set these values in production:

```dotenv
POSTGRES_USER=schoolorbit_provisioner
POSTGRES_PASSWORD=${POSTGRES_PASSWORD}
ADMIN_APP_PASSWORD=${ADMIN_APP_PASSWORD}
TENANT_OWNER_PASSWORD=${TENANT_OWNER_PASSWORD}
DATABASE_URL=postgresql://schoolorbit_admin_app:${ADMIN_APP_PASSWORD}@postgres:5432/schoolorbit_admin?sslmode=disable
SELF_HOSTED_POSTGRES_ADMIN_URL=postgresql://schoolorbit_provisioner:${POSTGRES_PASSWORD}@postgres:5432/postgres?sslmode=disable
SELF_HOSTED_POSTGRES_APP_HOST=postgres
SELF_HOSTED_POSTGRES_APP_PORT=5432
SELF_HOSTED_POSTGRES_TENANT_USER=schoolorbit_tenant_owner
SELF_HOSTED_POSTGRES_TENANT_PASSWORD=${TENANT_OWNER_PASSWORD}
SELF_HOSTED_POSTGRES_SSLMODE=disable
BACKUP_ROOT=/opt/schoolorbit/backups
BACKUP_RETENTION_DAYS=14
BACKUP_RCLONE_REMOTE=
```

## 2. Create Roles and Admin Database

Set password variables and connect as the provisioner:

```bash
export ADMIN_APP_PASSWORD='replace-with-admin-app-password'
export TENANT_OWNER_PASSWORD='replace-with-tenant-owner-password'

podman exec -it schoolorbit-postgres \
  psql -U "$POSTGRES_USER" -d postgres \
  -v ADMIN_APP_PASSWORD="$ADMIN_APP_PASSWORD" \
  -v TENANT_OWNER_PASSWORD="$TENANT_OWNER_PASSWORD"
```

Run inside `psql`:

```sql
CREATE ROLE schoolorbit_admin_app LOGIN PASSWORD :'ADMIN_APP_PASSWORD';
CREATE ROLE schoolorbit_tenant_owner LOGIN PASSWORD :'TENANT_OWNER_PASSWORD';
CREATE DATABASE schoolorbit_admin OWNER schoolorbit_admin_app;
GRANT CREATE ON DATABASE postgres TO schoolorbit_provisioner;
```

## 3. Migrate backend-admin Data

Export from Neon before shutting it down:

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

## 4. Backup and Restore Check

Run backup:

```bash
DATABASE_URL="$DATABASE_URL" \
BACKUP_ROOT=/opt/schoolorbit/backups \
BACKUP_RETENTION_DAYS=14 \
  ./scripts/backup_postgres.sh
```

Run restore check for an admin dump:

```bash
latest_backup_dir="$(find /opt/schoolorbit/backups -mindepth 1 -maxdepth 1 -type d | sort | tail -1)"
admin_dump="$(find "$latest_backup_dir" -maxdepth 1 -name 'schoolorbit_admin_*.dump' | sort | tail -1)"

RESTORE_CHECK_ADMIN_URL="$SELF_HOSTED_POSTGRES_ADMIN_URL" \
RESTORE_CHECK_KIND=admin \
RESTORE_CHECK_DUMP_PATH="$admin_dump" \
  ./scripts/restore_check_postgres.sh
```

Run restore check for a tenant dump:

```bash
tenant_dump="$(find "$latest_backup_dir" -maxdepth 1 -name 'tenant_sandbox_*.dump' | sort | tail -1)"

RESTORE_CHECK_ADMIN_URL="$SELF_HOSTED_POSTGRES_ADMIN_URL" \
RESTORE_CHECK_KIND=tenant \
RESTORE_CHECK_DUMP_PATH="$tenant_dump" \
  ./scripts/restore_check_postgres.sh
```

## 5. Provision Sandbox Tenant

Create a sandbox school from the admin UI or API. Verify the tenant database:

```bash
MIGRATION_AUDIT_DATABASE_URL="postgresql://schoolorbit_tenant_owner:${TENANT_OWNER_PASSWORD}@postgres:5432/schoolorbit_sandbox?sslmode=disable" \
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

## 6. Copy Important Neon Tenants

Prepare a clean target tenant database first:

```bash
PREPARE_CLEAN_TENANT_DATABASE_URL="postgresql://schoolorbit_tenant_owner:${TENANT_OWNER_PASSWORD}@postgres:5432/schoolorbit_target?sslmode=disable" \
PREPARE_CLEAN_TENANT_CONFIRM=public \
PREPARE_CLEAN_TENANT_ALLOW_NON_TEST=1 \
  ./scripts/prepare_clean_tenant_db.sh
```

Dry-run data copy:

```bash
CUTOVER_SOURCE_DATABASE_URL="$NEON_TENANT_DATABASE_URL" \
CUTOVER_TARGET_DATABASE_URL="postgresql://schoolorbit_tenant_owner:${TENANT_OWNER_PASSWORD}@postgres:5432/schoolorbit_target?sslmode=disable" \
  ./scripts/cutover_tenant_data.sh
```

Apply data copy:

```bash
CUTOVER_MODE=apply \
CUTOVER_SOURCE_DATABASE_URL="$NEON_TENANT_DATABASE_URL" \
CUTOVER_TARGET_DATABASE_URL="postgresql://schoolorbit_tenant_owner:${TENANT_OWNER_PASSWORD}@postgres:5432/schoolorbit_target?sslmode=disable" \
CUTOVER_TARGET_SCHEMA=public \
CUTOVER_ALLOW_PUBLIC_TARGET=1 \
CUTOVER_ALLOW_NON_TEST_TARGET=1 \
CUTOVER_CONFIRM_TARGET_TRUNCATE=public \
  ./scripts/cutover_tenant_data.sh
```

Update `backend-admin.schools.db_connection_string` for that tenant only after dry-run and apply checks pass.

## 7. Shut Down Neon Billing

After self-hosted production validation passes:

1. keep final dump files in object storage;
2. run restore checks for representative admin and tenant dumps;
3. verify production login and smoke tests;
4. delete Neon databases and close paid Neon resources.
```

- [ ] **Step 2: Check runbook markers**

Run:

```bash
rg -n "UNRESOLVED|REPLACE_ME" docs/POSTGRES_SELF_HOST_MIGRATION.md
```

Expected: no output.

- [ ] **Step 3: Commit**

```bash
git add docs/POSTGRES_SELF_HOST_MIGRATION.md
git commit -m "docs: add self-host postgres migration runbook"
```

---

### Task 9: Full Verification

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

- [ ] **Step 3: Run backend-school migration tests**

Run:

```bash
cd backend-school
cargo test db::migration::tests --bin backend-school
```

Expected: PASS.

- [ ] **Step 4: Check scripts**

Run:

```bash
bash -n scripts/backup_postgres.sh
bash -n scripts/restore_check_postgres.sh
```

Expected: both commands exit 0.

- [ ] **Step 5: Validate compose syntax**

Run:

```bash
docker compose -f docker-compose.yml config >/tmp/schoolorbit-compose.out
docker compose -f podman-compose.yml config >/tmp/schoolorbit-podman-compose.out
```

Expected: both commands exit 0, or document that compose validation must run on the deployment host if Docker Compose is unavailable locally.

- [ ] **Step 6: Run repository checks**

Run:

```bash
git diff --check
git status --short
```

Expected: `git diff --check` exits 0. `git status --short` should show no uncommitted implementation changes.

---

### Task 10: Sandbox Deployment Verification

**Files:**
- No repository changes expected unless the runbook needs correction.

- [ ] **Step 1: Start self-host stack**

Run on the deployment host:

```bash
cd /opt/stack
podman-compose -f podman-compose.yml up -d postgres backend-admin backend-school
```

Expected:

```text
schoolorbit-postgres       running/healthy
schoolorbit-backend-admin  running/healthy
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

- backend-admin logs include `Tenant database created`;
- `schools.db_connection_string` points to the self-hosted PostgreSQL host;
- backend-school provisioning completes.

- [ ] **Step 5: Verify tenant migration baseline**

Run:

```bash
MIGRATION_AUDIT_DATABASE_URL="postgresql://schoolorbit_tenant_owner:${TENANT_OWNER_PASSWORD}@postgres:5432/schoolorbit_sandbox?sslmode=disable" \
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

- [ ] **Step 7: Verify backup and restore check**

Run:

```bash
DATABASE_URL="$DATABASE_URL" \
BACKUP_ROOT=/opt/schoolorbit/backups \
  ./scripts/backup_postgres.sh
```

Then run restore checks for one admin dump and the sandbox tenant dump:

```bash
latest_backup_dir="$(find /opt/schoolorbit/backups -mindepth 1 -maxdepth 1 -type d | sort | tail -1)"
admin_dump="$(find "$latest_backup_dir" -maxdepth 1 -name 'schoolorbit_admin_*.dump' | sort | tail -1)"
tenant_dump="$(find "$latest_backup_dir" -maxdepth 1 -name 'tenant_sandbox_*.dump' | sort | tail -1)"

RESTORE_CHECK_ADMIN_URL="$SELF_HOSTED_POSTGRES_ADMIN_URL" \
RESTORE_CHECK_KIND=admin \
RESTORE_CHECK_DUMP_PATH="$admin_dump" \
  ./scripts/restore_check_postgres.sh

RESTORE_CHECK_ADMIN_URL="$SELF_HOSTED_POSTGRES_ADMIN_URL" \
RESTORE_CHECK_KIND=tenant \
RESTORE_CHECK_DUMP_PATH="$tenant_dump" \
  ./scripts/restore_check_postgres.sh
```

Expected: both restore checks pass.

- [ ] **Step 8: Record deployment result**

If sandbox succeeds, add this section to `docs/POSTGRES_SELF_HOST_MIGRATION.md`:

```markdown
## Sandbox Verification Log

- 2026-06-24: Sandbox tenant provisioned on PostgreSQL 18 self-hosted. Smoke test passed. Native backup and restore check passed.
```

Commit:

```bash
git add docs/POSTGRES_SELF_HOST_MIGRATION.md
git commit -m "docs: record self-host sandbox verification"
```
