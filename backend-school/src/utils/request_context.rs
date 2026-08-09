use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::{load_actor_context_for_session, ActorContext};
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::utils::tenant::TenantContext;
use crate::AppState;

pub struct ActorTenantContext {
    pub tenant: TenantContext,
    pub actor: ActorContext,
}

pub struct CurrentUserTenantContext {
    pub tenant: TenantContext,
    pub user_id: Uuid,
}

pub async fn actor_tenant_context_from_session(
    state: &AppState,
    session: &AuthenticatedSession,
) -> Result<ActorTenantContext, AppError> {
    let tenant = session.tenant.clone();
    let actor = load_actor_context_for_session(
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

    #[tokio::test]
    async fn typed_session_identity_preserves_tenant_and_user() {
        let session = authenticated_session("tenant-a");

        let context = current_user_tenant_context_from_session(&session);

        assert_eq!(context.tenant.subdomain, "tenant-a");
        assert_eq!(context.user_id, Uuid::parse_str(USER_ID).unwrap());
    }
}
