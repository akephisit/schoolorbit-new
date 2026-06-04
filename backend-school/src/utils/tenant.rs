use axum::http::HeaderMap;
use sqlx::PgPool;

use crate::db::school_mapping::get_school_database_url;
use crate::error::AppError;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;

#[derive(Clone)]
pub struct TenantContext {
    pub subdomain: String,
    pub pool: PgPool,
}

pub async fn resolve_tenant_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<TenantContext, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    resolve_tenant_context_by_subdomain(state, &subdomain).await
}

pub async fn resolve_tenant_context_by_subdomain(
    state: &AppState,
    subdomain: &str,
) -> Result<TenantContext, AppError> {
    let db_url = get_school_database_url(&state.admin_client, subdomain)
        .await
        .map_err(|error| {
            tracing::warn!(
                subdomain = %subdomain,
                error = %error,
                "Failed to resolve school database URL"
            );
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state
        .pool_manager
        .get_pool(&db_url, subdomain)
        .await
        .map_err(|error| {
            tracing::error!(
                subdomain = %subdomain,
                error = %error,
                "Failed to resolve tenant database pool"
            );
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;

    Ok(TenantContext {
        subdomain: subdomain.to_string(),
        pool,
    })
}

pub async fn resolve_tenant_pool(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<PgPool, AppError> {
    Ok(resolve_tenant_context(state, headers).await?.pool)
}
