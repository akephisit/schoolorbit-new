use crate::services::SchoolService;
use axum::{
    extract::Query,
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
}

#[derive(Debug, Serialize)]
pub struct ListSchoolsResponse {
    pub schools: Vec<SchoolInfo>,
    pub total: i64,
}

/// Internal endpoint to list schools - protected by INTERNAL_API_SECRET
/// Used by GitHub Actions and other internal services
pub async fn list_schools_internal(
    headers: HeaderMap,
    Query(params): Query<ListSchoolsQuery>,
) -> Result<Json<ListSchoolsResponse>, (StatusCode, String)> {
    // Verify internal API secret
    let internal_secret = std::env::var("INTERNAL_API_SECRET")
        .unwrap_or_else(|_| "default-secret".to_string());

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

    // Get database pool
    let pool = DB_POOL.get().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "Database not initialized".to_string(),
    ))?;

    let service = SchoolService::new(pool.clone());

    // Get all schools (no pagination for internal use)
    let (schools, _total) = service
        .list_schools(1, 1000)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Filter by status if provided
    let filtered_schools: Vec<_> = schools
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
        })
        .collect();

    let filtered_total = filtered_schools.len() as i64;

    Ok(Json(ListSchoolsResponse {
        schools: filtered_schools,
        total: filtered_total,
    }))
}
