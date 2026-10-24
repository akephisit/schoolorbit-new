mod permission_sync {
    /// Permission sync utility - Auto-sync permission registry to database
    use school_permissions::registry::ALL_PERMISSIONS;
    use sqlx::{PgPool, Postgres, QueryBuilder};

    /// Sync all permissions from registry to database
    /// This is called after migrations complete to ensure DB is up-to-date
    pub async fn sync_permissions(pool: &PgPool) -> Result<(), sqlx::Error> {
        // Preserve the empty-registry safeguard: never deactivate every permission.
        let registry_codes: Vec<&str> = ALL_PERMISSIONS.iter().map(|p| p.code).collect();
        if registry_codes.is_empty() {
            return Ok(());
        }
        let mut transaction = pool.begin().await?;
        // Keep retired rows and their grants for reconciliation, without authorization.
        let deactivated = school_database::DbSession::new(&mut *transaction)
            .query(
                "migrations.src.lib.sync_permissions",
                "UPDATE permissions SET is_active = false, updated_at = NOW()
             WHERE is_active = true AND NOT (code = ANY($1::text[]))",
            )
            .bind(&registry_codes)
            .execute()
            .await?
            .rows_affected();

        let mut query = QueryBuilder::<Postgres>::new(
            "INSERT INTO permissions (code, name, module, action, scope, description, is_active) ",
        );
        query.push_values(ALL_PERMISSIONS, |mut row, permission| {
            row.push_bind(permission.code)
                .push_bind(permission.name)
                .push_bind(permission.module)
                .push_bind(permission.action)
                .push_bind(permission.scope)
                .push_bind(permission.description)
                .push_bind(true);
        });
        query.push(
            " ON CONFLICT (code) DO UPDATE SET
              name = EXCLUDED.name, module = EXCLUDED.module, action = EXCLUDED.action,
              scope = EXCLUDED.scope, description = EXCLUDED.description,
              is_active = true, updated_at = NOW()",
        );
        school_database::DbSession::new(&mut *transaction)
            .prepared_query("migrations.src.lib.sync_permissions", query.build())
            .execute()
            .await?;
        transaction.commit().await?;
        if deactivated > 0 {
            tracing::info!(
                "Deactivated {} permissions absent from the canonical registry",
                deactivated
            );
        }

        Ok(())
    }
}

pub use permission_sync::sync_permissions;

use dashmap::DashMap;
use sqlx::migrate::Migrator;
use sqlx::PgPool;
use std::borrow::Cow;
use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

fn all_migrations_without_db_lock() -> Migrator {
    let base = sqlx::migrate!("../../migrations");
    Migrator {
        migrations: Cow::Owned(base.iter().cloned().collect()),
        ignore_missing: base.ignore_missing,
        locking: false,
        no_tx: base.no_tx,
        table_name: base.table_name,
        create_schemas: base.create_schemas,
    }
}

static TENANT_MIGRATION_GATE: Mutex<()> = Mutex::const_new(());

// Classify before erasing the native error: driver details can contain SQL and values.
fn safe_migration_failure(error: sqlx::migrate::MigrateError) -> String {
    use sqlx::migrate::MigrateError;
    let (category, version) = match &error {
        MigrateError::Execute(error) => (school_database::error_class(error), None),
        MigrateError::ExecuteMigration(error, version) => {
            (school_database::error_class(error), Some(*version))
        }
        MigrateError::Source(_) => ("source", None),
        MigrateError::VersionMissing(version) => ("version_missing", Some(*version)),
        MigrateError::VersionMismatch(version) => ("version_mismatch", Some(*version)),
        MigrateError::VersionNotPresent(version) => ("version_not_present", Some(*version)),
        MigrateError::VersionTooOld(version, _) => ("version_too_old", Some(*version)),
        MigrateError::VersionTooNew(version, _) => ("version_too_new", Some(*version)),
        MigrateError::Dirty(version) => ("dirty", Some(*version)),
        _ => ("migration_configuration", None),
    };
    match version {
        Some(version) => format!("Migration failed: {category} (version {version})"),
        None => format!("Migration failed: {category}"),
    }
}

