use axum::http::HeaderMap;
use chrono::Utc;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::{load_actor_context_or_error, ActorContext};
use crate::modules::auth::{
    http::presented_session_token,
    session_crypto::RawSessionToken,
    session_repository::SessionMaintenanceMode,
    session_service::{self, AuthenticatedSession},
};
use crate::utils::tenant::{
    resolve_auth_tenant_context, resolve_tenant_context, resolve_tenant_context_by_subdomain,
    TenantContext,
};
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

pub async fn tenant_context_by_subdomain(
    state: &AppState,
    subdomain: &str,
) -> Result<TenantContext, AppError> {
    resolve_tenant_context_by_subdomain(state, subdomain).await
}

pub async fn actor_tenant_context_from_session(
    state: &AppState,
    session: &AuthenticatedSession,
) -> Result<ActorTenantContext, AppError> {
    let tenant = session.tenant.clone();
    let actor = load_actor_context_or_error(
        session.user_id,
        &tenant.subdomain,
        &tenant.pool,
        &state.permission_cache,
    )
    .await?;

    Ok(ActorTenantContext { tenant, actor })
}

pub fn current_user_tenant_context_from_session(
    session: &AuthenticatedSession,
) -> CurrentUserTenantContext {
    CurrentUserTenantContext {
        tenant: session.tenant.clone(),
        user_id: session.user_id,
    }
}

// Temporary compatibility for handler modules migrated in Tasks 7-10. This
// authenticates only the opaque session credential and deliberately never
// rotates it; the router middleware remains the sole ordinary-request owner of
// session rotation and response credentials.
async fn authenticated_session_from_headers(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<AuthenticatedSession, AppError> {
    let tenant = resolve_auth_tenant_context(&state.auth_runtime, headers, None).await?;
    let token = presented_session_token(headers)?
        .ok_or_else(|| AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string()))?;
    let context = state.auth_runtime.service_context(tenant);
    session_service::authenticate(
        &context,
        token.token_hash(),
        Utc::now(),
        SessionMaintenanceMode::TouchOnly,
        RawSessionToken::generate,
    )
    .await?
    .map(|authentication| authentication.authenticated)
    .ok_or_else(|| AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string()))
}

pub async fn actor_tenant_context(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<ActorTenantContext, AppError> {
    let session = authenticated_session_from_headers(state, headers).await?;
    actor_tenant_context_from_session(state, &session).await
}

pub async fn current_user_tenant_context_from_headers(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<CurrentUserTenantContext, AppError> {
    let session = authenticated_session_from_headers(state, headers).await?;
    Ok(current_user_tenant_context_from_session(&session))
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::postgres::PgPoolOptions;

    const USER_ID: &str = "8b391685-4a1c-4f25-a544-b1c5bd0d457e";

    fn authenticated_session(tenant: &str) -> AuthenticatedSession {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://invalid:invalid@127.0.0.1:1/invalid")
            .unwrap();
        AuthenticatedSession {
            tenant: TenantContext {
                tenant_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
                subdomain: tenant.to_string(),
                pool,
            },
            session_id: Uuid::parse_str("22222222-2222-2222-2222-222222222222").unwrap(),
            user_id: Uuid::parse_str(USER_ID).unwrap(),
            username: "teacher.one".to_string(),
            user_type: "staff".to_string(),
        }
    }

    #[test]
    fn typed_session_identity_preserves_tenant_and_user() {
        let session = authenticated_session("tenant-a");

        let context = current_user_tenant_context_from_session(&session);

        assert_eq!(context.tenant.subdomain, "tenant-a");
        assert_eq!(context.user_id, Uuid::parse_str(USER_ID).unwrap());
    }
}
