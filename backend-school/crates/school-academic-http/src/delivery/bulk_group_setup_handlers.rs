use axum::{
    extract::{Extension, Path, State},
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use crate::policies::learning_offering_access_policy::{self, OfferingAction};
use crate::state::AcademicHttpState;
use school_academic_delivery::models::BulkGroupSetupRequest;
#[cfg(feature = "api-docs")]
use school_academic_delivery::models::{BulkGroupSetupPreview, BulkGroupSetupResponse};
use school_academic_delivery::services::bulk_group_setup;
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
#[cfg(feature = "api-docs")]
use school_http::ApiErrorResponse;
use school_http::{ApiResponse, HttpError};

#[cfg_attr(feature = "api-docs", utoipa::path(
    post, path="/api/academic/offerings/{id}/groups/bulk-setup/preview", operation_id="previewBulkLearningGroupSetup", tag="academic",
    params(("id"=Uuid,Path,description="Learning offering ID")), request_body=BulkGroupSetupRequest,
    responses(
        (status=200,description="Read-only preview of the explicitly selected draft groups",body=ApiResponse<BulkGroupSetupPreview>),
        (status=400,description="Invalid target scope or configuration",body=ApiErrorResponse),
        (status=401,description="Authentication required",body=ApiErrorResponse),
        (status=403,description="Offering management denied",body=ApiErrorResponse),
        (status=404,description="Offering, version or group missing",body=ApiErrorResponse),
        (status=409,description="Stale draft or closed academic context",body=ApiErrorResponse)
    )
))]
pub async fn preview_bulk_group_setup(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<BulkGroupSetupRequest>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    learning_offering_access_policy::require_learning_offering_access(
        &context.tenant.pool,
        &context.actor,
        id,
        OfferingAction::Manage,
    )
    .await?;
    let preview = bulk_group_setup::preview(&context.tenant.pool, id, request).await?;
    Ok(Json(ApiResponse::ok(preview)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    put, path="/api/academic/offerings/{id}/groups/bulk-setup", operation_id="applyBulkLearningGroupSetup", tag="academic",
    params(("id"=Uuid,Path,description="Learning offering ID")), request_body=BulkGroupSetupRequest,
    responses(
        (status=200,description="Atomic setup saved without publishing or rebuilding student rosters",body=ApiResponse<BulkGroupSetupResponse>),
        (status=400,description="Invalid target scope or configuration",body=ApiErrorResponse),
        (status=401,description="Authentication required",body=ApiErrorResponse),
        (status=403,description="Offering management denied",body=ApiErrorResponse),
        (status=404,description="Offering, version or group missing",body=ApiErrorResponse),
        (status=409,description="Stale draft, blocked target or no changes; no group saved",body=ApiErrorResponse)
    )
))]
pub async fn apply_bulk_group_setup(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<BulkGroupSetupRequest>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    learning_offering_access_policy::require_learning_offering_access(
        &context.tenant.pool,
        &context.actor,
        id,
        OfferingAction::Manage,
    )
    .await?;
    let outcome =
        bulk_group_setup::apply(&context.tenant.pool, context.actor.user_id, id, request).await?;
    state.websocket_manager.broadcast_learning_delivery_changed(
        session.tenant.subdomain.clone(),
        context.actor.user_id,
        outcome.signal.academic_term_id,
        outcome.signal.learning_offering_id,
        None,
        outcome.signal.row_version,
    );
    Ok(Json(ApiResponse::ok(outcome.response)).into_response())
}
