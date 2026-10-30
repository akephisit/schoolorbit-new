use std::sync::Arc;

use async_trait::async_trait;
use axum::http::HeaderMap;
use school_academic_timetable::collaboration::{MutationAuthorizationPort, TimetableMutationContext};
use school_academic_timetable::models::collaboration::DraftMutationIdentity;
use school_auth::runtime::AuthRuntime;
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::{actor_tenant_context_from_session, ActorTenantContext};
use school_authorization::ActorContext;
use school_errors::AppError;
use uuid::Uuid;

use crate::state::AcademicHttpState;

struct SessionMutationAuthorization { runtime: AuthRuntime, session: AuthenticatedSession }

#[async_trait]
impl MutationAuthorizationPort for SessionMutationAuthorization {
    async fn validate_admission(&self, accepted_actor: &ActorContext) -> Result<(), AppError> {
        if !school_auth_http::tenant::revalidate_realtime_session(&self.runtime, &self.session).await.map_err(|_| AppError::AuthError("Session cannot be verified".into()))? {
            return Err(AppError::AuthError("Session is no longer active".into()));
        }
        let current = actor_tenant_context_from_session(&self.runtime, &self.session).await.map_err(|_| AppError::AuthError("Current authorization cannot be verified".into()))?;
        let mut accepted = accepted_actor.permissions.clone(); accepted.sort();
        let mut permissions = current.actor.permissions; permissions.sort();
        if current.actor.user_id != accepted_actor.user_id || current.tenant.tenant_id != self.session.tenant.tenant_id || accepted != permissions {
            return Err(AppError::Forbidden("Authorization changed while the timetable operation was queued".into()));
        }
        Ok(())
    }
}

pub(crate) fn from_session(state: &AcademicHttpState, headers: &HeaderMap, session: &AuthenticatedSession, context: &ActorTenantContext) -> Result<TimetableMutationContext, school_http::HttpError> {
    let keys = ["x-timetable-connection-id", "x-timetable-lease-token", "x-timetable-mutation-id"];
    let present = keys.iter().filter(|key| headers.contains_key(**key)).count();
    let identity = if present == 0 { None } else {
        if present != keys.len() { return Err(school_http::HttpError::BadRequest("Complete timetable ownership proof is required".into())); }
        let parse = |name: &str| headers.get(name).and_then(|value| value.to_str().ok()).and_then(|value| Uuid::parse_str(value).ok()).ok_or_else(|| school_http::HttpError::BadRequest("Invalid timetable ownership proof".into()));
        Some(DraftMutationIdentity { connection_id: parse(keys[0])?, lease_token: parse(keys[1])?, mutation_id: parse(keys[2])? })
    };
    Ok(TimetableMutationContext { runtime: state.collaboration.clone(), actor: context.actor.clone(), tenant_id: context.tenant.tenant_id, session_id: session.session_id,
        identity, staff_directory: state.staff_directory, authorization: Arc::new(SessionMutationAuthorization { runtime: state.auth_runtime.clone(), session: session.clone() }), pool_lease: context.tenant.pool.clone() })
}