pub async fn run_tenant_migrations(pool: &PgPool) -> Result<(), String> {
    let started = std::time::Instant::now();
    let owner: String = sqlx::query_scalar("SELECT current_user::text")
        .fetch_one(pool)
        .await
        .map_err(|_| "Migration identity unavailable".to_string())?;
    if owner.starts_with("so_") && owner.ends_with("_runtime") {
        return Err("Runtime identities cannot migrate".into());
    }
    let runtime = runtime_role_for_owner(&owner)?;
    // The deployment/provision endpoints share one process. Serialize real tenant
    // owner jobs without serializing unrelated disposable test schemas.
    let _guard = if runtime.is_some() {
        Some(TENANT_MIGRATION_GATE.lock().await)
    } else {
        None
    };
    all_migrations_without_db_lock()
        .run(pool)
        .await
        .map_err(safe_migration_failure)?;

    sync_permissions(pool).await.map_err(|error| {
        format!(
            "Permission sync failed after migrations: {}",
            school_database::error_class(&error)
        )
    })?;

    if let Some(runtime) = runtime {
        grant_runtime_privileges(pool, &runtime).await?;
    }

    tracing::info!(
        operation = "tenant_migration",
        duration_ms = started.elapsed().as_secs_f64() * 1000.0
    );

    Ok(())
}

/// Runtime readiness validates the immutable timeline; it never creates migration tables.
pub async fn verify_tenant_schema<'e, E>(executor: E) -> Result<i64, String>
where
    E: sqlx::Executor<'e, Database = sqlx::Postgres>,
{
    let applied: Vec<(i64, bool, Vec<u8>)> =
        sqlx::query_as("SELECT version, success, checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(executor)
            .await
            .map_err(|_| "Tenant migration history unavailable".to_string())?;
    let migrator = all_migrations_without_db_lock();
    if applied.len() != migrator.iter().count()
        || !applied
            .iter()
            .zip(migrator.iter())
            .all(|((version, success, checksum), expected)| {
                *success
                    && *version == expected.version
                    && checksum.as_slice() == expected.checksum.as_ref()
            })
    {
        return Err("Tenant schema is not ready for this release".into());
    }
    Ok(applied.last().map_or(0, |(version, _, _)| *version))
}

/// Only the tenant owner may grant the corresponding runtime role. Disposable
/// test databases have no production role pair and keep their fixture ownership.
fn runtime_role_for_owner(owner: &str) -> Result<Option<String>, String> {
    let Some(identity) = owner
        .strip_prefix("so_")
        .and_then(|s| s.strip_suffix("_owner"))
    else {
        return Ok(None);
    };
    if identity.len() != 32 || !identity.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Invalid migration identity".into());
    }
    Ok(Some(format!("so_{identity}_runtime")))
}

async fn grant_runtime_privileges(pool: &PgPool, runtime: &str) -> Result<(), String> {
    let statement = format!(
        "REVOKE CREATE ON SCHEMA public FROM PUBLIC; GRANT USAGE ON SCHEMA public TO \"{runtime}\";
         GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO \"{runtime}\";
         GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO \"{runtime}\";
         REVOKE INSERT, UPDATE, DELETE ON _sqlx_migrations FROM \"{runtime}\";
         ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO \"{runtime}\";
         ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT USAGE, SELECT ON SEQUENCES TO \"{runtime}\""
    );
    sqlx::raw_sql(sqlx::AssertSqlSafe(statement))
        .execute(pool)
        .await
        .map_err(|_| "Runtime privilege reconciliation failed".to_string())?;
    Ok(())
}

/// Track which schools have been migrated and synced in this session
#[derive(Clone)]
pub struct MigrationTracker {
    migrated: Arc<RwLock<HashSet<String>>>,
    migration_locks: Arc<DashMap<String, Arc<Mutex<()>>>>,
}

impl MigrationTracker {
    pub fn new() -> Self {
        Self {
            migrated: Arc::new(RwLock::new(HashSet::new())),
            migration_locks: Arc::new(DashMap::new()),
        }
    }

    async fn run_once<F, Fut>(
        &self,
        subdomain: &str,
        completed: &RwLock<HashSet<String>>,
        locks: &DashMap<String, Arc<Mutex<()>>>,
        operation: F,
    ) -> Result<bool, String>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<(), String>>,
    {
        {
            let completed = completed.read().await;
            if completed.contains(subdomain) {
                return Ok(false);
            }
        }

        let lock = locks
            .entry(subdomain.to_string())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();
        let _guard = lock.lock().await;

        {
            let completed = completed.read().await;
            if completed.contains(subdomain) {
                return Ok(false);
            }
        }

        operation().await?;

        let mut completed = completed.write().await;
        completed.insert(subdomain.to_string());

        Ok(true)
    }

