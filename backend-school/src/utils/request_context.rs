use axum::http::HeaderMap;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::auth::extract_user_id;
use crate::middleware::permission::{load_actor_context_or_error, ActorContext};
use crate::modules::auth::models::Claims;
use crate::utils::jwt::{authenticate_for_tenant, AuthenticatedRequest};
use crate::utils::tenant::{
    resolve_tenant_context, resolve_tenant_context_by_subdomain, TenantContext,
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

enum CurrentUserIdentity<'a> {
    MiddlewareClaims(&'a Claims),
    HeaderAuthentication(&'a AuthenticatedRequest),
}

impl CurrentUserIdentity<'_> {
    fn tenant(&self) -> &str {
        match self {
            Self::MiddlewareClaims(claims) => &claims.tenant,
            Self::HeaderAuthentication(authenticated) => &authenticated.tenant,
        }
    }

    fn user_id(&self) -> Result<Uuid, AppError> {
        match self {
            Self::MiddlewareClaims(claims) => user_id_from_claims(claims),
            Self::HeaderAuthentication(authenticated) => Ok(authenticated.user_id),
        }
    }
}

fn current_user_tenant_context_from_identity(
    tenant: TenantContext,
    identity: CurrentUserIdentity<'_>,
) -> Result<CurrentUserTenantContext, AppError> {
    if identity.tenant() != tenant.subdomain {
        return Err(AppError::AuthError(
            "Invalid user authentication".to_string(),
        ));
    }
    let user_id = identity.user_id()?;

    Ok(CurrentUserTenantContext { tenant, user_id })
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
    current_user_tenant_context_from_identity(tenant, CurrentUserIdentity::MiddlewareClaims(claims))
}

pub async fn current_user_tenant_context_from_headers(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<CurrentUserTenantContext, AppError> {
    let tenant = tenant_context(state, headers).await?;
    let authenticated = authenticate_for_tenant(headers, &tenant.subdomain)?;

    current_user_tenant_context_from_identity(
        tenant,
        CurrentUserIdentity::HeaderAuthentication(&authenticated),
    )
}

pub async fn optional_user_id_from_headers(headers: &HeaderMap, pool: &PgPool) -> Option<Uuid> {
    extract_user_id(headers, pool).await.ok()
}

pub fn user_id_from_claims(claims: &Claims) -> Result<Uuid, AppError> {
    Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::AuthError("Invalid user authentication".to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::jwt::{AuthenticatedRequest, JWT_AUDIENCE, JWT_ISSUER, TOKEN_VERSION};
    use sqlx::postgres::PgPoolOptions;

    const USER_ID: &str = "8b391685-4a1c-4f25-a544-b1c5bd0d457e";

    fn claims(tenant: &str) -> Claims {
        Claims {
            sub: USER_ID.to_string(),
            username: "teacher.one".to_string(),
            user_type: "staff".to_string(),
            tenant: tenant.to_string(),
            iss: JWT_ISSUER.to_string(),
            aud: JWT_AUDIENCE.to_string(),
            token_version: TOKEN_VERSION,
            exp: i64::MAX,
            iat: 0,
        }
    }

    fn authenticated_request(tenant: &str) -> AuthenticatedRequest {
        AuthenticatedRequest {
            claims: claims(tenant),
            user_id: Uuid::parse_str(USER_ID).unwrap(),
            tenant: tenant.to_string(),
        }
    }

    fn tenant_context(subdomain: &str) -> TenantContext {
        let pool = PgPoolOptions::new()
            .connect_lazy("postgres://invalid:invalid@127.0.0.1:1/invalid")
            .unwrap();

        TenantContext {
            subdomain: subdomain.to_string(),
            pool,
        }
    }

    #[tokio::test]
    async fn middleware_claims_identity_accepts_matching_tenant() {
        let claims = claims("tenant-a");

        let context = current_user_tenant_context_from_identity(
            tenant_context("tenant-a"),
            CurrentUserIdentity::MiddlewareClaims(&claims),
        )
        .unwrap();

        assert_eq!(context.tenant.subdomain, "tenant-a");
        assert_eq!(context.user_id, Uuid::parse_str(USER_ID).unwrap());
    }

    #[tokio::test]
    async fn middleware_claims_identity_rejects_mismatched_tenant() {
        let claims = claims("tenant-b");

        let result = current_user_tenant_context_from_identity(
            tenant_context("tenant-a"),
            CurrentUserIdentity::MiddlewareClaims(&claims),
        );

        assert!(matches!(result, Err(AppError::AuthError(_))));
    }

    #[tokio::test]
    async fn header_authenticated_identity_accepts_matching_tenant() {
        let authenticated = authenticated_request("tenant-a");

        let context = current_user_tenant_context_from_identity(
            tenant_context("tenant-a"),
            CurrentUserIdentity::HeaderAuthentication(&authenticated),
        )
        .unwrap();

        assert_eq!(context.tenant.subdomain, "tenant-a");
        assert_eq!(context.user_id, Uuid::parse_str(USER_ID).unwrap());
    }

    #[tokio::test]
    async fn header_authenticated_identity_rejects_mismatched_tenant() {
        let authenticated = authenticated_request("tenant-b");

        let result = current_user_tenant_context_from_identity(
            tenant_context("tenant-a"),
            CurrentUserIdentity::HeaderAuthentication(&authenticated),
        );

        assert!(matches!(result, Err(AppError::AuthError(_))));
    }
}
