use crate::state::AcademicLifecycleHttpState;
use async_trait::async_trait;
use school_academic_lifecycle::ports::{PreparationAuthorizationPort, PreparationCommandContext};
use school_auth::{runtime::AuthRuntime, session_service::AuthenticatedSession};
use school_auth_http::context::{actor_tenant_context_from_session, ActorTenantContext};
use school_authorization::ActorContext;
use school_errors::AppError;
use std::sync::Arc;

struct SessionPreparationAuthorization {
    runtime: AuthRuntime,
    session: AuthenticatedSession,
    permission_revision: school_authorization::PermissionCacheRevision,
    realtime: Arc<dyn school_academic_realtime::AcademicRealtimePort>,
}

#[async_trait]
impl PreparationAuthorizationPort for SessionPreparationAuthorization {
    async fn validate_admission(&self, accepted_actor: &ActorContext) -> Result<(), AppError> {
        if self
            .runtime
            .permission_cache
            .snapshot_revision(&self.session.tenant.subdomain, accepted_actor.user_id)
            != self.permission_revision
        {
            return Err(AppError::Forbidden(
                "Authorization scope changed while the term preparation was queued".into(),
            ));
        }
        if !school_auth_http::tenant::revalidate_realtime_session(&self.runtime, &self.session)
            .await?
        {
            return Err(AppError::AuthError("Session is no longer active".into()));
        }
        let current = actor_tenant_context_from_session(&self.runtime, &self.session).await?;
        let mut accepted = accepted_actor.permissions.clone();
        accepted.sort();
        let mut permissions = current.actor.permissions;
        permissions.sort();
        if current.permission_revision != self.permission_revision
            || current.actor.user_id != accepted_actor.user_id
            || current.tenant.tenant_id != self.session.tenant.tenant_id
            || accepted != permissions
        {
            return Err(AppError::Forbidden(
                "Authorization changed while the term preparation was queued".into(),
            ));
        }
        Ok(())
    }
    fn committed(&self, outcome: &school_academic_lifecycle::models::TermPreparationOutcome) {
        self.realtime.broadcast_academic_core_changed(
            self.session.tenant.subdomain.clone(),
            self.session.user_id,
            "academic_term_preparation",
            Some(outcome.run_id),
            None,
            Some(outcome.target_term_id),
        );
    }
}

pub(crate) fn from_session(
    state: &AcademicLifecycleHttpState,
    session: &AuthenticatedSession,
    context: &ActorTenantContext,
) -> PreparationCommandContext {
    PreparationCommandContext {
        actor: context.actor.clone(),
        tenant_id: context.tenant.tenant_id,
        session_id: session.session_id,
        pool_lease: context.tenant.pool.clone(),
        authorization: Arc::new(SessionPreparationAuthorization {
            runtime: state.auth_runtime.clone(),
            session: session.clone(),
            permission_revision: context.permission_revision,
            realtime: state.websocket_manager.clone(),
        }),
    }
}
