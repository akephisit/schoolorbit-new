use crate::AppState;
use axum::http::HeaderMap;
use school_auth::runtime::AuthRuntime;
use school_errors::AppError;
use school_tenancy::TenantContext;
use sqlx::PgPool;

pub async fn resolve_tenant_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<TenantContext, AppError> {
    school_auth_http::tenant::resolve_tenant_context(&state.auth_runtime, headers).await
}
pub async fn resolve_tenant_context_by_subdomain(
    state: &AppState,
    subdomain: &str,
) -> Result<TenantContext, AppError> {
    school_auth_http::tenant::resolve_tenant_context_by_subdomain(&state.auth_runtime, subdomain)
        .await
}
pub async fn tenant_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<TenantContext, AppError> {
    resolve_tenant_context(state, headers).await
}
pub async fn tenant_pool(state: &AppState, headers: &HeaderMap) -> Result<PgPool, AppError> {
    Ok(resolve_tenant_context(state, headers).await?.pool)
}
pub async fn tenant_context_by_subdomain(
    state: &AppState,
    subdomain: &str,
) -> Result<TenantContext, AppError> {
    resolve_tenant_context_by_subdomain(state, subdomain).await
}
pub async fn resolve_auth_tenant_context(
    runtime: &AuthRuntime,
    headers: &HeaderMap,
    hint: Option<&str>,
) -> Result<TenantContext, AppError> {
    school_auth_http::tenant::resolve_auth_tenant_context(runtime, headers, hint).await
}
