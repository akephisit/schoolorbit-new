use crate::permissions::registry::{codes, PermissionDef, ALL_PERMISSIONS};
use dashmap::DashMap;
use sqlx::migrate::Migrator;
use sqlx::PgPool;
use std::borrow::Cow;
use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};

pub const PERMISSION_SYNC_MIGRATION_CHECKPOINT_VERSION: i64 = 120;

const GRANT_BASELINE_REQUIRED_PERMISSION_CODES: &[&str] = &[
    codes::ACADEMIC_CURRICULUM_MANAGE_ORGANIZATION_UNIT,
    codes::ORGANIZATION_WORK_READ_OWN,
    codes::ORGANIZATION_WORK_READ_ORGANIZATION_UNIT,
    // Migration 124 is immutable and still references the pre-127 code.
    // Migration 127 canonicalizes this row to organization_work.create.own.
    "organization_work.create",
    codes::ORGANIZATION_WORK_UPDATE_OWN,
    codes::ORGANIZATION_WORK_APPROVE_ORGANIZATION_UNIT,
    codes::STAFF_PROFILE_READ_ORGANIZATION_TREE,
    codes::STAFF_PROFILE_READ_SCHOOL,
    codes::STAFF_PII_READ_SCHOOL,
];

fn migrator_through_version(max_version: i64) -> Migrator {
    let base = sqlx::migrate!("./migrations");
    Migrator {
        migrations: Cow::Owned(
            base.iter()
                .filter(|migration| migration.version <= max_version)
                .cloned()
                .collect(),
        ),
        ignore_missing: true,
        locking: false,
        no_tx: base.no_tx,
    }
}

fn all_migrations_without_db_lock() -> Migrator {
    let base = sqlx::migrate!("./migrations");
    Migrator {
        migrations: Cow::Owned(base.iter().cloned().collect()),
        ignore_missing: base.ignore_missing,
        locking: false,
        no_tx: base.no_tx,
    }
}

fn permission_def_for_code(code: &str) -> Option<&'static PermissionDef> {
    ALL_PERMISSIONS
        .iter()
        .find(|permission| permission.code == code)
}

fn permission_def_for_migration_code(code: &str) -> Option<&'static PermissionDef> {
    match code {
        "organization_work.create" => permission_def_for_code(codes::ORGANIZATION_WORK_CREATE_OWN),
        _ => permission_def_for_code(code),
    }
}

async fn sync_grant_baseline_prerequisite_permissions(pool: &PgPool) -> Result<(), String> {
    for code in GRANT_BASELINE_REQUIRED_PERMISSION_CODES {
        let permission = permission_def_for_migration_code(code)
            .ok_or_else(|| format!("Required migration permission is not registered: {code}"))?;

        sqlx::query(
            r#"
            INSERT INTO permissions (code, name, module, action, scope, description)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (code) DO UPDATE
            SET
                name = EXCLUDED.name,
                module = EXCLUDED.module,
                action = EXCLUDED.action,
                scope = EXCLUDED.scope,
                description = EXCLUDED.description,
                updated_at = NOW()
            "#,
        )
        .bind(*code)
        .bind(permission.name)
        .bind(permission.module)
        .bind(permission.action)
        .bind(permission.scope)
        .bind(permission.description)
        .execute(pool)
        .await
        .map_err(|error| {
            format!("Failed to sync prerequisite migration permission {code}: {error}")
        })?;
    }

    Ok(())
}

pub async fn run_tenant_migrations(pool: &PgPool) -> Result<(), String> {
    migrator_through_version(PERMISSION_SYNC_MIGRATION_CHECKPOINT_VERSION)
        .run(pool)
        .await
        .map_err(|error| {
            format!(
                "Migration checkpoint {} failed: {}",
                PERMISSION_SYNC_MIGRATION_CHECKPOINT_VERSION, error
            )
        })?;

    crate::utils::permission_sync::sync_permissions(pool)
        .await
        .map_err(|error| {
            format!(
                "Permission sync checkpoint {} failed: {}",
                PERMISSION_SYNC_MIGRATION_CHECKPOINT_VERSION, error
            )
        })?;

    sync_grant_baseline_prerequisite_permissions(pool).await?;

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
            println!("🔄 Running migrations for school: {}", subdomain);

            run_tenant_migrations(pool)
                .await
                .map_err(|e| format!("Migration failed for {}: {}", subdomain, e))?;

            println!("✅ Migrations completed for: {}", subdomain);
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
                println!("🔄 Syncing permissions for school: {}", subdomain);

                crate::utils::permission_sync::sync_permissions(pool)
                    .await
                    .map_err(|e| format!("Permission sync failed for {}: {}", subdomain, e))?;

                println!("✅ Permissions synced for: {}", subdomain);
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
    fn permission_sync_checkpoint_migrator_stops_before_grant_baseline() {
        let migrator = migrator_through_version(PERMISSION_SYNC_MIGRATION_CHECKPOINT_VERSION);
        let versions = migrator
            .iter()
            .map(|migration| migration.version)
            .collect::<Vec<_>>();

        assert!(versions.contains(&PERMISSION_SYNC_MIGRATION_CHECKPOINT_VERSION));
        assert!(!versions.contains(&124));
        assert!(!migrator.locking);
        assert!(versions
            .iter()
            .all(|version| *version <= PERMISSION_SYNC_MIGRATION_CHECKPOINT_VERSION));
    }

    #[test]
    fn full_migrator_disables_database_advisory_lock_for_pooler_safety() {
        let migrator = all_migrations_without_db_lock();

        assert!(!migrator.locking);
        assert!(migrator.version_exists(124));
    }

    #[test]
    fn grant_baseline_checkpoint_permissions_are_registered() {
        let mut seen = std::collections::HashSet::new();

        for code in GRANT_BASELINE_REQUIRED_PERMISSION_CODES {
            assert!(seen.insert(code));
            assert!(
                permission_def_for_migration_code(code).is_some(),
                "{code} must exist in the permission registry before migration 124"
            );
        }
    }

    #[test]
    fn grant_baseline_checkpoint_includes_organization_work_approval_dependency() {
        assert!(GRANT_BASELINE_REQUIRED_PERMISSION_CODES
            .contains(&&codes::ORGANIZATION_WORK_APPROVE_ORGANIZATION_UNIT));
    }

    #[test]
    fn grant_baseline_checkpoint_keeps_immutable_migration_124_code_until_canonicalized() {
        assert!(GRANT_BASELINE_REQUIRED_PERMISSION_CODES.contains(&&"organization_work.create"));
        assert_eq!(
            permission_def_for_migration_code("organization_work.create").map(|permission| {
                (
                    permission.code,
                    permission.module,
                    permission.action,
                    permission.scope,
                )
            }),
            Some((
                codes::ORGANIZATION_WORK_CREATE_OWN,
                "organization_work",
                "create",
                "own",
            ))
        );
    }
}
