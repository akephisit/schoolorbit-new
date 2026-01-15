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

#[derive(sqlx::FromRow)]
struct SchoolRow {
    subdomain: String,
    db_connection_string: Option<String>,
    migration_version: Option<i32>,
    migration_status: Option<String>,
    last_migrated_at: Option<chrono::NaiveDateTime>,
    migration_error: Option<String>,
}

/// Migrate all active schools
pub async fn migrate_all_schools(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    println!("🔄 Starting migration for all active schools...");

    // Get latest migration version
    let latest_version = get_latest_migration_version().await
        .map_err(|e| AppError::InternalServerError(format!("Failed to determine latest migration version: {}", e)))?;

    println!("📊 Latest migration version: {}", latest_version);

    // Get all active schools from admin database
    let schools = sqlx::query_as::<_, SchoolRow>(
        "SELECT subdomain, db_connection_string, migration_version, migration_status, 
                last_migrated_at, migration_error
         FROM schools 
         WHERE status = 'active' AND db_connection_string IS NOT NULL"
    )
    .fetch_all(&state.admin_pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to fetch schools: {}", e);
        AppError::InternalServerError("Failed to fetch schools from admin database".to_string())
    })?;

    println!("📊 Found {} active schools", schools.len());

    let mut results = Vec::new();

    for school in schools {
        let subdomain = school.subdomain.clone();
        let db_url = school.db_connection_string.unwrap_or_default();

        if db_url.is_empty() {
            results.push(MigrationResult {
                subdomain: subdomain.clone(),
                status: "skipped".to_string(),
                version: None,
                error: Some("No database connection string".to_string()),
            });
            
            // Update status in admin DB
            let _ = update_migration_status(
                &state.admin_pool,
                &subdomain,
                0,
                "failed",
                Some("No database connection string"),
            )
            .await;
            
            continue;
        }

        // Attempt migration
        let result = migrate_single_school(&state, &subdomain, &db_url, latest_version).await;

        results.push(result);
    }

    let success_count = results
        .iter()
        .filter(|r| r.status == "migrated" || r.status == "already_migrated")
        .count();
    let failed_count = results.iter().filter(|r| r.status == "failed").count();

    println!(
        "✅ Migration complete: {} success, {} failed",
        success_count, failed_count
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
pub async fn migration_status(State(state): State<AppState>) -> Result<impl IntoResponse, AppError> {
    // Get latest version
    let latest_version = get_latest_migration_version().await.unwrap_or(0);

    // Get all schools with migration info
    let schools = sqlx::query_as::<_, SchoolRow>(
        "SELECT subdomain, db_connection_string, migration_version, migration_status,
                last_migrated_at, migration_error
         FROM schools 
         WHERE status = 'active'
         ORDER BY subdomain"
    )
    .fetch_all(&state.admin_pool)
    .await
    .map_err(|e| {
        eprintln!("❌ Failed to fetch schools: {}", e);
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

            // Count statistics
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
                last_migrated_at: s.last_migrated_at.map(|dt| dt.to_string()),
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
    println!("🔄 Migrating school: {}", subdomain);

    // Get pool (this will also run migrations lazily)
    let pool = match state.pool_manager.get_pool(db_url, subdomain).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ Failed to get pool for {}: {}", subdomain, e);
            
            // Update admin DB
            let _ = update_migration_status(
                &state.admin_pool,
                subdomain,
                0,
                "failed",
                Some(&e),
            )
            .await;

            return MigrationResult {
                subdomain: subdomain.to_string(),
                status: "failed".to_string(),
                version: None,
                error: Some(e),
            };
        }
    };

    // Get current version from school database
    let current_version = match get_current_version(&pool).await {
        Ok(v) => v,
        Err(e) => {
            eprintln!("❌ Failed to get version for {}: {}", subdomain, e);
            
            let _ = update_migration_status(
                &state.admin_pool,
                subdomain,
                0,
                "failed",
                Some(&e),
            )
            .await;

            return MigrationResult {
                subdomain: subdomain.to_string(),
                status: "failed".to_string(),
                version: None,
                error: Some(e),
            };
        }
    };

    // Update admin DB with success
    match update_migration_status(
        &state.admin_pool,
        subdomain,
        current_version as i32,
        "migrated",
        None,
    )
    .await
    {
        Ok(_) => {
            let status = if current_version == latest_version {
                "migrated"
            } else {
                "already_migrated"
            };

            println!("✅ {} migrated to version {}", subdomain, current_version);

            MigrationResult {
                subdomain: subdomain.to_string(),
                status: status.to_string(),
                version: Some(current_version),
                error: None,
            }
        }
        Err(e) => {
            eprintln!("⚠️ Migration succeeded but failed to update admin DB: {}", e);

            MigrationResult {
                subdomain: subdomain.to_string(),
                status: "migrated".to_string(),
                version: Some(current_version),
                error: Some(format!("Failed to update admin DB: {}", e)),
            }
        }
    }
}

/// Update migration status in admin database
async fn update_migration_status(
    admin_pool: &PgPool,
    subdomain: &str,
    version: i32,
    status: &str,
    error: Option<&str>,
) -> Result<(), String> {
    sqlx::query(
        "UPDATE schools 
         SET migration_version = $1, 
             migration_status = $2,
             last_migrated_at = CASE WHEN $2 = 'migrated' THEN NOW() ELSE last_migrated_at END,
             migration_error = $3
         WHERE subdomain = $4"
    )
    .bind(version)
    .bind(status)
    .bind(error)
    .bind(subdomain)
    .execute(admin_pool)
    .await
    .map_err(|e| format!("Failed to update migration status: {}", e))?;

    Ok(())
}

/// Get latest migration version from migrations directory
async fn get_latest_migration_version() -> Result<i64, String> {
    // Count migration files in migrations directory
    let migration_dir = std::path::Path::new("./migrations");
    
    if !migration_dir.exists() {
        return Ok(0);
    }

    let entries = std::fs::read_dir(migration_dir)
        .map_err(|e| format!("Failed to read migrations directory: {}", e))?;

    let mut max_version: i64 = 0;

    for entry in entries {
        if let Ok(entry) = entry {
            if let Some(filename) = entry.file_name().to_str() {
                // Extract version from filename (e.g., "001_create_users.sql" -> 1)
                if let Some(version_str) = filename.split('_').next() {
                    if let Ok(version) = version_str.parse::<i64>() {
                        max_version = max_version.max(version);
                    }
                }
            }
        }
    }

    Ok(max_version)
}

/// Get current migration version from school database
async fn get_current_version(pool: &PgPool) -> Result<i64, String> {
    let version = sqlx::query_scalar::<_, i64>(
        "SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations"
    )
    .fetch_one(pool)
    .await
    .map_err(|e| format!("Failed to get current version: {}", e))?;

    Ok(version)
}
