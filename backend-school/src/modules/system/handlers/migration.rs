use crate::AppState;
use crate::error::AppError;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
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
}

/// Migrate all active schools
pub async fn migrate_all_schools(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    tracing::info!("🔄 Starting migration for all active schools...");

    let latest_version = get_latest_migration_version().await
        .map_err(|e| AppError::InternalServerError(format!("Failed to determine latest migration version: {}", e)))?;

    tracing::info!("📊 Latest migration version: {}", latest_version);

    let schools = state.admin_client.list_active_schools().await
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
                let _ = state.admin_client
                    .update_migration_status(&subdomain, 0, "failed", Some("No database connection string"))
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

    tracing::info!("✅ Migration complete: {} success, {} failed", success_count, failed_count);

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
pub async fn migration_status(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    let latest_version = get_latest_migration_version().await.unwrap_or(0);

    let schools = state.admin_client.list_active_schools().await
        .map_err(|e| {
            tracing::error!("❌ Failed to fetch schools: {}", e);
            AppError::InternalServerError("Failed to fetch migration status".to_string())
        })?;

    let total = schools.len();
    let mut migrated = 0;
    let mut pending = 0;
    let mut failed = 0;
    let mut outdated = 0;

    let school_statuses: Vec<SchoolMigrationStatus> = schools
        .into_iter()
        .map(|s| {
            let version = s.migration_version.unwrap_or(0);
            let status = s.migration_status.unwrap_or_else(|| "pending".to_string());

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

            SchoolMigrationStatus {
                subdomain: s.subdomain,
                migration_version: version,
                migration_status: if version < latest_version as i32 && status == "migrated" {
                    "outdated".to_string()
                } else {
                    status
                },
                last_migrated_at: s.last_migrated_at,
                migration_error: s.migration_error,
            }
        })
        .collect();

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

    let pool = match state.pool_manager.get_pool(db_url, subdomain).await {
        Ok(p) => p,
        Err(e) => {
            tracing::error!("❌ Failed to get pool for {}: {}", subdomain, e);
            let _ = state.admin_client
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
            let _ = state.admin_client
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

    match state.admin_client
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
            tracing::warn!("⚠️ Migration succeeded but failed to update admin service: {}", e);
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
    let version = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations",
    )
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Failed to get current version: {}", e))?;

    Ok(version)
}
