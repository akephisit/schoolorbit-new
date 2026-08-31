use crate::error::AppError;
use crate::modules::academic::reconciliation::{
    read_academic_core_cleanup_audit, ReconciliationCheck, PHASE_B_MIGRATION_VERSION,
};
use crate::modules::academic::services::timetable_service::{self, LegacyCurrentTeacherConflict};
use crate::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
use serde::Serialize;
use sqlx::PgPool;
use std::future::Future;
use std::time::Duration;

#[derive(Serialize)]
struct MigrationResult {
    subdomain: String,
    status: String,
    version: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
    #[serde(
        rename = "academicDiagnostics",
        skip_serializing_if = "Option::is_none"
    )]
    academic_diagnostics: Option<AcademicMigrationDiagnostics>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AcademicMigrationDiagnostics {
    current_teacher_conflicts: Vec<LegacyCurrentTeacherConflict>,
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
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct AcademicCoreCutoverStatus {
    status: String,
    migration_version: i64,
    passed: Option<bool>,
    checks: Vec<ReconciliationCheck>,
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
                database_code = ?match &error {
                    AppError::DbError(sqlx::Error::Database(database_error)) => database_error.code(),
                    _ => None,
                }
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

    let mut results = Vec::new();

    for school in schools {
        let subdomain = school.subdomain.clone();
        let db_url = match school.db_connection_string {
            Some(ref url) if !url.is_empty() => url.clone(),
            _ => {
                let _ = state
                    .admin_client
                    .update_migration_status(
                        &subdomain,
                        0,
                        "failed",
                        Some("No database connection string"),
                    )
                    .await;

                results.push(MigrationResult {
                    subdomain,
                    status: "skipped".to_string(),
                    version: None,
                    error: Some("No database connection string".to_string()),
                    academic_diagnostics: None,
                });
                continue;
            }
        };

        let result = migrate_single_school(&state, &subdomain, &db_url, latest_version).await;
        results.push(result);
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

        let (version, academic_core_cutover) = if let Some(database_url) = school
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
                    academic_core_cutover_status_from_database(&pool, reported_version).await
                }
                Err(_) => (
                    reported_version,
                    academic_core_cutover_unavailable(reported_version),
                ),
            }
        } else {
            (
                reported_version,
                academic_core_cutover_status(None, reported_version).await,
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

    let pool = match state
        .pool_manager
        .get_pool_with_permission_change(db_url, subdomain)
        .await
    {
        Ok((pool, permissions_changed)) => {
            if permissions_changed {
                state.permission_cache.invalidate_tenant(subdomain);
                state.notify_all_permissions_changed(subdomain);
            }
            pool
        }
        Err(e) => {
            tracing::error!("❌ Failed to get pool for {}: {}", subdomain, e);
            let academic_diagnostics =
                collect_academic_migration_diagnostics(state, subdomain, db_url, &e).await;
            let _ = state
                .admin_client
                .update_migration_status(subdomain, 0, "failed", Some(&e))
                .await;
            return MigrationResult {
                subdomain: subdomain.to_string(),
                status: "failed".to_string(),
                version: None,
                error: Some(e),
                academic_diagnostics,
            };
        }
    };

    let current_version = match get_current_version(&pool).await {
        Ok(v) => v,
        Err(e) => {
            tracing::error!("❌ Failed to get version for {}: {}", subdomain, e);
            let _ = state
                .admin_client
                .update_migration_status(subdomain, 0, "failed", Some(&e))
                .await;
            return MigrationResult {
                subdomain: subdomain.to_string(),
                status: "failed".to_string(),
                version: None,
                error: Some(e),
                academic_diagnostics: None,
            };
        }
    };

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
                academic_diagnostics: None,
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
                academic_diagnostics: None,
            }
        }
    }
}

async fn collect_academic_migration_diagnostics(
    state: &AppState,
    subdomain: &str,
    db_url: &str,
    migration_error: &str,
) -> Option<AcademicMigrationDiagnostics> {
    if !migration_error.contains("ACADEMIC_054_CURRENT_TEACHER_CONFLICT") {
        return None;
    }

    let diagnostic = run_with_diagnostic_timeout(Duration::from_secs(6), async {
        let pool = state
            .pool_manager
            .get_pool_without_migrations(db_url, subdomain)
            .await
            .map_err(|_| "pool")?;
        timetable_service::list_legacy_current_teacher_conflicts(&pool)
            .await
            .map_err(|_| "query")
    })
    .await;

    match diagnostic {
        Some(Ok(current_teacher_conflicts)) => {
            tracing::info!(
                subdomain,
                conflict_count = current_teacher_conflicts.len(),
                "Collected bounded academic migration diagnostics"
            );
            Some(AcademicMigrationDiagnostics {
                current_teacher_conflicts,
            })
        }
        Some(Err(reason)) => {
            tracing::warn!(
                subdomain,
                reason = if reason == "pool" {
                    "academic_migration_diagnostic_pool_unavailable"
                } else {
                    "academic_migration_diagnostic_query_failed"
                }
            );
            None
        }
        None => {
            tracing::warn!(
                subdomain,
                reason = "academic_migration_diagnostic_timed_out"
            );
            None
        }
    }
}

