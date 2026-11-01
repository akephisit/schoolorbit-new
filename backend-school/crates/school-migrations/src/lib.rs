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
mod academic149;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationStage {
    Identity,
    Execution,
    PermissionSync,
    RuntimeGrants,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MigrationErrorClass {
    Database,
    RowNotFound,
    PoolTimeout,
    PoolClosed,
    Connection,
    Decode,
    Encode,
    Protocol,
    Configuration,
    Other,
    Source,
    VersionMissing,
    VersionMismatch,
    VersionNotPresent,
    VersionTooOld,
    VersionTooNew,
    Dirty,
    MigrationConfiguration,
}
impl MigrationErrorClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Database => "database",
            Self::RowNotFound => "row_not_found",
            Self::PoolTimeout => "pool_timeout",
            Self::PoolClosed => "pool_closed",
            Self::Connection => "connection",
            Self::Decode => "decode",
            Self::Encode => "encode",
            Self::Protocol => "protocol",
            Self::Configuration => "configuration",
            Self::Other => "other",
            Self::Source => "source",
            Self::VersionMissing => "version_missing",
            Self::VersionMismatch => "version_mismatch",
            Self::VersionNotPresent => "version_not_present",
            Self::VersionTooOld => "version_too_old",
            Self::VersionTooNew => "version_too_new",
            Self::Dirty => "dirty",
            Self::MigrationConfiguration => "migration_configuration",
        }
    }
    pub fn from_sqlx(error: &sqlx::Error) -> Self {
        match school_database::error_class(error) {
            "database" => Self::Database,
            "row_not_found" => Self::RowNotFound,
            "pool_timeout" => Self::PoolTimeout,
            "pool_closed" => Self::PoolClosed,
            "connection" => Self::Connection,
            "decode" => Self::Decode,
            "encode" => Self::Encode,
            "protocol" => Self::Protocol,
            "configuration" => Self::Configuration,
            _ => Self::Other,
        }
    }
}

/// A native SQLSTATE only; database messages, details and SQL never leave this boundary.
pub fn diagnostic_sqlstate(error: &sqlx::Error) -> Option<String> {
    let code = error.as_database_error()?.code()?;
    validated_sqlstate(&code)
}
pub fn validated_sqlstate(code: &str) -> Option<String> {
    (code.len() == 5
        && code
            .bytes()
            .all(|byte| byte.is_ascii_uppercase() || byte.is_ascii_digit()))
    .then(|| code.to_string())
}

pub fn diagnostic_version(version: i64) -> Option<i64> {
    (1..=9_007_199_254_740_991)
        .contains(&version)
        .then_some(version)
}

