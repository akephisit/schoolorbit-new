use crate::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use school_academic_core::reconciliation::{
    read_academic_core_cleanup_audit, ReconciliationCheck, PHASE_B_MIGRATION_VERSION,
};
use school_academic_results::services::{
    read_gradebook_results_cutover_audit, GRADEBOOK_RESULTS_MIGRATION_VERSION,
};
use school_http::HttpError as AppError;
use serde::Serialize;
use sqlx::PgPool;

#[derive(Serialize)]
struct MigrationResult {
    subdomain: String,
    status: String,
    version: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct MigrateAllResponse {
    total: usize,
    success: usize,
    failed: usize,
    latest_version: i64,
    results: Vec<MigrationResult>,
}

#[derive(Serialize)]
struct MigrationStatusResponse {
    total_schools: usize,
    migrated: usize,
    pending: usize,
    failed: usize,
    outdated: usize,
    active_pools: usize,
    latest_version: i64,
    schools: Vec<SchoolMigrationStatus>,
}

#[derive(Serialize)]
struct SchoolMigrationStatus {
    subdomain: String,
    migration_version: i32,
    migration_status: String,
    last_migrated_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    migration_error: Option<String>,
    #[serde(rename = "academicCoreCutover")]
    academic_core_cutover: AcademicCoreCutoverStatus,
    #[serde(rename = "academicContextCutover")]
    academic_context_cutover: AcademicContextCutoverStatus,
    #[serde(rename = "gradebookResultsCutover")]
    gradebook_results_cutover: GradebookResultsCutoverStatus,
    #[serde(rename = "deliveryTimetableCutover")]
    delivery_timetable_cutover: DeliveryTimetableCutoverStatus,
    #[serde(rename = "staffProfileCutover")]
    staff_profile_cutover: school_staff::profile_cutover::ProfileCutoverStatus,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AcademicContextCutoverStatus {
    status: String,
    migration_version: i64,
    passed: Option<bool>,
    checks: Vec<ReconciliationCheck>,
}
async fn academic_context_cutover_status(
    pool: Option<&PgPool>,
    version: i32,
) -> AcademicContextCutoverStatus {
    if version < 148 {
        return AcademicContextCutoverStatus {
            status: "pendingMigration".into(),
            migration_version: 148,
            passed: None,
            checks: vec![],
        };
    }
    if let Some(pool) = pool {
        if let Ok(audit) =
            school_academic_core::context_reconciliation::read_academic_context_cutover_audit(pool)
                .await
        {
            return AcademicContextCutoverStatus {
                status: if audit.completed {
                    "cutoverCompleted"
                } else {
                    "cutoverFailed"
                }
                .into(),
                migration_version: 148,
                passed: Some(audit.completed),
                checks: audit.checks,
            };
        }
    }
    AcademicContextCutoverStatus {
        status: "unavailable".into(),
        migration_version: 148,
        passed: Some(false),
        checks: vec![],
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AcademicCoreCutoverStatus {
    status: String,
    migration_version: i64,
    passed: Option<bool>,
    checks: Vec<ReconciliationCheck>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GradebookResultsCutoverStatus {
    status: String,
    migration_version: i64,
    passed: Option<bool>,
    checks: Vec<ReconciliationCheck>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeliveryTimetableCutoverStatus {
    status: String,
    migration_version: i64,
    passed: Option<bool>,
    checks: Vec<school_academic_delivery::services::versions::DeliveryCutoverCheck>,
}
async fn staff_profile_cutover_status(
    pool: Option<&PgPool>,
    version: i32,
) -> school_staff::profile_cutover::ProfileCutoverStatus {
    if version >= 114 {
        if let Some(pool) = pool {
            match school_staff::profile_cutover::read_profile_cutover(pool).await {
                Ok(status) => return status,
                Err(error) => {
                    tracing::warn!(reason="staff_profile_cutover_audit_query_failed",database_code=?error.database_code())
                }
            }
        }
    }
    school_staff::profile_cutover::ProfileCutoverStatus::unavailable(version)
}

fn delivery_timetable_cutover_unavailable() -> DeliveryTimetableCutoverStatus {
    DeliveryTimetableCutoverStatus {
        status: "failed".into(),
        migration_version: 88,
        passed: Some(false),
        checks: Vec::new(),
    }
}
async fn delivery_timetable_cutover_status(
    pool: Option<&PgPool>,
    version: i32,
) -> DeliveryTimetableCutoverStatus {
    if version < 88 {
        return DeliveryTimetableCutoverStatus {
            status: "cutoverPending".into(),
            migration_version: 88,
            passed: None,
            checks: Vec::new(),
        };
    }
    let Some(pool) = pool else {
        return delivery_timetable_cutover_unavailable();
    };
    match school_academic_delivery::services::versions::read_cutover_audit(pool).await {
        Ok(audit) => DeliveryTimetableCutoverStatus {
            status: if audit.completed {
                "cutoverCompleted"
            } else {
                "failed"
            }
            .into(),
            migration_version: 88,
            passed: Some(audit.completed),
            checks: audit.checks,
        },
        Err(error) => {
            tracing::warn!(reason="delivery_timetable_cutover_audit_query_failed",database_code=?error.database_code());
            delivery_timetable_cutover_unavailable()
        }
    }
}

fn gradebook_results_cutover_unavailable(current_version: i32) -> GradebookResultsCutoverStatus {
    GradebookResultsCutoverStatus {
        status: "failed".to_string(),
        migration_version: GRADEBOOK_RESULTS_MIGRATION_VERSION,
        passed: Some(false),
        checks: vec![ReconciliationCheck {
            code: "GRADEBOOK_RESULTS_CUTOVER_AUDIT_UNAVAILABLE".to_string(),
            passed: false,
            source_count: GRADEBOOK_RESULTS_MIGRATION_VERSION,
            target_count: i64::from(current_version),
        }],
    }
}

async fn gradebook_results_cutover_status(
    pool: Option<&PgPool>,
    current_version: i32,
) -> GradebookResultsCutoverStatus {
    if current_version < GRADEBOOK_RESULTS_MIGRATION_VERSION as i32 {
        return GradebookResultsCutoverStatus {
            status: "cutoverPending".to_string(),
            migration_version: GRADEBOOK_RESULTS_MIGRATION_VERSION,
            passed: None,
            checks: Vec::new(),
        };
    }

    let Some(pool) = pool else {
        return gradebook_results_cutover_unavailable(current_version);
    };

    match read_gradebook_results_cutover_audit(pool).await {
        Ok(audit) => GradebookResultsCutoverStatus {
            status: if audit.completed {
                "cutoverCompleted".to_string()
            } else {
                "failed".to_string()
            },
            migration_version: GRADEBOOK_RESULTS_MIGRATION_VERSION,
            passed: Some(audit.completed),
            checks: audit.checks,
        },
        Err(error) => {
            tracing::warn!(
                reason = "gradebook_results_cutover_audit_query_failed",
                database_code = ?error.database_code()
            );
            gradebook_results_cutover_unavailable(current_version)
        }
    }
}

fn academic_core_cutover_unavailable(current_version: i32) -> AcademicCoreCutoverStatus {
    AcademicCoreCutoverStatus {
        status: "failed".to_string(),
        migration_version: PHASE_B_MIGRATION_VERSION,
        passed: Some(false),
        checks: vec![ReconciliationCheck {
            code: "ACADEMIC_CORE_CLEANUP_AUDIT_UNAVAILABLE".to_string(),
            passed: false,
            source_count: PHASE_B_MIGRATION_VERSION,
            target_count: i64::from(current_version),
        }],
    }
}

async fn academic_core_cutover_status(
    pool: Option<&PgPool>,
    current_version: i32,
) -> AcademicCoreCutoverStatus {
    if current_version < 43 {
        return AcademicCoreCutoverStatus {
            status: "notApplicable".to_string(),
            migration_version: PHASE_B_MIGRATION_VERSION,
            passed: None,
            checks: Vec::new(),
        };
    }

    if current_version < PHASE_B_MIGRATION_VERSION as i32 {
        return AcademicCoreCutoverStatus {
            status: "cleanupPending".to_string(),
            migration_version: PHASE_B_MIGRATION_VERSION,
            passed: None,
            checks: Vec::new(),
        };
    }

    let Some(pool) = pool else {
        return academic_core_cutover_unavailable(current_version);
    };

    match read_academic_core_cleanup_audit(pool).await {
        Ok(audit) => AcademicCoreCutoverStatus {
            status: if audit.completed {
                "cleanupCompleted".to_string()
            } else {
                "failed".to_string()
            },
            migration_version: audit.migration_version,
            passed: Some(audit.completed),
            checks: audit.checks,
        },
        Err(error) => {
            tracing::warn!(
                reason = "academic_core_cleanup_audit_query_failed",
                database_code = ?error.database_code()
            );
            AcademicCoreCutoverStatus {
                status: "failed".to_string(),
                migration_version: PHASE_B_MIGRATION_VERSION,
                passed: Some(false),
                checks: vec![ReconciliationCheck {
                    code: "ACADEMIC_CORE_CLEANUP_AUDIT_UNAVAILABLE".to_string(),
                    passed: false,
                    source_count: PHASE_B_MIGRATION_VERSION,
                    target_count: i64::from(current_version),
                }],
            }
        }
    }
}

async fn academic_core_cutover_status_from_database(
    pool: &PgPool,
    reported_version: i32,
) -> (i32, AcademicCoreCutoverStatus) {
    match get_current_version(pool).await {
        Ok(actual_version) => {
            let actual_version = i32::try_from(actual_version).unwrap_or(i32::MAX);
            (
                actual_version,
                academic_core_cutover_status(Some(pool), actual_version).await,
            )
        }
        Err(error) => {
            tracing::warn!(
                reason = "academic_core_actual_version_unavailable",
                reported_version,
                error = %error
            );
            (
                reported_version,
                academic_core_cutover_unavailable(reported_version),
            )
        }
    }
}

/// Migrate all active schools
pub async fn migrate_all_schools(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    tracing::info!("🔄 Starting migration for all active schools...");

    let latest_version = get_latest_migration_version().await.map_err(|e| {
        AppError::InternalServerError(format!(
            "Failed to determine latest migration version: {}",
            e
        ))
    })?;

    tracing::info!("📊 Latest migration version: {}", latest_version);

    let schools = state
        .admin_client
        .list_active_schools()
        .await
        .map_err(|e| {
            tracing::error!("❌ Failed to fetch schools: {}", e);
            AppError::InternalServerError("Failed to fetch schools from admin service".to_string())
        })?;

    tracing::info!("📊 Found {} active schools", schools.len());

    let mut results = Vec::with_capacity(schools.len());
    for school in schools {
        let subdomain = school.subdomain.clone();
        let db_url = match state
            .admin_client
            .get_migration_database_url(&subdomain)
            .await
        {
            Ok(url) => url,
            Err(_) => {
                results.push(MigrationResult {
                    subdomain,
                    status: "failed".into(),
                    version: None,
                    error: Some("Migration connection unavailable".into()),
                });
                continue;
            }
        };

        results.push(migrate_single_school(&state, &subdomain, &db_url, latest_version).await);
    }

    let success_count = results
        .iter()
        .filter(|r| r.status == "migrated" || r.status == "already_migrated")
        .count();
    let failed_count = results.iter().filter(|r| r.status == "failed").count();

    tracing::info!(
        "✅ Migration complete: {} success, {} failed",
        success_count,
        failed_count
    );

    Ok((
        StatusCode::OK,
        Json(MigrateAllResponse {
            total: results.len(),
            success: success_count,
            failed: failed_count,
            latest_version,
            results,
        }),
    ))
}

/// Get migration status for all schools
pub async fn migration_status(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let latest_version = get_latest_migration_version().await.unwrap_or(0);

    let schools = state
        .admin_client
        .list_active_schools()
        .await
        .map_err(|e| {
            tracing::error!("❌ Failed to fetch schools: {}", e);
            AppError::InternalServerError("Failed to fetch migration status".to_string())
        })?;

    let total = schools.len();
    let mut migrated = 0;
    let mut pending = 0;
    let mut failed = 0;
    let mut outdated = 0;

    let mut school_statuses = Vec::with_capacity(total);
    for school in schools {
        let reported_version = school.migration_version.unwrap_or(0);
        let status = school
            .migration_status
            .unwrap_or_else(|| "pending".to_string());

        let (
            version,
            academic_core_cutover,
            academic_context_cutover,
            gradebook_results_cutover,
            delivery_timetable_cutover,
            staff_profile_cutover,
        ) = if let Some(database_url) = school
            .db_connection_string
            .as_deref()
            .filter(|value| !value.is_empty())
        {
            match state
                .pool_manager
                .get_pool_for_read_only_status(database_url, &school.subdomain)
                .await
            {
                Ok(pool) => {
                    let (version, academic_core_cutover) =
                        academic_core_cutover_status_from_database(&pool, reported_version).await;
                    let gradebook_results_cutover =
                        gradebook_results_cutover_status(Some(&pool), version).await;
                    (
                        version,
                        academic_core_cutover,
                        academic_context_cutover_status(Some(&pool), version).await,
                        gradebook_results_cutover,
                        delivery_timetable_cutover_status(Some(&pool), version).await,
                        staff_profile_cutover_status(Some(&pool), version).await,
                    )
                }
                Err(_) => (
                    reported_version,
                    academic_core_cutover_unavailable(reported_version),
                    academic_context_cutover_status(None, reported_version).await,
                    gradebook_results_cutover_unavailable(reported_version),
                    delivery_timetable_cutover_unavailable(),
                    staff_profile_cutover_status(None, reported_version).await,
                ),
            }
        } else {
            (
                reported_version,
                academic_core_cutover_status(None, reported_version).await,
                academic_context_cutover_status(None, reported_version).await,
                gradebook_results_cutover_status(None, reported_version).await,
                delivery_timetable_cutover_status(None, reported_version).await,
                staff_profile_cutover_status(None, reported_version).await,
            )
        };

        match status.as_str() {
            "migrated" => {
                if version < latest_version as i32 {
                    outdated += 1;
                } else {
                    migrated += 1;
                }
            }
            "failed" => failed += 1,
            _ => pending += 1,
        }

        school_statuses.push(SchoolMigrationStatus {
            subdomain: school.subdomain,
            migration_version: version,
            migration_status: if version < latest_version as i32 && status == "migrated" {
                "outdated".to_string()
            } else {
                status
            },
            last_migrated_at: school.last_migrated_at,
            migration_error: school.migration_error,
            academic_core_cutover,
            academic_context_cutover,
            gradebook_results_cutover,
            delivery_timetable_cutover,
            staff_profile_cutover,
        });
    }

    let active_pools = state.pool_manager.pool_count().await;

    Ok((
        StatusCode::OK,
        Json(MigrationStatusResponse {
            total_schools: total,
            migrated,
            pending,
            failed,
            outdated,
            active_pools,
            latest_version,
            schools: school_statuses,
        }),
    ))
}

/// Helper: Migrate a single school
async fn migrate_single_school(
    state: &AppState,
    subdomain: &str,
    db_url: &str,
    latest_version: i64,
) -> MigrationResult {
    tracing::info!("🔄 Migrating school: {}", subdomain);

    let migration = async {
        let owner_pool = school_tenancy::connect_privileged_pool(db_url, 1).await?;
        let result = school_migrations::run_tenant_migrations(&owner_pool).await;
        // Migrations/permission sync may have committed even if a later grant failed.
        // Invalidate before drainage or any other fallible post-migration work.
        state.invalidate_permission_tenant(subdomain);
        state.notify_all_permissions_changed(subdomain);
        let drainage = school_tenancy::close_pool(&owner_pool, "migration_owner_drain").await;
        result?;
        drainage?;
        let runtime = state
            .admin_client
            .get_school_database_info(subdomain)
            .await?;
        let version = verify_runtime_schema(&runtime.database_url).await?;
        state
            .pool_manager
            .retire_pool(&runtime.database_url, subdomain)
            .await?;
        let pool = state
            .pool_manager
            .get_pool_without_migrations(&runtime.database_url, subdomain)
            .await?;
        Ok::<_, String>((pool, version))
    }
    .await;
    let (pool, current_version) = match migration {
        Ok((pool, version)) => (pool, version),
        Err(error) => {
            let _ = state
                .admin_client
                .update_migration_status(subdomain, 0, "failed", Some(&error))
                .await;
            return MigrationResult {
                subdomain: subdomain.into(),
                status: "failed".into(),
                version: None,
                error: Some(error),
            };
        }
    };

    // Startup can precede tenant migration during a coordinated deployment.
    // Register attendance only after the new schema is available, without idle tenant polling.
    let registration = tokio::time::timeout(
        std::time::Duration::from_secs(30),
        school_attendance::runtime::touch(
            subdomain,
            pool.as_ref().clone(),
            state.notification_channel.clone(),
        ),
    )
    .await;
    if !matches!(registration, Ok(Ok(()))) {
        let error_code = if registration.is_err() {
            "attendance_schedule_registration_timeout"
        } else {
            "attendance_schedule_registration_failed"
        };
        tracing::warn!(
            operation = "attendance_schedule_registration",
            error_code,
            pool_connections = pool.size(),
            pool_idle_connections = pool.num_idle(),
            pool_closed = pool.is_closed(),
            "Attendance schedule registration failed"
        );
        if state
            .admin_client
            .update_migration_status(
                subdomain,
                current_version as i32,
                "failed",
                Some(error_code),
            )
            .await
            .is_err()
        {
            tracing::warn!(
                operation = "migration_status_update",
                error_code = "migration_status_update_failed"
            );
        }
        return MigrationResult {
            subdomain: subdomain.into(),
            status: "failed".into(),
            version: Some(current_version),
            error: Some(error_code.into()),
        };
    }

    match state
        .admin_client
        .update_migration_status(subdomain, current_version as i32, "migrated", None)
        .await
    {
        Ok(_) => {
            let status = if current_version == latest_version {
                "migrated"
            } else {
                "already_migrated"
            };
            tracing::info!("✅ {} migrated to version {}", subdomain, current_version);
            MigrationResult {
                subdomain: subdomain.to_string(),
                status: status.to_string(),
                version: Some(current_version),
                error: None,
            }
        }
        Err(e) => {
            tracing::warn!(
                "⚠️ Migration succeeded but failed to update admin service: {}",
                e
            );
            MigrationResult {
                subdomain: subdomain.to_string(),
                status: "migrated".to_string(),
                version: Some(current_version),
                error: Some(format!("Failed to update admin service: {}", e)),
            }
        }
    }
}

/// Get latest migration version from migrations directory
async fn get_latest_migration_version() -> Result<i64, String> {
    let migration_dir = std::path::Path::new("./migrations");

    if !migration_dir.exists() {
        return Ok(0);
    }

    let entries = std::fs::read_dir(migration_dir)
        .map_err(|e| format!("Failed to read migrations directory: {}", e))?;

    let mut max_version: i64 = 0;

    for entry in entries.flatten() {
        if let Some(filename) = entry.file_name().to_str() {
            if let Some(version_str) = filename.split('_').next() {
                if let Ok(version) = version_str.parse::<i64>() {
                    max_version = max_version.max(version);
                }
            }
        }
    }

    Ok(max_version)
}

/// A one-shot verifier owns one connection, not a second pool lifecycle.
async fn verify_runtime_schema(database_url: &str) -> Result<i64, String> {
    let started = std::time::Instant::now();
    let mut stage = "connect";
    let result = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        let mut connection = school_tenancy::connect_privileged_connection(database_url).await?;
        stage = "schema_history";
        let result = school_migrations::verify_tenant_schema(&mut *connection).await;
        stage = "connection_close";
        let closed = connection.close().await;
        let version = result?;
        closed.map_err(|_| "Runtime verification connection close failed".to_string())?;
        Ok(version)
    })
    .await;
    tracing::info!(
        operation = "runtime_schema_verification",
        stage,
        duration_ms = started.elapsed().as_secs_f64() * 1000.0,
        succeeded = matches!(&result, Ok(Ok(_)))
    );
    result.map_err(|_| {
        tracing::warn!(
            operation = "runtime_schema_verification",
            stage,
            error_code = "runtime_schema_verification_timeout"
        );
        "Runtime schema verification timed out".to_string()
    })?
}

/// Get current migration version from school database
async fn get_current_version(pool: &PgPool) -> Result<i64, String> {
    let version =
        sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
            .fetch_one(pool)
            .await
            .map_err(|e| {
                tracing::warn!(
                    operation = "tenant_schema_version",
                    pool_connections = pool.size(),
                    pool_idle_connections = pool.num_idle(),
                    pool_closed = pool.is_closed(),
                    "Tenant schema version read failed"
                );
                format!("Failed to get current version: {}", e)
            })?;

    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::integration_tests::academic::cutover_test_support::{
        apply_migrations_through, record_passing_phase_a_reconciliation_marker,
        seed_academic_cutover_fixture, seed_release_two_predecessor, CutoverFixture,
    };
    use school_test_db::create_named_test_pool;

    async fn verification_url(pool: &PgPool) -> String {
        let schema: String = sqlx::query_scalar("SELECT current_schema()::text")
            .fetch_one(pool)
            .await
            .unwrap();
        let mut url = reqwest::Url::parse(&std::env::var("TEST_DATABASE_URL").unwrap()).unwrap();
        url.query_pairs_mut()
            .append_pair("options", &format!("-csearch_path={schema},public"));
        url.to_string()
    }

    #[tokio::test]
    async fn deployment_runtime_verification_does_not_borrow_an_exhausted_request_pool() {
        let pool = create_named_test_pool("deployment_runtime_verifier").await;
        school_migrations::run_tenant_migrations(&pool)
            .await
            .unwrap();
        let url = verification_url(&pool).await;
        let requests = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&url)
            .await
            .unwrap();
        let held = requests.acquire().await.unwrap();
        assert!(requests.try_acquire().is_none());
        let version = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            verify_runtime_schema(&url),
        )
        .await
        .expect("deployment verification must not wait for a request connection")
        .unwrap();
        assert_eq!(version, get_latest_migration_version().await.unwrap());
        drop(held);
        requests.close().await;
    }

    #[tokio::test]
    async fn deployment_runtime_verification_rejects_incomplete_history() {
        let pool = phase_a_pool("deployment_runtime_verifier_old_schema").await;
        let result = verify_runtime_schema(&verification_url(&pool).await).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn deployment_verification_rejects_corrupt_history_without_repairing_it() {
        let pool = create_named_test_pool("runtime_verifier_corrupt_history").await;
        // This fixture exercises the immutable history contract, not business DDL.
        sqlx::query("CREATE TABLE _sqlx_migrations (version bigint PRIMARY KEY, success boolean NOT NULL, checksum bytea NOT NULL)")
            .execute(&pool).await.unwrap();
        for migration in sqlx::migrate!("./migrations").iter() {
            sqlx::query(
                "INSERT INTO _sqlx_migrations (version,success,checksum) VALUES ($1,true,$2)",
            )
            .bind(migration.version)
            .bind(migration.checksum.as_ref())
            .execute(&pool)
            .await
            .unwrap();
        }
        let url = verification_url(&pool).await;
        let version = verify_runtime_schema(&url).await.unwrap();
        sqlx::query("UPDATE _sqlx_migrations SET checksum = decode('00', 'hex') WHERE version=$1")
            .bind(version)
            .execute(&pool)
            .await
            .unwrap();
        assert!(verify_runtime_schema(&url).await.is_err());
        let checksum: Vec<u8> =
            sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version=$1")
                .bind(version)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(checksum, vec![0]);
    }

    #[tokio::test]
    async fn delivery_cutover_status_requires_both_actual_reconciliation_audits() {
        let pending = delivery_timetable_cutover_status(None, 87).await;
        assert_eq!(pending.status, "cutoverPending");
        assert_eq!(pending.passed, None);
        assert_eq!(
            delivery_timetable_cutover_status(None, 88).await.status,
            "failed"
        );
        let pool = create_named_test_pool("migration_status_delivery_cutover").await;
        seed_release_two_predecessor(&pool).await.unwrap();
        apply_migrations_through(&pool, 89).await.unwrap();
        let complete = delivery_timetable_cutover_status(Some(&pool), 88).await;
        assert_eq!(complete.status, "cutoverCompleted");
        assert_eq!(complete.passed, Some(true));
        assert_eq!(complete.checks.len(), 30);
        sqlx::query(
            "DELETE FROM academic_delivery_version_migration_audit WHERE migration_version=87",
        )
        .execute(&pool)
        .await
        .unwrap();
        let incomplete = delivery_timetable_cutover_status(Some(&pool), 88).await;
        assert_eq!(incomplete.status, "failed");
        assert_eq!(incomplete.passed, Some(false));
    }

    async fn phase_a_pool(name: &str) -> PgPool {
        let pool = create_named_test_pool(name).await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_migrations_through(&pool, 44).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn academic_core_status_is_not_applicable_before_migration_043() {
        let status = academic_core_cutover_status(None, 42).await;
        let value = serde_json::to_value(status).unwrap();

        assert_eq!(value["status"], "notApplicable");
        assert_eq!(value["migrationVersion"], 45);
        assert!(value["passed"].is_null());
        assert_eq!(value["checks"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn academic_core_status_reports_cleanup_pending_before_phase_b() {
        let pool = create_named_test_pool("migration_status_academic_core").await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_migrations_through(&pool, 43).await.unwrap();

        let status = academic_core_cutover_status(Some(&pool), 43).await;
        let value = serde_json::to_value(status).unwrap();
        assert_eq!(value["status"], "cleanupPending");
        assert_eq!(value["migrationVersion"], 45);
        assert!(value["passed"].is_null());
        assert_eq!(value["checks"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn academic_core_status_uses_the_database_version_over_stale_admin_metadata() {
        let pool = create_named_test_pool("migration_status_actual_version").await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_migrations_through(&pool, 43).await.unwrap();

        let (actual_version, status) = academic_core_cutover_status_from_database(&pool, 42).await;

        assert_eq!(actual_version, 43);
        assert_eq!(status.status, "cleanupPending");
        assert_eq!(status.passed, None);
    }

    #[tokio::test]
    async fn academic_core_status_reads_the_completed_phase_b_audit() {
        let pool = phase_a_pool("migration_status_phase_b_completed").await;
        record_passing_phase_a_reconciliation_marker(&pool)
            .await
            .unwrap();
        apply_migrations_through(&pool, 45).await.unwrap();

        let status = academic_core_cutover_status(Some(&pool), 45).await;
        let value = serde_json::to_value(status).unwrap();
        let encoded = serde_json::to_string(&value).unwrap();

        assert_eq!(value["status"], "cleanupCompleted");
        assert_eq!(value["migrationVersion"], 45);
        assert_eq!(value["passed"], true);
        assert_eq!(value["checks"].as_array().unwrap().len(), 2);
        assert!(!encoded.contains("entityMap"));
        assert!(!encoded.contains("sourceId"));
        assert!(!encoded.contains("targetId"));
    }

    #[tokio::test]
    async fn gradebook_results_status_is_pending_before_migration_060() {
        let status = gradebook_results_cutover_status(None, 59).await;
        let value = serde_json::to_value(status).unwrap();

        assert_eq!(value["status"], "cutoverPending");
        assert_eq!(value["migrationVersion"], 60);
        assert!(value["passed"].is_null());
        assert_eq!(value["checks"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn gradebook_results_status_reads_bounded_completed_audit() {
        let pool = create_named_test_pool("migration_status_gradebook_results").await;
        seed_release_two_predecessor(&pool).await.unwrap();
        apply_migrations_through(&pool, 60).await.unwrap();
        sqlx::query(
            "INSERT INTO permissions (code,name,module,action,scope,description,is_active)
             VALUES ('academic_result.future.school','Future result capability','academic_result',
                     'future','school','Future permission must not invalidate the cutover',true)",
        )
        .execute(&pool)
        .await
        .unwrap();

        let status = gradebook_results_cutover_status(Some(&pool), 60).await;
        let value = serde_json::to_value(status).unwrap();
        let encoded = serde_json::to_string(&value).unwrap();

        assert_eq!(value["status"], "cutoverCompleted");
        assert_eq!(value["migrationVersion"], 60);
        assert_eq!(value["passed"], true);
        assert!(value["checks"].as_array().unwrap().len() >= 6);
        assert!(value["checks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|check| check["passed"] == true));
        for forbidden in [
            "studentId",
            "studentAcademicYearId",
            "displayName",
            "score",
            "outcome",
        ] {
            assert!(!encoded.contains(forbidden));
        }
    }

    #[test]
    fn school_status_serializes_academic_core_cutover_in_camel_case() {
        let value = serde_json::to_value(SchoolMigrationStatus {
            academic_context_cutover: AcademicContextCutoverStatus {
                status: "pendingMigration".into(),
                migration_version: 148,
                passed: None,
                checks: vec![],
            },
            subdomain: "fixture".to_string(),
            migration_version: 45,
            migration_status: "migrated".to_string(),
            last_migrated_at: None,
            migration_error: None,
            academic_core_cutover: AcademicCoreCutoverStatus {
                status: "cleanupCompleted".to_string(),
                migration_version: 45,
                passed: Some(true),
                checks: Vec::new(),
            },
            delivery_timetable_cutover: delivery_timetable_cutover_unavailable(),
            staff_profile_cutover: school_staff::profile_cutover::ProfileCutoverStatus::unavailable(
                114,
            ),
            gradebook_results_cutover: GradebookResultsCutoverStatus {
                status: "cutoverCompleted".to_string(),
                migration_version: 60,
                passed: Some(true),
                checks: Vec::new(),
            },
        })
        .unwrap();

        assert!(value.get("academicCoreCutover").is_some());
        assert!(value.get("academic_core_cutover").is_none());
        assert!(value.get("gradebookResultsCutover").is_some());
        assert!(value.get("gradebook_results_cutover").is_none());
        assert!(value.get("personnelCutover").is_none());
        assert!(value.get("deliveryTimetableCutover").is_some());
    }

    #[test]
    fn reconcile_all_route_is_removed_after_phase_b() {
        let app_source = include_str!("../../app.rs");
        let internal_routes = &app_source[app_source.find("fn internal_routes").unwrap()
            ..app_source.find("fn protected_routes").unwrap()];
        assert!(!internal_routes.contains("/internal/academic-core/reconcile-all"));
        assert!(internal_routes
            .contains("route_layer(from_fn(middleware::internal_auth::validate_internal_secret))"));
    }
}