async fn run_with_diagnostic_timeout<T, F>(duration: Duration, future: F) -> Option<T>
where
    F: Future<Output = T>,
{
    tokio::time::timeout(duration, future).await.ok()
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

/// Get current migration version from school database
async fn get_current_version(pool: &PgPool) -> Result<i64, String> {
    let version =
        sqlx::query_scalar::<_, i64>("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
            .fetch_one(pool)
            .await
            .map_err(|e| format!("Failed to get current version: {}", e))?;

    Ok(version)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        modules::academic::cutover_test_support::{
            apply_migrations_through, record_passing_phase_a_reconciliation_marker,
            seed_academic_cutover_fixture, CutoverFixture,
        },
        test_helpers::create_named_test_pool,
    };

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

    #[test]
    fn migration_result_serializes_only_bounded_academic_conflict_diagnostics() {
        let teacher_id = uuid::Uuid::new_v4();
        let version_id = uuid::Uuid::new_v4();
        let period_id = uuid::Uuid::new_v4();
        let first_entry_id = uuid::Uuid::new_v4();
        let second_entry_id = uuid::Uuid::new_v4();
        let value = serde_json::to_value(MigrationResult {
            subdomain: "fixture".to_string(),
            status: "failed".to_string(),
            version: None,
            error: Some("ACADEMIC_054_CURRENT_TEACHER_CONFLICT".to_string()),
            academic_diagnostics: Some(AcademicMigrationDiagnostics {
                current_teacher_conflicts: vec![
                    crate::modules::academic::services::timetable_service::LegacyCurrentTeacherConflict {
                        teacher_id,
                        timetable_version_id: version_id,
                        day_of_week: "MON".to_string(),
                        bell_schedule_period_id: period_id,
                        entry_count: 2,
                        group_code_count: 2,
                        entry_ids: vec![first_entry_id, second_entry_id],
                        group_codes: vec!["ก-1\nforged=true".to_string(), "ก-2".to_string()],
                    },
                ],
            }),
        })
        .unwrap();
        let encoded = serde_json::to_string(&value).unwrap();

        assert_eq!(
            value["academicDiagnostics"]["currentTeacherConflicts"][0]["teacherId"],
            teacher_id.to_string()
        );
        assert_eq!(
            value["academicDiagnostics"]["currentTeacherConflicts"][0]["groupCodes"],
            serde_json::json!(["ก-1\nforged=true", "ก-2"])
        );
        assert_eq!(
            value["academicDiagnostics"]["currentTeacherConflicts"][0]["entryCount"],
            2
        );
        assert!(!encoded.contains('\n'));
        assert!(!encoded.contains("displayName"));
        assert!(!encoded.contains("firstName"));
        assert!(!encoded.contains("lastName"));
    }

    #[tokio::test]
    async fn migration_diagnostic_timeout_is_bounded() {
        let result = run_with_diagnostic_timeout(
            std::time::Duration::from_millis(1),
            std::future::pending::<()>(),
        )
        .await;

        assert!(result.is_none());
    }

    #[test]
    fn school_status_serializes_academic_core_cutover_in_camel_case() {
        let value = serde_json::to_value(SchoolMigrationStatus {
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
        })
        .unwrap();

        assert!(value.get("academicCoreCutover").is_some());
        assert!(value.get("academic_core_cutover").is_none());
    }

    #[test]
    fn reconcile_all_route_is_removed_after_phase_b() {
        let app_source = include_str!("../../../app.rs");
        let internal_routes = &app_source[app_source.find("fn internal_routes").unwrap()
            ..app_source.find("fn protected_routes").unwrap()];
        assert!(!internal_routes.contains("/internal/academic-core/reconcile-all"));
        assert!(internal_routes
            .contains("route_layer(from_fn(middleware::internal_auth::validate_internal_secret))"));
    }
}
