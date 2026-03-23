use axum::{
    extract::{Path, Query},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::sync::OnceLock;

static DB_POOL: OnceLock<PgPool> = OnceLock::new();

pub fn init_pool(pool: PgPool) {
    DB_POOL.set(pool).ok();
}

fn verify_internal_secret(headers: &HeaderMap) -> Result<(), (StatusCode, String)> {
    let internal_secret = std::env::var("INTERNAL_API_SECRET")
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Server configuration error".to_string()))?;

    let auth_header = headers
        .get("X-Internal-Secret")
        .and_then(|v| v.to_str().ok())
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "Missing X-Internal-Secret header".to_string(),
        ))?;

    if auth_header != internal_secret {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Invalid internal API secret".to_string(),
        ));
    }

    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct ListSchoolsQuery {
    #[serde(default)]
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SchoolInfo {
    pub id: String,
    pub subdomain: String,
    pub name: String,
    pub status: String,
    pub db_connection_string: Option<String>,
    pub migration_version: Option<i32>,
    pub migration_status: Option<String>,
    pub last_migrated_at: Option<String>,
    pub migration_error: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListSchoolsResponse {
    pub schools: Vec<SchoolInfo>,
    pub total: i64,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMigrationStatusRequest {
    pub migration_version: i32,
    pub migration_status: String,
    pub migration_error: Option<String>,
}

#[derive(sqlx::FromRow)]
struct SchoolRow {
    id: uuid::Uuid,
    subdomain: String,
    name: String,
    status: String,
    db_connection_string: Option<String>,
    migration_version: Option<i32>,
    migration_status: Option<String>,
    last_migrated_at: Option<chrono::NaiveDateTime>,
    migration_error: Option<String>,
}

/// Internal endpoint to list schools - protected by INTERNAL_API_SECRET
/// Used by GitHub Actions, backend-school cleanup, and migration tracking
pub async fn list_schools_internal(
    headers: HeaderMap,
    Query(params): Query<ListSchoolsQuery>,
) -> Result<Json<ListSchoolsResponse>, (StatusCode, String)> {
    verify_internal_secret(&headers)?;

    let pool = DB_POOL.get().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "Database not initialized".to_string(),
    ))?;

    let schools = sqlx::query_as::<_, SchoolRow>(
        "SELECT id, subdomain, name, status, db_connection_string,
                migration_version, migration_status, last_migrated_at, migration_error
         FROM schools
         ORDER BY created_at DESC",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let filtered: Vec<SchoolInfo> = schools
        .into_iter()
        .filter(|s| {
            if let Some(ref status) = params.status {
                &s.status == status
            } else {
                true
            }
        })
        .map(|s| SchoolInfo {
            id: s.id.to_string(),
            subdomain: s.subdomain,
            name: s.name,
            status: s.status,
            db_connection_string: s.db_connection_string,
            migration_version: s.migration_version,
            migration_status: s.migration_status,
            last_migrated_at: s.last_migrated_at.map(|dt| dt.to_string()),
            migration_error: s.migration_error,
        })
        .collect();

    let total = filtered.len() as i64;

    Ok(Json(ListSchoolsResponse {
        schools: filtered,
        total,
    }))
}

/// Internal endpoint to get a single school by subdomain - protected by INTERNAL_API_SECRET
/// Used by backend-school to resolve tenant database URL on each request
pub async fn get_school_by_subdomain_internal(
    headers: HeaderMap,
    Path(subdomain): Path<String>,
) -> Result<Json<SchoolInfo>, (StatusCode, String)> {
    verify_internal_secret(&headers)?;

    let pool = DB_POOL.get().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "Database not initialized".to_string(),
    ))?;

    let school = sqlx::query_as::<_, SchoolRow>(
        "SELECT id, subdomain, name, status, db_connection_string,
                migration_version, migration_status, last_migrated_at, migration_error
         FROM schools
         WHERE subdomain = $1 AND status IN ('active', 'provisioning')",
    )
    .bind(&subdomain)
    .fetch_optional(pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((
        StatusCode::NOT_FOUND,
        format!("School '{}' not found or inactive", subdomain),
    ))?;

    Ok(Json(SchoolInfo {
        id: school.id.to_string(),
        subdomain: school.subdomain,
        name: school.name,
        status: school.status,
        db_connection_string: school.db_connection_string,
        migration_version: school.migration_version,
        migration_status: school.migration_status,
        last_migrated_at: school.last_migrated_at.map(|dt| dt.to_string()),
        migration_error: school.migration_error,
    }))
}

/// Internal endpoint to update migration status - protected by INTERNAL_API_SECRET
/// Called by backend-school after migrating a tenant database
pub async fn update_migration_status_internal(
    headers: HeaderMap,
    Path(subdomain): Path<String>,
    Json(body): Json<UpdateMigrationStatusRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    verify_internal_secret(&headers)?;

    let pool = DB_POOL.get().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "Database not initialized".to_string(),
    ))?;

    sqlx::query(
        "UPDATE schools
         SET migration_version = $1,
             migration_status = $2,
             last_migrated_at = CASE WHEN $2 = 'migrated' THEN NOW() ELSE last_migrated_at END,
             migration_error = $3,
             updated_at = NOW()
         WHERE subdomain = $4",
    )
    .bind(body.migration_version)
    .bind(&body.migration_status)
    .bind(&body.migration_error)
    .bind(&subdomain)
    .execute(pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to update migration status: {}", e),
        )
    })?;

    Ok(StatusCode::OK)
}