    /// Run migrations and permission reconciliation as one tracked operation.
    pub async fn run_migrations_once(
        &self,
        subdomain: &str,
        pool: &PgPool,
    ) -> Result<bool, String> {
        self.run_once(subdomain, &self.migrated, &self.migration_locks, || async {
            tracing::info!("🔄 Running migrations for school: {}", subdomain);

            run_tenant_migrations(pool)
                .await
                .map_err(|e| format!("Migration failed for {}: {}", subdomain, e))?;

            tracing::info!("✅ Migrations completed for: {}", subdomain);
            Ok(())
        })
        .await
    }

    /// Get list of migrated schools
    pub async fn get_migrated_schools(&self) -> Vec<String> {
        let migrated = self.migrated.read().await;
        migrated.iter().cloned().collect()
    }

    /// Get migration count
    pub async fn migration_count(&self) -> usize {
        let migrated = self.migrated.read().await;
        migrated.len()
    }
}

impl Default for MigrationTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn run_once_allows_only_one_concurrent_operation_per_subdomain() {
        let tracker = MigrationTracker::new();
        let operation_count = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();

        for _ in 0..20 {
            let tracker = tracker.clone();
            let operation_count = operation_count.clone();

            handles.push(tokio::spawn(async move {
                tracker
                    .run_once(
                        "sandbox",
                        &tracker.migrated,
                        &tracker.migration_locks,
                        || {
                            let operation_count = operation_count.clone();
                            async move {
                                operation_count.fetch_add(1, Ordering::SeqCst);
                                sleep(Duration::from_millis(20)).await;
                                Ok(())
                            }
                        },
                    )
                    .await
            }));
        }

        let mut newly_run = 0;
        for handle in handles {
            if handle
                .await
                .expect("task should finish")
                .expect("run_once should succeed")
            {
                newly_run += 1;
            }
        }

        assert_eq!(newly_run, 1);
        assert_eq!(operation_count.load(Ordering::SeqCst), 1);
        assert_eq!(tracker.migration_count().await, 1);
    }

    #[test]
    fn active_migrator_uses_contiguous_versions_without_db_lock() {
        let migrator = all_migrations_without_db_lock();
        let versions = migrator
            .iter()
            .map(|migration| migration.version)
            .collect::<Vec<_>>();
        let expected_versions = (1..=versions.len() as i64).collect::<Vec<_>>();

        assert!(!migrator.locking);
        assert_eq!(versions, expected_versions);
    }

    #[test]
    fn active_baseline_sql_is_clean_application_schema() {
        let baseline_sql = include_str!("../../../migrations/001_baseline.sql");

        assert!(!baseline_sql.trim().is_empty());
        assert!(
            !baseline_sql.contains("_sqlx_migrations"),
            "clean baseline SQL must not include SQLx migration history"
        );
        assert!(
            !baseline_sql.contains("schoolorbit_baseline_"),
            "baseline SQL must not contain a temporary generation schema name"
        );
        assert!(
            !baseline_sql.contains("\\restrict") && !baseline_sql.contains("\\unrestrict"),
            "baseline SQL must not contain pg_dump psql meta-commands because sqlx executes it directly"
        );
        assert!(
            !baseline_sql.contains("set_config('search_path', '', false)"),
            "baseline SQL must not clear search_path; it must honor the connection search_path for test schemas"
        );
        assert!(
            baseline_sql.contains("CREATE EXTENSION IF NOT EXISTS \"uuid-ossp\";")
                && baseline_sql.contains("CREATE EXTENSION IF NOT EXISTS pg_trgm;"),
            "baseline SQL must include database extensions before schema objects"
        );
        assert!(
            baseline_sql.contains("organization_work.approve.organization_unit")
                && baseline_sql.contains("academic_curriculum.manage.organization_tree")
                && baseline_sql.contains("ORG-BASELINE-V1"),
            "baseline SQL must include canonical permissions and organization template data"
        );
    }
}
