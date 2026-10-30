use std::sync::Arc;

use async_trait::async_trait;
use axum::http::HeaderMap;
use school_academic_timetable::collaboration::{
    MutationAuthorizationPort, TimetableMutationContext,
};
use school_academic_timetable::models::collaboration::DraftMutationIdentity;
use school_auth::runtime::AuthRuntime;
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::{actor_tenant_context_from_session, ActorTenantContext};
use school_authorization::ActorContext;
use school_errors::AppError;
use uuid::Uuid;

use crate::state::AcademicHttpState;

struct SessionMutationAuthorization {
    runtime: AuthRuntime,
    session: AuthenticatedSession,
    permission_revision: school_authorization::PermissionCacheRevision,
}

#[async_trait]
impl MutationAuthorizationPort for SessionMutationAuthorization {
    async fn validate_admission(&self, accepted_actor: &ActorContext) -> Result<(), AppError> {
        if self
            .runtime
            .permission_cache
            .snapshot_revision(&self.session.tenant.subdomain, accepted_actor.user_id)
            != self.permission_revision
        {
            return Err(AppError::Forbidden(
                "Authorization scope changed while the timetable operation was queued".into(),
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
                "Authorization changed while the timetable operation was queued".into(),
            ));
        }
        Ok(())
    }
}

pub(crate) fn from_session(
    state: &AcademicHttpState,
    headers: &HeaderMap,
    session: &AuthenticatedSession,
    context: &ActorTenantContext,
) -> Result<TimetableMutationContext, school_http::HttpError> {
    state
        .websocket_manager
        .register_tenant(&context.tenant.subdomain, context.tenant.tenant_id)?;
    let identity = TimetableMutationHeaders::parse(headers)?;
    Ok(TimetableMutationContext {
        runtime: state.collaboration.clone(),
        actor: context.actor.clone(),
        tenant_id: context.tenant.tenant_id,
        session_id: session.session_id,
        identity,
        staff_directory: state.staff_directory,
        authorization: Arc::new(SessionMutationAuthorization {
            runtime: state.auth_runtime.clone(),
            session: session.clone(),
            permission_revision: context.permission_revision,
        }),
        pool_lease: context.tenant.pool.clone(),
    })
}

/// Optional ownership proof for a captured single-source interaction. Supply all three headers
/// together. Bulk/lifecycle operations omit proof and enter the same atomic domain exclusion.
#[cfg_attr(feature = "api-docs", derive(utoipa::IntoParams))]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Header))]
pub struct TimetableMutationHeaders {
    /// Server-issued tab connection ID; requires the lease and mutation headers together.
    #[cfg_attr(feature = "api-docs", param(rename = "X-Timetable-Connection-Id"))]
    pub connection_id: Option<Uuid>,
    /// Private confirmed lease token. Never broadcast this value to peers.
    #[cfg_attr(feature = "api-docs", param(rename = "X-Timetable-Lease-Token"))]
    pub lease_token: Option<Uuid>,
    /// Client mutation identity, echoed in the committed draft delta.
    #[cfg_attr(feature = "api-docs", param(rename = "X-Timetable-Mutation-Id"))]
    pub mutation_id: Option<Uuid>,
}

impl TimetableMutationHeaders {
    fn parse(headers: &HeaderMap) -> Result<Option<DraftMutationIdentity>, school_http::HttpError> {
        let parse = |name: &str| -> Result<Option<Uuid>, school_http::HttpError> {
            if headers.get_all(name).iter().count() > 1 {
                return Err(school_http::HttpError::BadRequest(
                    "Duplicate timetable ownership proof".into(),
                ));
            }
            headers
                .get(name)
                .map(|value| {
                    value
                        .to_str()
                        .ok()
                        .and_then(|text| Uuid::parse_str(text).ok())
                        .ok_or_else(|| {
                            school_http::HttpError::BadRequest(
                                "Invalid timetable ownership proof".into(),
                            )
                        })
                })
                .transpose()
        };
        let proof = Self {
            connection_id: parse("x-timetable-connection-id")?,
            lease_token: parse("x-timetable-lease-token")?,
            mutation_id: parse("x-timetable-mutation-id")?,
        };
        match (proof.connection_id, proof.lease_token, proof.mutation_id) {
            (None, None, None) => Ok(None),
            (Some(connection_id), Some(lease_token), Some(mutation_id)) => {
                Ok(Some(DraftMutationIdentity {
                    connection_id,
                    lease_token,
                    mutation_id,
                }))
            }
            _ => Err(school_http::HttpError::BadRequest(
                "Complete timetable ownership proof is required".into(),
            )),
        }
    }
}