fn refusal_code(version: Option<i64>, error: &sqlx::Error) -> &'static str {
    let Some(database) = error.as_database_error() else {
        return "unclassified";
    };
    match (version, database.message()) {
        (Some(151), "PUBLICATION_151_INITIAL_SOURCE_UNMAPPABLE") => {
            "PUBLICATION_151_INITIAL_SOURCE_UNMAPPABLE"
        }
        (Some(151), "PUBLICATION_151_INITIAL_TEACHERS_UNMAPPABLE") => {
            "PUBLICATION_151_INITIAL_TEACHERS_UNMAPPABLE"
        }
        (Some(151), "PUBLICATION_151_SOURCE_LINEAGE_INVALID") => {
            "PUBLICATION_151_SOURCE_LINEAGE_INVALID"
        }
        (Some(151), "PUBLICATION_151_PRESERVATION_FAILED") => "PUBLICATION_151_PRESERVATION_FAILED",
        (Some(151), "PUBLICATION_151_ASSIGNMENT_CONTEXT_INVALID") => {
            "PUBLICATION_151_ASSIGNMENT_CONTEXT_INVALID"
        }
        (Some(151), "PUBLICATION_151_RECONCILIATION_FAILED") => {
            "PUBLICATION_151_RECONCILIATION_FAILED"
        }
        (Some(147), "ACADEMIC_CONTEXT_147_RUNNING_CONTEXT_MISMATCH") => {
            "ACADEMIC_CONTEXT_147_RUNNING_CONTEXT_MISMATCH"
        }
        (Some(147), "ACADEMIC_CONTEXT_DEFAULT_MUST_BE_VISIBLE") => {
            "ACADEMIC_CONTEXT_DEFAULT_MUST_BE_VISIBLE"
        }
        (Some(148), "ACADEMIC_CONTEXT_148_UNKNOWN_DATE_DEPENDENCY") => {
            "ACADEMIC_CONTEXT_148_UNKNOWN_DATE_DEPENDENCY"
        }
        (Some(148), "ACADEMIC_CONTEXT_148_CALENDAR_YEAR_UNMAPPABLE") => {
            "ACADEMIC_CONTEXT_148_CALENDAR_YEAR_UNMAPPABLE"
        }
        (Some(148), "ACADEMIC_CONTEXT_148_CALENDAR_ROOM_YEAR_MISMATCH") => {
            "ACADEMIC_CONTEXT_148_CALENDAR_ROOM_YEAR_MISMATCH"
        }
        (Some(148), "ACADEMIC_CONTEXT_148_CALENDAR_MATERIALIZATION_TOO_LARGE") => {
            "ACADEMIC_CONTEXT_148_CALENDAR_MATERIALIZATION_TOO_LARGE"
        }
        (Some(148), "ACADEMIC_CONTEXT_148_TERM_RECEIPT_SOURCE_UNMAPPABLE") => {
            "ACADEMIC_CONTEXT_148_TERM_RECEIPT_SOURCE_UNMAPPABLE"
        }
        (Some(148), "ACADEMIC_CONTEXT_148_YEAR_ACTIVATION_RECEIPT_UNMAPPABLE") => {
            "ACADEMIC_CONTEXT_148_YEAR_ACTIVATION_RECEIPT_UNMAPPABLE"
        }
        (Some(148), "ACADEMIC_CONTEXT_148_PROMOTION_ITEM_INTENT_UNMAPPABLE") => {
            "ACADEMIC_CONTEXT_148_PROMOTION_ITEM_INTENT_UNMAPPABLE"
        }
        (Some(148), "ACADEMIC_CONTEXT_148_PROMOTION_RUN_SNAPSHOT_UNMAPPABLE") => {
            "ACADEMIC_CONTEXT_148_PROMOTION_RUN_SNAPSHOT_UNMAPPABLE"
        }
        (Some(148), "ACADEMIC_CONTEXT_148_PROMOTION_ITEM_SNAPSHOT_UNMAPPABLE") => {
            "ACADEMIC_CONTEXT_148_PROMOTION_ITEM_SNAPSHOT_UNMAPPABLE"
        }
        (Some(148), "ACADEMIC_CONTEXT_148_CURRICULUM_CREATED_INTENT_UNMAPPABLE") => {
            "ACADEMIC_CONTEXT_148_CURRICULUM_CREATED_INTENT_UNMAPPABLE"
        }
        (Some(148), "ACADEMIC_CONTEXT_148_PREPARATION_REQUEST_UNMAPPABLE") => {
            "ACADEMIC_CONTEXT_148_PREPARATION_REQUEST_UNMAPPABLE"
        }
        (Some(148), "ACADEMIC_CONTEXT_148_PREPARATION_CREATED_INTENT_UNMAPPABLE") => {
            "ACADEMIC_CONTEXT_148_PREPARATION_CREATED_INTENT_UNMAPPABLE"
        }
        (Some(148), "ACADEMIC_CONTEXT_148_RECONCILIATION_FAILED") => {
            "ACADEMIC_CONTEXT_148_RECONCILIATION_FAILED"
        }
        (Some(148), "ACADEMIC_CONTEXT_148_POST_CLEANUP_RECONCILIATION_FAILED") => {
            "ACADEMIC_CONTEXT_148_POST_CLEANUP_RECONCILIATION_FAILED"
        }
        (Some(148), "PROMOTION_RUN_HISTORY_RETAINED") => "PROMOTION_RUN_HISTORY_RETAINED",
        (Some(148), "PROMOTION_RUN_IDENTITY_IMMUTABLE") => "PROMOTION_RUN_IDENTITY_IMMUTABLE",
        (Some(149), "ACADEMIC_CONTEXT_149_UNSUPPORTED_RECEIPT_STATE") => {
            "ACADEMIC_CONTEXT_149_UNSUPPORTED_RECEIPT_STATE"
        }
        (Some(149), "ACADEMIC_CONTEXT_149_UNKNOWN_RECEIPT_DEPENDENCY") => {
            "ACADEMIC_CONTEXT_149_UNKNOWN_RECEIPT_DEPENDENCY"
        }
        (Some(149), "ACADEMIC_CONTEXT_149_RECONCILIATION_FAILED") => {
            "ACADEMIC_CONTEXT_149_RECONCILIATION_FAILED"
        }
        (Some(149), "ACADEMIC_CONTEXT_149_PHASE_EVIDENCE_INVALID") => {
            "ACADEMIC_CONTEXT_149_PHASE_EVIDENCE_INVALID"
        }
        _ => "unclassified",
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MigrationDiagnostic {
    pub stage: MigrationStage,
    pub version: Option<i64>,
    pub error_class: MigrationErrorClass,
    pub sqlstate: Option<String>,
    pub code: &'static str,
}
#[derive(Debug)]
pub struct MigrationFailure {
    pub diagnostic: MigrationDiagnostic,
    legacy_message: String,
}
impl MigrationFailure {
    pub fn into_legacy_message(self) -> String {
        self.legacy_message
    }
    fn sqlx(stage: MigrationStage, error: &sqlx::Error, legacy_message: String) -> Self {
        Self {
            diagnostic: MigrationDiagnostic {
                stage,
                version: None,
                error_class: MigrationErrorClass::from_sqlx(error),
                sqlstate: diagnostic_sqlstate(error),
                code: "unclassified",
            },
            legacy_message,
        }
    }
    fn identity(legacy_message: String) -> Self {
        Self {
            diagnostic: MigrationDiagnostic {
                stage: MigrationStage::Identity,
                version: None,
                error_class: MigrationErrorClass::Configuration,
                sqlstate: None,
                code: "unclassified",
            },
            legacy_message,
        }
    }
}

// Classify before erasing the native error: driver details can contain SQL and values.
fn migration_failure(error: sqlx::migrate::MigrateError) -> MigrationFailure {
    use sqlx::migrate::MigrateError;
    use MigrationErrorClass as Class;
    let (category, version) = match &error {
        MigrateError::Execute(error) => (Class::from_sqlx(error), None),
        MigrateError::ExecuteMigration(error, version) => (Class::from_sqlx(error), Some(*version)),
        MigrateError::Source(_) => (Class::Source, None),
        MigrateError::VersionMissing(version) => (Class::VersionMissing, Some(*version)),
        MigrateError::VersionMismatch(version) => (Class::VersionMismatch, Some(*version)),
        MigrateError::VersionNotPresent(version) => (Class::VersionNotPresent, Some(*version)),
        MigrateError::VersionTooOld(version, _) => (Class::VersionTooOld, Some(*version)),
        MigrateError::VersionTooNew(version, _) => (Class::VersionTooNew, Some(*version)),
        MigrateError::Dirty(version) => (Class::Dirty, Some(*version)),
        _ => (Class::MigrationConfiguration, None),
    };
    let legacy_message = match version {
        Some(version) => format!(
            "Migration failed: {} (version {version})",
            category.as_str()
        ),
        None => format!("Migration failed: {}", category.as_str()),
    };
    let (sqlstate, code) = match &error {
        MigrateError::Execute(error) | MigrateError::ExecuteMigration(error, _) => {
            (diagnostic_sqlstate(error), refusal_code(version, error))
        }
        _ => (None, "unclassified"),
    };
    MigrationFailure {
        diagnostic: MigrationDiagnostic {
            stage: MigrationStage::Execution,
            version: version.and_then(diagnostic_version),
            error_class: category,
            sqlstate,
            code,
        },
        legacy_message,
    }
}

pub async fn run_tenant_migrations(pool: &PgPool) -> Result<(), String> {
    run_tenant_migrations_detailed(pool)
        .await
        .map_err(MigrationFailure::into_legacy_message)
}

/// The canonical runner with bounded diagnostics for release orchestration.
pub async fn run_tenant_migrations_detailed(pool: &PgPool) -> Result<(), MigrationFailure> {
    let started = std::time::Instant::now();
    let owner: String = sqlx::query_scalar("SELECT current_user::text")
        .fetch_one(pool)
        .await
        .map_err(|error| {
            MigrationFailure::sqlx(
                MigrationStage::Identity,
                &error,
                "Migration identity unavailable".into(),
            )
        })?;
    if owner.starts_with("so_") && owner.ends_with("_runtime") {
        return Err(MigrationFailure::identity(
            "Runtime identities cannot migrate".into(),
        ));
    }
    let runtime = runtime_role_for_owner(&owner).map_err(MigrationFailure::identity)?;
    // The deployment/provision endpoints share one process. Serialize real tenant
    // owner jobs without serializing unrelated disposable test schemas.
    let _guard = if runtime.is_some() {
        Some(TENANT_MIGRATION_GATE.lock().await)
    } else {
        None
    };
    {
        let mut connection = pool
            .acquire()
            .await
            .map_err(|error| migration_failure(sqlx::migrate::MigrateError::Execute(error)))?;
        academic149::run_transition(&mut connection, &all_migrations_without_db_lock())
            .await
            .map_err(migration_failure)?;
        // End the owned lease before pool-based permission/grant work (including pool size one).
        drop(connection);
    }

    sync_permissions(pool).await.map_err(|error| {
        MigrationFailure::sqlx(
            MigrationStage::PermissionSync,
            &error,
            format!(
                "Permission sync failed after migrations: {}",
                school_database::error_class(&error)
            ),
        )
    })?;

    if let Some(runtime) = runtime {
        grant_runtime_privileges(pool, &runtime)
            .await
            .map_err(|error| {
                MigrationFailure::sqlx(
                    MigrationStage::RuntimeGrants,
                    &error,
                    "Runtime privilege reconciliation failed".into(),
                )
            })?;
    }

    tracing::info!(
        operation = "tenant_migration",
        duration_ms = started.elapsed().as_secs_f64() * 1000.0
    );

    Ok(())
}

/// Highest immutable migration compiled into this exact release.
pub fn latest_migration_version() -> i64 {
    all_migrations_without_db_lock()
        .iter()
        .last()
        .map_or(0, |migration| migration.version)
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

async fn grant_runtime_privileges(pool: &PgPool, runtime: &str) -> Result<(), sqlx::Error> {
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
        .await?;
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
    #[test]
    fn canonical_runner_future_remains_send_for_http_consumers() {
        fn accept<F: std::future::Future + Send>(_: F) {}
        fn check(pool: &PgPool) {
            accept(run_tenant_migrations_detailed(pool));
        }
        let _ = check as fn(&PgPool);
    }
    #[test]
    fn recovery_refusal_codes_require_native149_and_legacy_connection_errors_are_preserved() {
        for code in [
            "ACADEMIC_CONTEXT_149_UNSUPPORTED_RECEIPT_STATE",
            "ACADEMIC_CONTEXT_149_UNKNOWN_RECEIPT_DEPENDENCY",
            "ACADEMIC_CONTEXT_149_RECONCILIATION_FAILED",
            "ACADEMIC_CONTEXT_149_PHASE_EVIDENCE_INVALID",
        ] {
            let failure = migration_failure(sqlx::migrate::MigrateError::ExecuteMigration(
                native(code, "P0001"),
                149,
            ));
            assert_eq!(failure.diagnostic.code, code);
            let wrong = migration_failure(sqlx::migrate::MigrateError::ExecuteMigration(
                native(code, "P0001"),
                148,
            ));
            assert_eq!(wrong.diagnostic.code, "unclassified");
            let spoofed = migration_failure(sqlx::migrate::MigrateError::ExecuteMigration(
                native(&format!("{code} private-sentinel"), "P0001"),
                149,
            ));
            assert_eq!(spoofed.diagnostic.code, "unclassified");
            assert!(!format!("{spoofed:?}").contains("private-sentinel"));
        }
        let connection = migration_failure(sqlx::migrate::MigrateError::Execute(
            sqlx::Error::PoolTimedOut,
        ));
        assert_eq!(connection.diagnostic.stage, MigrationStage::Execution);
        assert_eq!(
            connection.into_legacy_message(),
            "Migration failed: pool_timeout"
        );
    }
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    use tokio::time::{sleep, Duration};

    #[derive(Debug)]
    struct NativeError {
        message: String,
        code: String,
    }
    impl std::fmt::Display for NativeError {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str(&self.message)
        }
    }
    impl std::error::Error for NativeError {}
    impl sqlx::error::DatabaseError for NativeError {
        fn message(&self) -> &str {
            &self.message
        }
        fn code(&self) -> Option<std::borrow::Cow<'_, str>> {
            Some(std::borrow::Cow::Borrowed(&self.code))
        }
        fn as_error(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn as_error_mut(&mut self) -> &mut (dyn std::error::Error + Send + Sync + 'static) {
            self
        }
        fn into_error(self: Box<Self>) -> Box<dyn std::error::Error + Send + Sync + 'static> {
            self
        }
        fn kind(&self) -> sqlx::error::ErrorKind {
            sqlx::error::ErrorKind::Other
        }
    }
    fn native(message: &str, code: &str) -> sqlx::Error {
        sqlx::Error::Database(Box::new(NativeError {
            message: message.into(),
            code: code.into(),
        }))
    }

    #[test]
    fn migration_diagnostics_require_exact_native_refusal_and_matching_version() {
        let code = "ACADEMIC_CONTEXT_148_UNKNOWN_DATE_DEPENDENCY";
        for (version, message, expected) in [
            (148, code.to_string(), code),
            (147, code.to_string(), "unclassified"),
            (149, code.to_string(), "unclassified"),
            (148, format!("prefix {code}"), "unclassified"),
            (
                148,
                format!("{code} postgres://private-sentinel"),
                "unclassified",
            ),
            (148, format!("{code}\n"), "unclassified"),
            (
                147,
                "ACADEMIC_CONTEXT_147_RUNNING_CONTEXT_MISMATCH".into(),
                "ACADEMIC_CONTEXT_147_RUNNING_CONTEXT_MISMATCH",
            ),
        ] {
            let failure = migration_failure(sqlx::migrate::MigrateError::ExecuteMigration(
                native(&message, "P0001"),
                version,
            ));
            assert_eq!(failure.diagnostic.stage, MigrationStage::Execution);
            assert_eq!(failure.diagnostic.version, Some(version));
            assert_eq!(
                failure.diagnostic.error_class,
                MigrationErrorClass::Database
            );
            assert_eq!(failure.diagnostic.sqlstate.as_deref(), Some("P0001"));
            assert_eq!(failure.diagnostic.code, expected);
            assert!(!format!("{failure:?}").contains("private-sentinel"));
            assert_eq!(
                failure.into_legacy_message(),
                format!("Migration failed: database (version {version})")
            );
        }
        let failure =
            migration_failure(sqlx::migrate::MigrateError::Execute(native(code, "P0001")));
        assert_eq!(failure.diagnostic.code, "unclassified");
        assert_eq!(failure.diagnostic.version, None);
    }

    #[test]
    fn migration_diagnostics_bound_sqlstate_without_erasing_future_versions() {
        for (code, expected) in [
            ("42501", Some("42501")),
            ("ABC12", Some("ABC12")),
            ("42p01", None),
            ("P0001\nsecret", None),
            ("secret-url", None),
        ] {
            let failure = migration_failure(sqlx::migrate::MigrateError::ExecuteMigration(
                native("private-sentinel postgres://credentials", code),
                148,
            ));
            assert_eq!(failure.diagnostic.sqlstate.as_deref(), expected);
            assert_eq!(failure.diagnostic.code, "unclassified");
            assert!(!format!("{failure:?}").contains("private-sentinel"));
            assert!(!failure.into_legacy_message().contains("credentials"));
        }
        let future = latest_migration_version() + 1;
        let failure = migration_failure(sqlx::migrate::MigrateError::VersionMissing(future));
        assert_eq!(failure.diagnostic.version, Some(future));
        assert_eq!(
            failure.diagnostic.error_class,
            MigrationErrorClass::VersionMissing
        );
        assert_eq!(
            failure.into_legacy_message(),
            format!("Migration failed: version_missing (version {future})")
        );
        for version in [0, -1, i64::MAX] {
            let failure = migration_failure(sqlx::migrate::MigrateError::VersionMissing(version));
            assert_eq!(failure.diagnostic.version, None);
            assert_eq!(
                failure.into_legacy_message(),
                format!("Migration failed: version_missing (version {version})")
            );
        }
        assert_eq!(
            diagnostic_version(9_007_199_254_740_991),
            Some(9_007_199_254_740_991)
        );
        assert_eq!(diagnostic_version(9_007_199_254_740_992), None);
    }

    #[test]
    fn post_migration_native_errors_keep_stage_and_legacy_adapter_message() {
        let failure = MigrationFailure::sqlx(
            MigrationStage::PermissionSync,
            &native("ACADEMIC_CONTEXT_148_UNKNOWN_DATE_DEPENDENCY", "42501"),
            "Permission sync failed after migrations: database".into(),
        );
        assert_eq!(failure.diagnostic.stage, MigrationStage::PermissionSync);
        assert_eq!(failure.diagnostic.sqlstate.as_deref(), Some("42501"));
        assert_eq!(failure.diagnostic.code, "unclassified");
        assert_eq!(
            failure.into_legacy_message(),
            "Permission sync failed after migrations: database"
        );
        let failure = MigrationFailure::sqlx(
            MigrationStage::RuntimeGrants,
            &native("private-sentinel", "42501"),
            "Runtime privilege reconciliation failed".into(),
        );
        assert_eq!(failure.diagnostic.stage, MigrationStage::RuntimeGrants);
        assert_eq!(
            failure.into_legacy_message(),
            "Runtime privilege reconciliation failed"
        );
        assert_eq!(
            MigrationFailure::identity("Runtime identities cannot migrate".into())
                .into_legacy_message(),
            "Runtime identities cannot migrate"
        );
    }

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
