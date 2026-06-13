use dashmap::DashMap;
use sqlx::migrate::Migrator;
use sqlx::PgPool;
use std::borrow::Cow;
use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

fn all_migrations_without_db_lock() -> Migrator {
    let base = sqlx::migrate!("./migrations");
    Migrator {
        migrations: Cow::Owned(base.iter().cloned().collect()),
        ignore_missing: base.ignore_missing,
        locking: false,
        no_tx: base.no_tx,
    }
}

pub async fn run_tenant_migrations(pool: &PgPool) -> Result<(), String> {
    all_migrations_without_db_lock()
        .run(pool)
        .await
        .map_err(|error| format!("Migration failed: {}", error))?;

    crate::utils::permission_sync::sync_permissions(pool)
        .await
        .map_err(|error| format!("Permission sync failed after migrations: {}", error))?;

    Ok(())
}

/// Track which schools have been migrated and synced in this session
#[derive(Clone)]
pub struct MigrationTracker {
    migrated: Arc<RwLock<HashSet<String>>>,
    permissions_synced: Arc<RwLock<HashSet<String>>>,
    migration_locks: Arc<DashMap<String, Arc<Mutex<()>>>>,
    permission_locks: Arc<DashMap<String, Arc<Mutex<()>>>>,
}

impl MigrationTracker {
    pub fn new() -> Self {
        Self {
            migrated: Arc::new(RwLock::new(HashSet::new())),
            permissions_synced: Arc::new(RwLock::new(HashSet::new())),
            migration_locks: Arc::new(DashMap::new()),
            permission_locks: Arc::new(DashMap::new()),
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

    /// Run migrations for a school (once per session)
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

    /// Sync permissions for a school (once per session)
    pub async fn sync_permissions_once(
        &self,
        subdomain: &str,
        pool: &PgPool,
    ) -> Result<bool, String> {
        self.run_once(
            subdomain,
            &self.permissions_synced,
            &self.permission_locks,
            || async {
                tracing::info!("🔄 Syncing permissions for school: {}", subdomain);

                crate::utils::permission_sync::sync_permissions(pool)
                    .await
                    .map_err(|e| format!("Permission sync failed for {}: {}", subdomain, e))?;

                tracing::info!("✅ Permissions synced for: {}", subdomain);
                Ok(())
            },
        )
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
    fn clean_migrator_contains_only_the_baseline_migration() {
        let migrator = all_migrations_without_db_lock();
        let versions = migrator
            .iter()
            .map(|migration| migration.version)
            .collect::<Vec<_>>();

        assert!(!migrator.locking);
        assert_eq!(versions, vec![1]);
    }

    #[test]
    fn active_baseline_sql_is_clean_application_schema() {
        let baseline_sql = include_str!("../../migrations/001_baseline.sql");

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
