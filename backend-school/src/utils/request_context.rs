use crate::AppState;
use school_auth::session_service::AuthenticatedSession;
pub use school_auth_http::context::{
    current_user_tenant_context_from_session, ActorTenantContext, CurrentUserTenantContext,
};
use school_errors::AppError;

pub async fn actor_tenant_context_from_session(
    state: &AppState,
    session: &AuthenticatedSession,
) -> Result<ActorTenantContext, AppError> {
    school_auth_http::context::actor_tenant_context_from_session(&state.auth_runtime, session).await
}
