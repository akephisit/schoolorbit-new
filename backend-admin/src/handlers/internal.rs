use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Json,
};
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;

const INTERNAL_SECRET_HEADER: &str = "X-Internal-Secret";
const INTERNAL_CALLER_HEADER: &str = "X-Internal-Caller";

fn verify_internal_secret(headers: &HeaderMap) -> Result<(), (StatusCode, String)> {
    let expected_secret = expected_internal_secret(headers)?;
    let provided_secret = headers
        .get(INTERNAL_SECRET_HEADER)
        .and_then(|v| v.to_str().ok())
        .ok_or((
            StatusCode::UNAUTHORIZED,
            "Missing X-Internal-Secret header".to_string(),
        ))?;

    if !secrets_match(provided_secret, &expected_secret) {
        return Err((
            StatusCode::UNAUTHORIZED,
            "Invalid internal API secret".to_string(),
        ));
    }

    Ok(())
}

fn expected_internal_secret(headers: &HeaderMap) -> Result<String, (StatusCode, String)> {
    if let Some(caller_secret) = internal_caller(headers).and_then(secret_for_caller) {
        return Ok(caller_secret);
    }

    std::env::var("INTERNAL_API_SECRET").map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Server configuration error".to_string(),
        )
    })
}

fn internal_caller(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(INTERNAL_CALLER_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|caller| !caller.is_empty())
}

fn secret_for_caller(caller: &str) -> Option<String> {
    let env_key = format!(
        "INTERNAL_API_SECRET_{}",
        caller.replace('-', "_").to_ascii_uppercase()
    );
    std::env::var(env_key)
        .ok()
        .filter(|secret| !secret.is_empty())
}

fn secrets_match(provided: &str, expected: &str) -> bool {
    provided.as_bytes().ct_eq(expected.as_bytes()).into()
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
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ListSchoolsQuery>,
) -> Result<Json<ListSchoolsResponse>, (StatusCode, String)> {
    verify_internal_secret(&headers)?;

    let schools = sqlx::query_as::<_, SchoolRow>(
        "SELECT id, subdomain, name, status, db_connection_string,
                migration_version, migration_status, last_migrated_at, migration_error
         FROM schools
         ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
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
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subdomain): Path<String>,
) -> Result<Json<SchoolInfo>, (StatusCode, String)> {
    verify_internal_secret(&headers)?;

    let school = sqlx::query_as::<_, SchoolRow>(
        "SELECT id, subdomain, name, status, db_connection_string,
                migration_version, migration_status, last_migrated_at, migration_error
         FROM schools
         WHERE subdomain = $1 AND status IN ('active', 'provisioning')",
    )
    .bind(&subdomain)
    .fetch_optional(&state.pool)
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
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subdomain): Path<String>,
    Json(body): Json<UpdateMigrationStatusRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    verify_internal_secret(&headers)?;

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
    .execute(&state.pool)
    .await
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Failed to update migration status: {}", e),
        )
    })?;

    Ok(StatusCode::OK)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn headers(secret: &str, caller: Option<&str>) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            INTERNAL_SECRET_HEADER,
            HeaderValue::from_str(secret).unwrap(),
        );
        if let Some(caller) = caller {
            headers.insert(
                INTERNAL_CALLER_HEADER,
                HeaderValue::from_str(caller).unwrap(),
            );
        }
        headers
    }

    #[test]
    fn verifies_fallback_internal_secret() {
        let _guard = env_lock();
        std::env::set_var("INTERNAL_API_SECRET", "shared-secret");
        std::env::remove_var("INTERNAL_API_SECRET_BACKEND_SCHOOL");

        assert!(verify_internal_secret(&headers("shared-secret", None)).is_ok());
        assert!(verify_internal_secret(&headers("wrong-secret", None)).is_err());
    }

    #[test]
    fn caller_secret_overrides_shared_secret() {
        let _guard = env_lock();
        std::env::set_var("INTERNAL_API_SECRET", "shared-secret");
        std::env::set_var("INTERNAL_API_SECRET_BACKEND_SCHOOL", "school-secret");

        assert!(verify_internal_secret(&headers("school-secret", Some("backend-school"))).is_ok());
        assert!(verify_internal_secret(&headers("shared-secret", Some("backend-school"))).is_err());
    }

    #[test]
    fn caller_secret_falls_back_to_shared_secret_when_unset() {
        let _guard = env_lock();
        std::env::set_var("INTERNAL_API_SECRET", "shared-secret");
        std::env::remove_var("INTERNAL_API_SECRET_BACKEND_SCHOOL");

        assert!(verify_internal_secret(&headers("shared-secret", Some("backend-school"))).is_ok());
    }
}
