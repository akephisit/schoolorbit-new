use axum::{
    extract::{Extension, Path, Query, State},
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use crate::policies::learning_offering_access_policy::{self, OfferingAction};
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::AppState;
use school_academic_delivery::models::versions::{
    DeliveryVersion, DeliveryVersionQuery, DeliveryVersionSummary,
};
use school_academic_delivery::services::versions;
use school_auth::session_service::AuthenticatedSession;
use school_http::{ApiErrorResponse, ApiResponse, HttpError};

#[utoipa::path(
    get, path="/api/academic/delivery-versions", operation_id="listDeliveryVersions", tag="academic",
    params(DeliveryVersionQuery),
    responses(
        (status=200,description="Opening versions with counts scoped to visible resources",body=ApiResponse<Vec<DeliveryVersionSummary>>),
        (status=401,description="Authentication required",body=ApiErrorResponse),
        (status=403,description="Opening read permission denied",body=ApiErrorResponse)
    )
)]
pub async fn list_versions(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<DeliveryVersionQuery>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let access = learning_offering_access_policy::require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Read,
    )
    .await?;
    let data =
        versions::list_versions(&context.tenant.pool, query.academic_term_id, &access).await?;
    Ok(Json(ApiResponse::ok(data)).into_response())
}

#[utoipa::path(
    get, path="/api/academic/delivery-versions/{id}", operation_id="getDeliveryVersion", tag="academic",
    params(("id"=Uuid,Path,description="Opening version ID")),
    responses(
        (status=200,description="Immutable opening graph scoped to visible resources",body=ApiResponse<DeliveryVersion>),
        (status=401,description="Authentication required",body=ApiErrorResponse),
        (status=403,description="Opening read permission denied",body=ApiErrorResponse),
        (status=404,description="Opening version not found",body=ApiErrorResponse)
    )
)]
pub async fn get_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let access = learning_offering_access_policy::require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Read,
    )
    .await?;
    let data = versions::visible_version(&context.tenant.pool, id, &access).await?;
    Ok(Json(ApiResponse::ok(data)).into_response())
}
