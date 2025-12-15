use crate::AppState;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Serialize)]
struct MigrationResult {
    subdomain: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct MigrateAllResponse {
    total: usize,
    success: usize,
    failed: usize,
    results: Vec<MigrationResult>,
}

#[derive(Serialize)]
struct MigrationStatusResponse {
    total_schools_migrated: usize,
    active_pools: usize,
    migrated_schools: Vec<String>,
}

#[derive(sqlx::FromRow)]
struct SchoolRow {
    subdomain: String,
    db_connection_string: Option<String>,
}

/// Migrate all active schools
pub async fn migrate_all_schools(State(state): State<AppState>) -> Response {
    println!("🔄 Starting migration for all active schools...");

    // Get all active schools from admin database
    let schools = match sqlx::query_as::<_, SchoolRow>(
        "SELECT subdomain, db_connection_string 
         FROM schools 
         WHERE status = 'active' AND db_connection_string IS NOT NULL"
    )
    .fetch_all(&state.admin_pool)
    .await
    {
        Ok(schools) => schools,
        Err(e) => {
            eprintln!("❌ Failed to fetch schools: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": "Failed to fetch schools from admin database"
                })),
            )
                .into_response();
        }
    };

    println!("📊 Found {} active schools", schools.len());

    let mut results = Vec::new();

    for school in schools {
        let subdomain = school.subdomain.clone();
        let db_url = school.db_connection_string.unwrap_or_default();

        if db_url.is_empty() {
            results.push(MigrationResult {
                subdomain: subdomain.clone(),
                status: "skipped".to_string(),
                error: Some("No database connection string".to_string()),
            });
            continue;
        }

        // Attempt migration
        let result = match migrate_single_school(&state, &subdomain, &db_url).await {
            Ok(newly_migrated) => {
                let status = if newly_migrated { "migrated" } else { "already_migrated" };
                MigrationResult {
                    subdomain: subdomain.clone(),
                    status: status.to_string(),
                    error: None,
                }
            }
            Err(e) => {
                eprintln!("❌ Migration failed for {}: {}", subdomain, e);
                MigrationResult {
                    subdomain: subdomain.clone(),
                    status: "failed".to_string(),
                    error: Some(e),
                }
            }
        };

        results.push(result);
    }

    let success_count = results.iter().filter(|r| r.status == "migrated" || r.status == "already_migrated").count();
    let failed_count = results.iter().filter(|r| r.status == "failed").count();

    println!(
        "✅ Migration complete: {} success, {} failed",
        success_count, failed_count
    );

    (
        StatusCode::OK,
        Json(MigrateAllResponse {
            total: results.len(),
            success: success_count,
            failed: failed_count,
            results,
        }),
    )
        .into_response()
}

/// Get migration status
pub async fn migration_status(State(state): State<AppState>) -> Response {
    let migrated_schools = state
        .pool_manager
        .migration_tracker()
        .get_migrated_schools()
        .await;

    let active_pools = state.pool_manager.pool_count().await;

    (
        StatusCode::OK,
        Json(MigrationStatusResponse {
            total_schools_migrated: migrated_schools.len(),
            active_pools,
            migrated_schools,
        }),
    )
        .into_response()
}

/// Helper: Migrate a single school
async fn migrate_single_school(
    state: &AppState,
    subdomain: &str,
    db_url: &str,
) -> Result<bool, String> {
    // Get pool (this will also run migrations lazily)
    let pool = state
        .pool_manager
        .get_pool(db_url, subdomain)
        .await?;

    // Check migration tracker
    let newly_migrated = state
        .pool_manager
        .migration_tracker()
        .run_migrations_once(subdomain, &pool)
        .await?;

    Ok(newly_migrated)
}
