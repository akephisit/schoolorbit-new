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

#[derive(Serialize)]
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

#[derive(Serialize)]
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
    db_name: String,
    db_runtime_password: Option<Vec<u8>>,
    db_migration_password: Option<Vec<u8>>,
    migration_version: Option<i32>,
    migration_status: Option<String>,
    last_migrated_at: Option<chrono::NaiveDateTime>,
    migration_error: Option<String>,
    mutation_pending: bool,
}

/// Internal endpoint to list schools - protected by INTERNAL_API_SECRET
/// Used by GitHub Actions, backend-school cleanup, and migration tracking
pub async fn list_schools_internal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(params): Query<ListSchoolsQuery>,
) -> Result<Json<ListSchoolsResponse>, (StatusCode, String)> {
    verify_internal_secret(&headers)?;

    let mut transaction = state.pool.begin().await.map_err(|_| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Tenant directory unavailable".into(),
        )
    })?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *transaction)
        .await
        .map_err(|_| {
            (
                StatusCode::SERVICE_UNAVAILABLE,
                "Tenant directory unavailable".into(),
            )
        })?;
    let blocked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM tenant_directory_mutations m JOIN schools s ON s.id = m.school_id WHERE ($1::text IS NULL OR s.status = $1))")
        .bind(params.status.as_deref()).fetch_one(&mut *transaction).await
        .map_err(|_| (StatusCode::SERVICE_UNAVAILABLE, "Tenant directory unavailable".into()))?;
    if blocked {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Tenant directory mutation pending".into(),
        ));
    }

    let schools = sqlx::query_as::<_, SchoolRow>(
        "SELECT id, subdomain, name, status, db_name, db_runtime_password, db_migration_password,
                migration_version, migration_status, last_migrated_at, migration_error,
                false AS mutation_pending
         FROM schools
         WHERE NOT EXISTS (SELECT 1 FROM tenant_directory_mutations m WHERE m.school_id = schools.id)
         ORDER BY created_at DESC",
    )
    .fetch_all(&mut *transaction)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    transaction.commit().await.map_err(|_| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Tenant directory unavailable".into(),
        )
    })?;
    let filtered: Vec<SchoolInfo> = schools
        .into_iter()
        .filter(|s| {
            if let Some(ref status) = params.status {
                &s.status == status
            } else {
                true
            }
        })
        .map(|s| {
            let connection = connection_for(&s, "runtime")?;
            Ok(SchoolInfo {
                id: s.id.to_string(),
                subdomain: s.subdomain,
                name: s.name,
                status: s.status,
                db_connection_string: Some(connection),
                migration_version: s.migration_version,
                migration_status: s.migration_status,
                last_migrated_at: s.last_migrated_at.map(|dt| dt.to_string()),
                migration_error: s.migration_error,
            })
        })
        .collect::<Result<Vec<_>, (StatusCode, String)>>()?;

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
        "SELECT id, subdomain, name, status, db_name, db_runtime_password, db_migration_password,
                migration_version, migration_status, last_migrated_at, migration_error,
                EXISTS(SELECT 1 FROM tenant_directory_mutations m WHERE m.school_id = schools.id) AS mutation_pending
         FROM schools
         WHERE subdomain = $1",
    )
    .bind(&subdomain)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
    .ok_or((
        StatusCode::NOT_FOUND,
        format!("School '{}' not found or inactive", subdomain),
    ))?;

    if school.mutation_pending {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Tenant directory mutation pending".into(),
        ));
    }
    if !matches!(school.status.as_str(), "active" | "provisioning") {
        return Err((StatusCode::NOT_FOUND, "School not found or inactive".into()));
    }
    let connection = connection_for(&school, "runtime")?;
    Ok(Json(SchoolInfo {
        id: school.id.to_string(),
        subdomain: school.subdomain,
        name: school.name,
        status: school.status,
        db_connection_string: Some(connection),
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

fn connection_for(school: &SchoolRow, purpose: &str) -> Result<String, (StatusCode, String)> {
    let encrypted = match purpose {
        "runtime" => school.db_runtime_password.as_deref(),
        "migration" => school.db_migration_password.as_deref(),
        _ => None,
    }
    .ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Tenant credentials are not reconciled".into(),
    ))?;
    let result = (|| {
        let cipher = crate::database_credentials::DatabaseCredentials::from_env()?;
        let password = cipher.decrypt(school.id, purpose, encrypted)?;
        crate::clients::postgres_client::PostgresClient::new()?.connection_string(
            school.id,
            &school.db_name,
            purpose,
            &password,
        )
    })();
    result.map_err(|_| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Tenant credentials unavailable".into(),
        )
    })
}

#[derive(Serialize)]
pub struct MigrationConnection {
    pub db_migration_connection_string: String,
}

pub async fn get_migration_connection_internal(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subdomain): Path<String>,
) -> Result<Json<MigrationConnection>, (StatusCode, String)> {
    verify_internal_secret(&headers)?;
    if internal_caller(&headers) != Some("backend-school") {
        return Err((
            StatusCode::FORBIDDEN,
            "Migration caller is not authorized".into(),
        ));
    }
    let school = sqlx::query_as::<_, SchoolRow>(
        "SELECT id, subdomain, name, status, db_name, db_runtime_password, db_migration_password,
          migration_version, migration_status, last_migrated_at, migration_error,
                EXISTS(SELECT 1 FROM tenant_directory_mutations m WHERE m.school_id = schools.id) AS mutation_pending
         FROM schools WHERE subdomain = $1",
    )
    .bind(subdomain)
    .fetch_optional(&state.pool)
    .await
    .map_err(|_| {
        (
            StatusCode::SERVICE_UNAVAILABLE,
            "Tenant lookup unavailable".into(),
        )
    })?
    .ok_or((StatusCode::NOT_FOUND, "Tenant unavailable".into()))?;
    if school.mutation_pending {
        return Err((
            StatusCode::SERVICE_UNAVAILABLE,
            "Tenant directory mutation pending".into(),
        ));
    }
    if !matches!(school.status.as_str(), "active" | "provisioning") {
        return Err((StatusCode::NOT_FOUND, "School not found or inactive".into()));
    }
    Ok(Json(MigrationConnection {
        db_migration_connection_string: connection_for(&school, "migration")?,
    }))
}
