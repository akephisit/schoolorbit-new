use crate::api_response::ApiResponse;
use crate::db::{admin_client::ActiveSchool, pool_manager::PoolManager};
use crate::error::AppError;
use crate::modules::academic::reconciliation::{
    reconcile_academic_core_cutover, reconcile_and_record_academic_core_cutover,
    ReconciliationCheck, PHASE_A_MIGRATION_VERSION,
};
use crate::AppState;
use axum::{extract::State, http::StatusCode, response::IntoResponse, Json};
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AcademicCoreTenantReconciliationResult {
    subdomain: String,
    status: String,
    migration_version: Option<i64>,
    passed: bool,
    checks: Vec<ReconciliationCheck>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error_code: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ReconcileAllAcademicCoreResponse {
    total: usize,
    success: usize,
    failed: usize,
    expected_migration_version: i64,
    results: Vec<AcademicCoreTenantReconciliationResult>,
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
        migration_version: 43,
        passed: Some(false),
        checks: vec![ReconciliationCheck {
            code: "ACADEMIC_CORE_RECON_UNAVAILABLE".to_string(),
            passed: false,
            source_count: 43,
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
            migration_version: 43,
            passed: None,
            checks: Vec::new(),
        };
    }

    let Some(pool) = pool else {
        return academic_core_cutover_unavailable(current_version);
    };

    match reconcile_academic_core_cutover(pool).await {
        Ok(reconciliation) => AcademicCoreCutoverStatus {
            status: if reconciliation.passed {
                "passed".to_string()
            } else {
                "failed".to_string()
            },
            migration_version: reconciliation.migration_version,
            passed: Some(reconciliation.passed),
            checks: reconciliation.checks,
        },
        Err(error) => {
            tracing::warn!(
                reason = "academic_core_reconciliation_query_failed",
                database_code = ?match &error {
                    AppError::DbError(sqlx::Error::Database(database_error)) => database_error.code(),
                    _ => None,
                }
            );
            AcademicCoreCutoverStatus {
                status: "failed".to_string(),
                migration_version: 43,
                passed: Some(false),
                checks: vec![ReconciliationCheck {
                    code: "ACADEMIC_CORE_RECON_UNAVAILABLE".to_string(),
                    passed: false,
                    source_count: 43,
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

fn failed_reconciliation(
    subdomain: String,
    migration_version: Option<i64>,
    error_code: &str,
    checks: Vec<ReconciliationCheck>,
) -> AcademicCoreTenantReconciliationResult {
    AcademicCoreTenantReconciliationResult {
        subdomain,
        status: "failed".to_string(),
        migration_version,
        passed: false,
        checks,
        error_code: Some(error_code.to_string()),
    }
}

async fn reconcile_active_school(
    pool_manager: &PoolManager,
    school: ActiveSchool,
) -> AcademicCoreTenantReconciliationResult {
    let subdomain = school.subdomain;
    let Some(database_url) = school
        .db_connection_string
        .filter(|value| !value.is_empty())
    else {
        return failed_reconciliation(
            subdomain,
            school.migration_version.map(i64::from),
            "DATABASE_URL_UNAVAILABLE",
            Vec::new(),
        );
    };

    let pool = match pool_manager
        .get_pool_for_read_only_status(&database_url, &subdomain)
        .await
    {
        Ok(pool) => pool,
        Err(_) => {
            return failed_reconciliation(
                subdomain,
                school.migration_version.map(i64::from),
                "POOL_UNAVAILABLE",
                Vec::new(),
            );
        }
    };
    let migration_version = match get_current_version(&pool).await {
        Ok(version) => version,
        Err(_) => {
            return failed_reconciliation(
                subdomain,
                None,
                "MIGRATION_VERSION_UNAVAILABLE",
                Vec::new(),
            );
        }
    };
    if migration_version != PHASE_A_MIGRATION_VERSION {
        return failed_reconciliation(
            subdomain,
            Some(migration_version),
            "PHASE_A_VERSION_MISMATCH",
            Vec::new(),
        );
    }

    match reconcile_and_record_academic_core_cutover(&pool).await {
        Ok(report) if report.passed => AcademicCoreTenantReconciliationResult {
            subdomain,
            status: "passed".to_string(),
            migration_version: Some(migration_version),
            passed: true,
            checks: report.checks,
            error_code: None,
        },
        Ok(report) => failed_reconciliation(
            subdomain,
            Some(migration_version),
            "CHECKS_FAILED",
            report.checks,
        ),
        Err(error) => {
            tracing::warn!(
                subdomain,
                reason = "academic_core_reconciliation_failed",
                database_code = ?match &error {
                    AppError::DbError(sqlx::Error::Database(database_error)) => database_error.code(),
                    _ => None,
                }
            );
            failed_reconciliation(
                subdomain,
                Some(migration_version),
                "RECONCILIATION_UNAVAILABLE",
                Vec::new(),
            )
        }
    }
}

async fn reconcile_all_active_schools(
    pool_manager: &PoolManager,
    schools: Vec<ActiveSchool>,
) -> ReconcileAllAcademicCoreResponse {
    let mut results = Vec::with_capacity(schools.len());
    // Operational reconciliation intentionally processes one tenant at a time so database load is
    // bounded and result ordering matches the admin inventory.
    for school in schools {
        results.push(reconcile_active_school(pool_manager, school).await);
    }
    let success = results.iter().filter(|result| result.passed).count();
    let failed = results.len() - success;
    ReconcileAllAcademicCoreResponse {
        total: results.len(),
        success,
        failed,
        expected_migration_version: PHASE_A_MIGRATION_VERSION,
        results,
    }
}

pub async fn reconcile_all_academic_core(
    State(state): State<AppState>,
) -> Result<impl IntoResponse, AppError> {
    let schools = state
        .admin_client
        .list_active_schools()
        .await
        .map_err(|_| {
            AppError::InternalServerError(
                "Failed to fetch schools for Academic Core reconciliation".to_string(),
            )
        })?;
    let response = reconcile_all_active_schools(&state.pool_manager, schools).await;
    Ok((StatusCode::OK, Json(ApiResponse::ok(response))))
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
            let _ = state
                .admin_client
                .update_migration_status(subdomain, 0, "failed", Some(&e))
                .await;
            return MigrationResult {
                subdomain: subdomain.to_string(),
                status: "failed".to_string(),
                version: None,
                error: Some(e),
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
        middleware::internal_auth::{validate_internal_secret, INTERNAL_CALLER_HEADER},
        modules::academic::cutover_test_support::{
            apply_migrations_through, seed_academic_cutover_fixture, CutoverFixture,
        },
        test_helpers::create_named_test_pool,
    };
    use axum::{
        body::Body,
        http::{Request, StatusCode},
        middleware::from_fn,
        routing::post,
        Router,
    };
    use tower::ServiceExt;

    async fn phase_a_pool(name: &str) -> PgPool {
        let pool = create_named_test_pool(name).await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_migrations_through(&pool, 44).await.unwrap();
        pool
    }

    fn active_school(subdomain: &str, database_url: Option<&str>) -> ActiveSchool {
        ActiveSchool {
            subdomain: subdomain.to_string(),
            db_connection_string: database_url.map(str::to_string),
            migration_version: Some(44),
            migration_status: Some("migrated".to_string()),
            last_migrated_at: None,
            migration_error: None,
        }
    }

    #[tokio::test]
    async fn academic_core_status_is_not_applicable_before_migration_043() {
        let status = academic_core_cutover_status(None, 42).await;
        let value = serde_json::to_value(status).unwrap();

        assert_eq!(value["status"], "notApplicable");
        assert_eq!(value["migrationVersion"], 43);
        assert!(value["passed"].is_null());
        assert_eq!(value["checks"], serde_json::json!([]));
    }

    #[tokio::test]
    async fn academic_core_status_exposes_aggregate_reconciliation_only() {
        let pool = create_named_test_pool("migration_status_academic_core").await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_migrations_through(&pool, 43).await.unwrap();

        let status = academic_core_cutover_status(Some(&pool), 43).await;
        let value = serde_json::to_value(status).unwrap();
        let encoded = serde_json::to_string(&value).unwrap();

        assert_eq!(value["status"], "passed");
        assert_eq!(value["passed"], true);
        assert_eq!(value["checks"].as_array().unwrap().len(), 6);
        assert!(!encoded.contains("sourceId"));
        assert!(!encoded.contains("targetId"));
        assert!(!encoded.contains("entityMap"));
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
        assert_eq!(status.status, "passed");
        assert_eq!(status.passed, Some(true));
    }

    #[test]
    fn school_status_serializes_academic_core_cutover_in_camel_case() {
        let value = serde_json::to_value(SchoolMigrationStatus {
            subdomain: "fixture".to_string(),
            migration_version: 43,
            migration_status: "migrated".to_string(),
            last_migrated_at: None,
            migration_error: None,
            academic_core_cutover: AcademicCoreCutoverStatus {
                status: "passed".to_string(),
                migration_version: 43,
                passed: Some(true),
                checks: Vec::new(),
            },
        })
        .unwrap();

        assert!(value.get("academicCoreCutover").is_some());
        assert!(value.get("academic_core_cutover").is_none());
    }

    #[tokio::test]
    async fn reconcile_all_route_is_internal_secret_authenticated() {
        let app_source = include_str!("../../../app.rs");
        let internal_routes = &app_source[app_source.find("fn internal_routes").unwrap()
            ..app_source.find("fn protected_routes").unwrap()];
        assert!(internal_routes.contains("/internal/academic-core/reconcile-all"));
        assert!(internal_routes
            .contains("route_layer(from_fn(middleware::internal_auth::validate_internal_secret))"));

        std::env::set_var(
            "INTERNAL_API_SECRET_ACADEMIC_RECONCILIATION_TEST",
            "reconcile-test-secret",
        );
        let guarded_route = Router::new()
            .route(
                "/internal/academic-core/reconcile-all",
                post(|| async { StatusCode::OK }),
            )
            .route_layer(from_fn(validate_internal_secret));
        let response = guarded_route
            .oneshot(
                Request::post("/internal/academic-core/reconcile-all")
                    .header(INTERNAL_CALLER_HEADER, "academic-reconciliation-test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        std::env::remove_var("INTERNAL_API_SECRET_ACADEMIC_RECONCILIATION_TEST");
    }

    #[tokio::test]
    async fn reconcile_all_aggregates_mixed_results_without_exposing_database_urls() {
        let passing_pool = phase_a_pool("reconcile_all_passing").await;
        let failing_pool = phase_a_pool("reconcile_all_failing").await;
        sqlx::query("UPDATE academic_years SET status = 'ready' WHERE status = 'active'")
            .execute(&failing_pool)
            .await
            .unwrap();

        let pool_manager = PoolManager::new();
        pool_manager
            .insert_test_pool("postgresql://fixture/passing", passing_pool)
            .await;
        pool_manager
            .insert_test_pool("postgresql://fixture/failing", failing_pool)
            .await;

        let response = reconcile_all_active_schools(
            &pool_manager,
            vec![
                active_school("passing", Some("postgresql://fixture/passing")),
                active_school("failing", Some("postgresql://fixture/failing")),
                active_school("missing", None),
            ],
        )
        .await;
        let encoded = serde_json::to_string(&response).unwrap();

        assert_eq!(response.total, 3);
        assert_eq!(response.success, 1);
        assert_eq!(response.failed, 2);
        assert_eq!(response.results[0].status, "passed");
        assert_eq!(
            response.results[1].error_code.as_deref(),
            Some("CHECKS_FAILED")
        );
        assert_eq!(
            response.results[2].error_code.as_deref(),
            Some("DATABASE_URL_UNAVAILABLE")
        );
        assert!(!encoded.contains("postgresql://"));
        assert!(!encoded.contains("sourceId"));
        assert!(!encoded.contains("targetId"));
    }

    #[tokio::test]
    async fn reconcile_all_requires_exact_phase_a_version_and_never_runs_migrations() {
        let version_43_pool = create_named_test_pool("reconcile_all_version_43").await;
        apply_migrations_through(&version_43_pool, 40)
            .await
            .unwrap();
        seed_academic_cutover_fixture(&version_43_pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_migrations_through(&version_43_pool, 43)
            .await
            .unwrap();

        let pool_manager = PoolManager::new();
        pool_manager
            .insert_test_pool("postgresql://fixture/version-43", version_43_pool)
            .await;
        let response = reconcile_all_active_schools(
            &pool_manager,
            vec![active_school(
                "version-43",
                Some("postgresql://fixture/version-43"),
            )],
        )
        .await;

        assert_eq!(response.success, 0);
        assert_eq!(response.failed, 1);
        assert_eq!(response.results[0].migration_version, Some(43));
        assert_eq!(
            response.results[0].error_code.as_deref(),
            Some("PHASE_A_VERSION_MISMATCH")
        );

        let source = include_str!("migration.rs");
        let forbidden_runner = ["run_tenant_", "migrations"].concat();
        assert!(!source.contains(&forbidden_runner));
    }

    #[tokio::test]
    async fn reconciliation_records_one_success_marker_and_none_for_failed_checks() {
        let passing_pool = phase_a_pool("reconcile_marker_passing").await;
        let first = reconcile_and_record_academic_core_cutover(&passing_pool)
            .await
            .unwrap();
        assert!(first.passed);
        let first_created_at: chrono::DateTime<chrono::Utc> = sqlx::query_scalar(
            "SELECT created_at FROM academic_core_cutover_audits WHERE migration_version = 44",
        )
        .fetch_one(&passing_pool)
        .await
        .unwrap();
        let (mapping_version, source_counts, target_counts, source_checksum, target_checksum): (
            String,
            sqlx::types::Json<std::collections::BTreeMap<String, i64>>,
            sqlx::types::Json<std::collections::BTreeMap<String, i64>>,
            String,
            String,
        ) = sqlx::query_as(
            r#"SELECT mapping_algorithm_version, source_counts, target_counts,
                      source_checksum::text, target_checksum::text
               FROM academic_core_cutover_audits
               WHERE migration_version = 44"#,
        )
        .fetch_one(&passing_pool)
        .await
        .unwrap();
        assert_eq!(mapping_version, "academic-core-v1-reconciliation");
        assert_eq!(source_counts.len(), 6);
        assert_eq!(target_counts.len(), 6);
        assert!(source_counts
            .get("ACADEMIC_CORE_RECON_SOURCE_TARGET_COUNTS")
            .is_some());
        assert_eq!(source_checksum.trim().len(), 64);
        assert_eq!(target_checksum.trim().len(), 64);

        let second = reconcile_and_record_academic_core_cutover(&passing_pool)
            .await
            .unwrap();
        assert!(second.passed);
        let (marker_count, second_created_at): (i64, chrono::DateTime<chrono::Utc>) =
            sqlx::query_as(
                "SELECT COUNT(*)::bigint, MIN(created_at) FROM academic_core_cutover_audits WHERE migration_version = 44",
            )
            .fetch_one(&passing_pool)
            .await
            .unwrap();
        assert_eq!(marker_count, 1);
        assert_eq!(second_created_at, first_created_at);

        let failed_pool = phase_a_pool("reconcile_marker_failed").await;
        sqlx::query("UPDATE academic_years SET status = 'ready' WHERE status = 'active'")
            .execute(&failed_pool)
            .await
            .unwrap();
        let failed = reconcile_and_record_academic_core_cutover(&failed_pool)
            .await
            .unwrap();
        assert!(!failed.passed);
        let failed_marker_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM academic_core_cutover_audits WHERE migration_version = 44",
        )
        .fetch_one(&failed_pool)
        .await
        .unwrap();
        assert_eq!(failed_marker_count, 0);
    }

    #[tokio::test]
    async fn reconciliation_accepts_an_empty_new_tenant_after_phase_a() {
        let pool = create_named_test_pool("reconcile_empty_new_tenant").await;
        apply_migrations_through(&pool, 44)
            .await
            .expect("an empty newly provisioned tenant must reach Phase A version 44");

        let report = reconcile_and_record_academic_core_cutover(&pool)
            .await
            .expect("empty canonical context must reconcile without invented year or term rows");

        assert!(report.passed);
        assert!(report.checks.iter().all(|check| check.passed));
        let marker_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM academic_core_cutover_audits WHERE migration_version = 44 AND mapping_algorithm_version = 'academic-core-v1-reconciliation'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(marker_count, 1);
    }
}
