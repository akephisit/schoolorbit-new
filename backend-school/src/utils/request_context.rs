use axum::http::HeaderMap;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::auth::extract_user_id;
use crate::middleware::permission::{load_actor_context_or_error, ActorContext};
use crate::modules::auth::models::Claims;
use crate::utils::tenant::{resolve_tenant_context, TenantContext};
use crate::AppState;

pub struct ActorTenantContext {
    pub tenant: TenantContext,
    pub actor: ActorContext,
}

pub struct CurrentUserTenantContext {
    pub tenant: TenantContext,
    pub user_id: Uuid,
}

pub async fn tenant_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<TenantContext, AppError> {
    resolve_tenant_context(state, headers).await
}

pub async fn tenant_pool(state: &AppState, headers: &HeaderMap) -> Result<PgPool, AppError> {
    Ok(tenant_context(state, headers).await?.pool)
}

pub async fn actor_tenant_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<ActorTenantContext, AppError> {
    let tenant = tenant_context(state, headers).await?;
    let actor = load_actor_context_or_error(headers, &tenant.pool, &state.permission_cache).await?;

    Ok(ActorTenantContext { tenant, actor })
}

pub async fn current_user_tenant_context_from_claims(
    state: &AppState,
    headers: &HeaderMap,
    claims: &Claims,
) -> Result<CurrentUserTenantContext, AppError> {
    let tenant = tenant_context(state, headers).await?;
    let user_id = user_id_from_claims(claims)?;

    Ok(CurrentUserTenantContext { tenant, user_id })
}

pub async fn current_user_tenant_context_from_headers(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<CurrentUserTenantContext, AppError> {
    let tenant = tenant_context(state, headers).await?;
    let user_id = extract_user_id(headers, &tenant.pool)
        .await
        .map_err(AppError::AuthError)?;

    Ok(CurrentUserTenantContext { tenant, user_id })
}

pub fn user_id_from_claims(claims: &Claims) -> Result<Uuid, AppError> {
    Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::AuthError("Invalid user authentication".to_string()))
}
