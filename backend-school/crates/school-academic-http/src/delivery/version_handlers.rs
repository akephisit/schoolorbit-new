use axum::{
    extract::{Extension, Path, Query, State},
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use crate::policies::learning_offering_access_policy::{self, OfferingAction};
use crate::state::AcademicHttpState;
use school_academic_delivery::models::versions::{
    DeliveryVersion, DeliveryVersionQuery, DeliveryVersionSummary,
};
use school_academic_delivery::services::versions;
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
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
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<DeliveryVersionQuery>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let access = learning_offering_access_policy::require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Read,
    )
    .await?;
    let data = versions::visible_version(&context.tenant.pool, id, &access).await?;
    Ok(Json(ApiResponse::ok(data)).into_response())
}

#[utoipa::path(delete,path="/api/academic/delivery-versions/{id}",operation_id="deleteDeliveryVersion",tag="academic",
params(("id"=Uuid,Path,description="Unpublished opening version ID")),
request_body=school_academic_delivery::models::versions::DeleteDeliveryVersionRequest,
responses((status=200,description="Permanently deleted opening version",body=ApiResponse<school_academic_delivery::models::versions::DeletedDeliveryVersion>),
(status=400,description="Invalid deletion request",body=ApiErrorResponse),
(status=403,description="Management scope denied",body=ApiErrorResponse),
(status=404,description="Deleted or missing version",body=ApiErrorResponse),
(status=409,description="Published, referenced or stale version",body=ApiErrorResponse)))]
pub async fn delete_version(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<school_academic_delivery::models::versions::DeleteDeliveryVersionRequest>,
) -> Result<Response, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let version = versions::get_version(&context.tenant.pool, id).await?;
    let mut ids = version
        .snapshot
        .offerings
        .iter()
        .map(|o| o.id)
        .collect::<Vec<_>>();
    if let Some(base) = version.source_version_id {
        ids.extend(
            versions::get_version(&context.tenant.pool, base)
                .await?
                .snapshot
                .offerings
                .into_iter()
                .map(|o| o.id),
        );
    }
    ids.sort();
    ids.dedup();
    learning_offering_access_policy::require_learning_offering_batch_access(
        &context.tenant.pool,
        &context.actor,
        &ids,
        OfferingAction::Manage,
    )
    .await?;
    let result = school_academic_delivery::services::change_sets::delete_version(
        &context.tenant.pool,
        context.actor.user_id,
        id,
        request,
    )
    .await?;
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        context.actor.user_id,
        "academic_term_change_set",
        Some(result.change_set_id),
        Some(version.academic_year_id),
        Some(version.academic_term_id),
    );
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        context.actor.user_id,
        "academic_delivery_version",
        Some(id),
        Some(version.academic_year_id),
        Some(version.academic_term_id),
    );
    Ok(Json(ApiResponse::ok(result)).into_response())
}
