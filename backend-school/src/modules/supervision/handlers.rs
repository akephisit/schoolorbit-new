use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use utoipa::{IntoParams, ToSchema};

use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::modules::supervision::models::{
    AcknowledgeObservationRequest, ApproveObservationRequest, CancelObservationRequest,
    CreateSupervisionCycleRequest, CreateSupervisionTemplateRequest,
    ReplaceObservationEvaluatorsRequest, RequestSupervisionObservationRequest,
    ReturnObservationRequest, SaveEvaluationRequest, SupervisionCycle, SupervisionCycleQuery,
    SupervisionEvaluatorAvailability, SupervisionObservation, SupervisionObservationFilter,
    SupervisionObservationStatus, SupervisionTeacherStatusRow, SupervisionTemplate,
    UpdateRequestedObservationRequest, UpdateSupervisionCycleRequest,
    UpdateSupervisionObservationRequest, UpdateSupervisionTemplateRequest,
};
use crate::modules::supervision::services;
use crate::policies::supervision_access_policy;
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::AppState;

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase")]
pub struct ListObservationsQuery {
    pub academic_year_id: Uuid,
    pub academic_term_id: Option<Uuid>,
    pub cycle_id: Option<Uuid>,
    pub status: Option<SupervisionObservationStatus>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemsData<T> {
    pub items: Vec<T>,
}

fn actor_can_view_unreleased_results(actor: &ActorContext) -> bool {
    supervision_access_policy::can_manage_school(actor)
        || supervision_access_policy::can_manage_organization_unit(actor)
        || supervision_access_policy::can_manage_organization_tree(actor)
        || supervision_access_policy::can_approve_school(actor)
}

fn redact_observation_results_for_actor(
    actor: &ActorContext,
    observation: &mut SupervisionObservation,
) {
    if !services::can_view_observation_results(
        observation.status,
        actor_can_view_unreleased_results(actor),
    ) {
        observation.average_rating = None;
    }
}

fn redacted_observation_for_actor(
    actor: &ActorContext,
    mut observation: SupervisionObservation,
) -> SupervisionObservation {
    redact_observation_results_for_actor(actor, &mut observation);
    observation
}

fn redact_observations_results_for_actor(
    actor: &ActorContext,
    observations: &mut [SupervisionObservation],
) {
    for observation in observations {
        redact_observation_results_for_actor(actor, observation);
    }
}

fn redact_teacher_status_results_for_actor(
    actor: &ActorContext,
    rows: &mut [SupervisionTeacherStatusRow],
) {
    let can_view_unreleased_results = actor_can_view_unreleased_results(actor);
    for row in rows {
        if let Some(status) = row.status {
            if !services::can_view_observation_results(status, can_view_unreleased_results) {
                row.average_rating = None;
            }
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/supervision/cycles",
    operation_id = "listSupervisionCycles",
    tag = "supervision",
    params(SupervisionCycleQuery),
    responses(
        (status = 200, description = "Supervision cycles", body = ApiResponse<ItemsData<SupervisionCycle>>),
        (status = 400, description = "Invalid academic context", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision access denied", body = ApiErrorResponse)
    )
)]
pub async fn list_cycles(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<SupervisionCycleQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    supervision_access_policy::require_supervision_access(&context.actor)?;

    let items = services::list_cycles(&context.tenant.pool, query).await?;

    Ok(Json(ApiResponse::ok(ItemsData::<SupervisionCycle> { items })).into_response())
}

#[utoipa::path(
    post,
    path = "/api/supervision/cycles",
    operation_id = "createSupervisionCycle",
    tag = "supervision",
    request_body = CreateSupervisionCycleRequest,
    responses(
        (status = 201, description = "Supervision cycle created", body = ApiResponse<SupervisionCycle>),
        (status = 400, description = "Invalid supervision cycle", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision management denied", body = ApiErrorResponse)
    )
)]
pub async fn create_cycle(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<CreateSupervisionCycleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    supervision_access_policy::require_manage_school(&context.actor)?;

    let cycle =
        services::create_cycle(&context.tenant.pool, payload, context.actor.user_id).await?;

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(cycle))).into_response())
}

#[utoipa::path(
    patch,
    path = "/api/supervision/cycles/{id}",
    operation_id = "updateSupervisionCycle",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision cycle ID")),
    request_body = UpdateSupervisionCycleRequest,
    responses(
        (status = 200, description = "Supervision cycle updated", body = ApiResponse<SupervisionCycle>),
        (status = 400, description = "Invalid supervision cycle", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision cycle not found", body = ApiErrorResponse)
    )
)]
pub async fn update_cycle(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSupervisionCycleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    supervision_access_policy::require_manage_school(&context.actor)?;

    let cycle = services::update_cycle(&context.tenant.pool, id, payload).await?;

    Ok(Json(ApiResponse::ok(cycle)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/supervision/templates",
    operation_id = "listSupervisionTemplates",
    tag = "supervision",
    responses(
        (status = 200, description = "Supervision templates", body = ApiResponse<ItemsData<SupervisionTemplate>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision access denied", body = ApiErrorResponse)
    )
)]
pub async fn list_templates(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    supervision_access_policy::require_supervision_access(&context.actor)?;

    let items = services::list_templates(&context.tenant.pool).await?;

    Ok(Json(ApiResponse::ok(ItemsData::<SupervisionTemplate> { items })).into_response())
}

#[utoipa::path(
    post,
    path = "/api/supervision/templates",
    operation_id = "createSupervisionTemplate",
    tag = "supervision",
    request_body = CreateSupervisionTemplateRequest,
    responses(
        (status = 201, description = "Supervision template created", body = ApiResponse<SupervisionTemplate>),
        (status = 400, description = "Invalid supervision template", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision management denied", body = ApiErrorResponse)
    )
)]
pub async fn create_template(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<CreateSupervisionTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    supervision_access_policy::require_manage_school(&context.actor)?;

    let template =
        services::create_template(&context.tenant.pool, payload, context.actor.user_id).await?;

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(template))).into_response())
}

#[utoipa::path(
    get,
    path = "/api/supervision/templates/{id}",
    operation_id = "getSupervisionTemplate",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision template ID")),
    responses(
        (status = 200, description = "Supervision template", body = ApiResponse<SupervisionTemplate>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision access denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision template not found", body = ApiErrorResponse)
    )
)]
pub async fn get_template(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    supervision_access_policy::require_supervision_access(&context.actor)?;

    let template = services::get_template(&context.tenant.pool, id).await?;

    Ok(Json(ApiResponse::ok(template)).into_response())
}

#[utoipa::path(
    patch,
    path = "/api/supervision/templates/{id}",
    operation_id = "updateSupervisionTemplate",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision template ID")),
    request_body = UpdateSupervisionTemplateRequest,
    responses(
        (status = 200, description = "Supervision template updated", body = ApiResponse<SupervisionTemplate>),
        (status = 400, description = "Invalid supervision template", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision template not found", body = ApiErrorResponse)
    )
)]
pub async fn update_template(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSupervisionTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    supervision_access_policy::require_manage_school(&context.actor)?;

    let template = services::update_template(&context.tenant.pool, id, payload).await?;

    Ok(Json(ApiResponse::ok(template)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/supervision/observations",
    operation_id = "listSupervisionObservations",
    tag = "supervision",
    params(ListObservationsQuery),
    responses(
        (status = 200, description = "Supervision observations", body = ApiResponse<ItemsData<SupervisionObservation>>),
        (status = 400, description = "Invalid observation filter", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision access denied", body = ApiErrorResponse)
    )
)]
pub async fn list_observations(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<ListObservationsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let access = supervision_access_policy::resolve_observation_list_access(
        &context.tenant.pool,
        &context.actor,
    )
    .await?;
    let mut items = services::list_observations(
        &context.tenant.pool,
        access,
        SupervisionObservationFilter {
            academic_year_id: query.academic_year_id,
            academic_term_id: query.academic_term_id,
            cycle_id: query.cycle_id,
            status: query.status,
        },
    )
    .await?;
    redact_observations_results_for_actor(&context.actor, &mut items);

    Ok(Json(ApiResponse::ok(ItemsData::<SupervisionObservation> {
        items,
    }))
    .into_response())
}

#[utoipa::path(
    get,
    path = "/api/supervision/observations/{id}",
    operation_id = "getSupervisionObservation",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    responses(
        (status = 200, description = "Supervision observation", body = ApiResponse<SupervisionObservation>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation read access denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
)]
pub async fn get_observation(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let mut observation = services::get_observation(&context.tenant.pool, id).await?;
    let evaluator_user_ids = observation
        .evaluators
        .iter()
        .map(|evaluator| evaluator.evaluator_user_id)
        .collect::<Vec<_>>();
    supervision_access_policy::require_observation_read_access(
        &context.tenant.pool,
        &context.actor,
        observation.observed_user_id,
        &evaluator_user_ids,
    )
    .await?;
    redact_observation_results_for_actor(&context.actor, &mut observation);

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/supervision/observations/{id}/review",
    operation_id = "getSupervisionObservationReview",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    responses(
        (status = 200, description = "Supervision observation review", body = ApiResponse<crate::modules::supervision::models::SupervisionObservationReview>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation review access denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
)]
pub async fn get_observation_review(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let observation = services::get_observation(&context.tenant.pool, id).await?;
    let evaluator_user_ids = observation
        .evaluators
        .iter()
        .map(|evaluator| evaluator.evaluator_user_id)
        .collect::<Vec<_>>();

    if services::can_view_observation_results(observation.status, false) {
        supervision_access_policy::require_observation_read_access(
            &context.tenant.pool,
            &context.actor,
            observation.observed_user_id,
            &evaluator_user_ids,
        )
        .await?;
    } else if !supervision_access_policy::can_approve_school(&context.actor)
        && !supervision_access_policy::can_manage_school(&context.actor)
    {
        supervision_access_policy::require_observation_management_access(
            &context.tenant.pool,
            &context.actor,
            observation.observed_user_id,
        )
        .await?;
    }

    let review = services::get_observation_review(&context.tenant.pool, id).await?;

    Ok(Json(ApiResponse::ok(review)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/supervision/observations/{id}/evaluator-availability",
    operation_id = "getSupervisionEvaluatorAvailability",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    responses(
        (status = 200, description = "Available supervision evaluators", body = ApiResponse<ItemsData<SupervisionEvaluatorAvailability>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
)]
pub async fn evaluator_availability(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let observation = services::get_observation(&context.tenant.pool, id).await?;
    supervision_access_policy::require_observation_management_access(
        &context.tenant.pool,
        &context.actor,
        observation.observed_user_id,
    )
    .await?;

    let items = services::evaluator_availability(&context.tenant.pool, id).await?;

    Ok(Json(ApiResponse::ok(ItemsData::<
        SupervisionEvaluatorAvailability,
    > {
        items,
    }))
    .into_response())
}

#[utoipa::path(
    get,
    path = "/api/supervision/observations/{id}/timetable-options",
    operation_id = "getSupervisionObservationTimetableOptions",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    responses(
        (status = 200, description = "Timetable options for the observation", body = ApiResponse<ItemsData<crate::modules::academic::models::timetable::TimetableEntry>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
)]
pub async fn observation_timetable_options(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let observation = services::get_observation(&context.tenant.pool, id).await?;
    let can_edit_own_request = observation.observed_user_id == context.actor.user_id
        && services::teacher_can_edit_requested_observation(observation.status)
        && supervision_access_policy::can_request_own(&context.actor);

    if !can_edit_own_request {
        supervision_access_policy::require_observation_management_access(
            &context.tenant.pool,
            &context.actor,
            observation.observed_user_id,
        )
        .await?;
    }

    let items = services::observation_timetable_options(&context.tenant.pool, id).await?;

    Ok(Json(ApiResponse::ok(ItemsData { items })).into_response())
}

#[utoipa::path(
    post,
    path = "/api/supervision/observations/requests",
    operation_id = "requestSupervisionObservation",
    tag = "supervision",
    request_body = RequestSupervisionObservationRequest,
    responses(
        (status = 201, description = "Supervision observation requested", body = ApiResponse<SupervisionObservation>),
        (status = 400, description = "Invalid supervision request", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision request denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision cycle or timetable entry not found", body = ApiErrorResponse),
        (status = 409, description = "Supervision booking conflict", body = ApiErrorResponse)
    )
)]
pub async fn request_observation(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<RequestSupervisionObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    supervision_access_policy::require_request_own(&context.actor)?;

    let observation =
        services::request_observation(&context.tenant.pool, context.actor.user_id, payload).await?;
    let observation = redacted_observation_for_actor(&context.actor, observation);

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(observation))).into_response())
}

#[utoipa::path(
    patch,
    path = "/api/supervision/observations/{id}/request",
    operation_id = "updateRequestedSupervisionObservation",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    request_body = UpdateRequestedObservationRequest,
    responses(
        (status = 200, description = "Requested observation updated", body = ApiResponse<SupervisionObservation>),
        (status = 400, description = "Requested observation cannot be updated", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision request denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse),
        (status = 409, description = "Supervision booking conflict", body = ApiErrorResponse)
    )
)]
pub async fn update_requested_observation(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateRequestedObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    supervision_access_policy::require_request_own(&context.actor)?;

    let observation = services::update_requested_observation(
        &context.tenant.pool,
        context.actor.user_id,
        id,
        payload,
    )
    .await?;
    let observation = redacted_observation_for_actor(&context.actor, observation);

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/supervision/observations/{id}/request",
    operation_id = "cancelRequestedSupervisionObservation",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    responses(
        (status = 200, description = "Requested observation cancelled", body = ApiResponse<SupervisionObservation>),
        (status = 400, description = "Requested observation cannot be cancelled", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision request denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
)]
pub async fn cancel_requested_observation(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    supervision_access_policy::require_request_own(&context.actor)?;

    let observation =
        services::cancel_requested_observation(&context.tenant.pool, context.actor.user_id, id)
            .await?;
    let observation = redacted_observation_for_actor(&context.actor, observation);

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[utoipa::path(
    patch,
    path = "/api/supervision/observations/{id}",
    operation_id = "updateSupervisionObservation",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    request_body = UpdateSupervisionObservationRequest,
    responses(
        (status = 200, description = "Supervision observation updated", body = ApiResponse<SupervisionObservation>),
        (status = 400, description = "Observation cannot be updated", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse),
        (status = 409, description = "Supervision booking conflict", body = ApiErrorResponse)
    )
)]
pub async fn update_observation(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSupervisionObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let current = services::get_observation(&context.tenant.pool, id).await?;
    supervision_access_policy::require_observation_management_access(
        &context.tenant.pool,
        &context.actor,
        current.observed_user_id,
    )
    .await?;

    let observation =
        services::update_observation(&context.tenant.pool, context.actor.user_id, id, payload)
            .await?;
    let observation = redacted_observation_for_actor(&context.actor, observation);

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[utoipa::path(
    put,
    path = "/api/supervision/observations/{id}/evaluators",
    operation_id = "replaceSupervisionObservationEvaluators",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    request_body = ReplaceObservationEvaluatorsRequest,
    responses(
        (status = 200, description = "Supervision evaluators replaced", body = ApiResponse<SupervisionObservation>),
        (status = 400, description = "Invalid evaluator assignment", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse),
        (status = 409, description = "Evaluator schedule conflict", body = ApiErrorResponse)
    )
)]
pub async fn replace_observation_evaluators(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ReplaceObservationEvaluatorsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let current = services::get_observation(&context.tenant.pool, id).await?;
    supervision_access_policy::require_observation_management_access(
        &context.tenant.pool,
        &context.actor,
        current.observed_user_id,
    )
    .await?;

    let observation = services::replace_observation_evaluators(
        &context.tenant.pool,
        context.actor.user_id,
        id,
        payload,
    )
    .await?;
    let observation = redacted_observation_for_actor(&context.actor, observation);

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/supervision/observations/{id}/cancel",
    operation_id = "cancelSupervisionObservation",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    request_body = CancelObservationRequest,
    responses(
        (status = 200, description = "Supervision observation cancelled", body = ApiResponse<SupervisionObservation>),
        (status = 400, description = "Observation cannot be cancelled", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
)]
pub async fn cancel_observation(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<CancelObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let current = services::get_observation(&context.tenant.pool, id).await?;
    supervision_access_policy::require_observation_management_access(
        &context.tenant.pool,
        &context.actor,
        current.observed_user_id,
    )
    .await?;

    let observation =
        services::cancel_observation(&context.tenant.pool, context.actor.user_id, id, payload)
            .await?;
    let observation = redacted_observation_for_actor(&context.actor, observation);

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/supervision/observations/{id}/approve-request",
    operation_id = "approveSupervisionObservationRequest",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    request_body = ApproveObservationRequest,
    responses(
        (status = 200, description = "Supervision request approved", body = ApiResponse<SupervisionObservation>),
        (status = 400, description = "Supervision request cannot be approved", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse),
        (status = 409, description = "Evaluator schedule conflict", body = ApiErrorResponse)
    )
)]
pub async fn approve_observation_request(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ApproveObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let current = services::get_observation(&context.tenant.pool, id).await?;
    supervision_access_policy::require_observation_management_access(
        &context.tenant.pool,
        &context.actor,
        current.observed_user_id,
    )
    .await?;

    let observation = services::approve_observation_request(
        &context.tenant.pool,
        context.actor.user_id,
        id,
        payload,
    )
    .await?;
    let observation = redacted_observation_for_actor(&context.actor, observation);

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/supervision/observations/{id}/return-request",
    operation_id = "returnSupervisionObservationRequest",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    request_body = ReturnObservationRequest,
    responses(
        (status = 200, description = "Supervision request returned", body = ApiResponse<SupervisionObservation>),
        (status = 400, description = "Supervision request cannot be returned", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
)]
pub async fn return_observation_request(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ReturnObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let current = services::get_observation(&context.tenant.pool, id).await?;
    supervision_access_policy::require_observation_management_access(
        &context.tenant.pool,
        &context.actor,
        current.observed_user_id,
    )
    .await?;

    let observation = services::return_observation_request(
        &context.tenant.pool,
        context.actor.user_id,
        id,
        payload,
    )
    .await?;
    let observation = redacted_observation_for_actor(&context.actor, observation);

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/supervision/observations/{id}/evaluations/me/submit",
    operation_id = "submitMySupervisionEvaluation",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    request_body = SaveEvaluationRequest,
    responses(
        (status = 200, description = "Supervision evaluation submitted", body = ApiResponse<SupervisionObservation>),
        (status = 400, description = "Invalid supervision evaluation", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Evaluation submission denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
)]
pub async fn submit_my_evaluation(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<SaveEvaluationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    supervision_access_policy::require_evaluate_assigned(&context.actor)?;

    let observation =
        services::submit_my_evaluation(&context.tenant.pool, context.actor.user_id, id, payload)
            .await?;
    let observation = redacted_observation_for_actor(&context.actor, observation);

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/supervision/observations/{id}/certify",
    operation_id = "certifySupervisionObservation",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    responses(
        (status = 200, description = "Supervision observation certified", body = ApiResponse<SupervisionObservation>),
        (status = 400, description = "Observation cannot be certified", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
)]
pub async fn certify_observation(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let current = services::get_observation(&context.tenant.pool, id).await?;
    supervision_access_policy::require_observation_management_access(
        &context.tenant.pool,
        &context.actor,
        current.observed_user_id,
    )
    .await?;

    let observation =
        services::certify_observation(&context.tenant.pool, context.actor.user_id, id).await?;
    let observation = redacted_observation_for_actor(&context.actor, observation);

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/supervision/observations/{id}/approve",
    operation_id = "approveSupervisionObservation",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    responses(
        (status = 200, description = "Supervision observation approved", body = ApiResponse<SupervisionObservation>),
        (status = 400, description = "Observation cannot be approved", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic approval denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
)]
pub async fn approve_observation(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    supervision_access_policy::require_approve_school(&context.actor)?;

    let observation =
        services::approve_observation(&context.tenant.pool, context.actor.user_id, id).await?;
    let observation = redacted_observation_for_actor(&context.actor, observation);

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/supervision/observations/{id}/acknowledge",
    operation_id = "acknowledgeSupervisionObservation",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    request_body = AcknowledgeObservationRequest,
    responses(
        (status = 200, description = "Supervision observation acknowledged", body = ApiResponse<SupervisionObservation>),
        (status = 400, description = "Observation cannot be acknowledged", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Acknowledgement denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
)]
pub async fn acknowledge_observation(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AcknowledgeObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    supervision_access_policy::require_supervision_access(&context.actor)?;

    let observation =
        services::acknowledge_observation(&context.tenant.pool, context.actor.user_id, id, payload)
            .await?;
    let observation = redacted_observation_for_actor(&context.actor, observation);

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/supervision/reports/cycles/{id}/progress",
    operation_id = "getSupervisionCycleProgress",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision cycle ID")),
    responses(
        (status = 200, description = "Supervision cycle progress", body = ApiResponse<crate::modules::supervision::models::SupervisionCycleProgress>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision report access denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision cycle not found", body = ApiErrorResponse)
    )
)]
pub async fn cycle_progress(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    supervision_access_policy::require_school_report_access(&context.actor)?;

    let progress = services::cycle_progress(&context.tenant.pool, id).await?;

    Ok(Json(ApiResponse::ok(progress)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/supervision/reports/cycles/{id}/teacher-status",
    operation_id = "getSupervisionTeacherStatusOverview",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision cycle ID")),
    responses(
        (status = 200, description = "Teacher supervision status overview", body = ApiResponse<ItemsData<SupervisionTeacherStatusRow>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision report access denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision cycle not found", body = ApiErrorResponse)
    )
)]
pub async fn teacher_status_overview(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let access = supervision_access_policy::resolve_observation_list_access(
        &context.tenant.pool,
        &context.actor,
    )
    .await?;
    if !access.school && access.organization_unit_ids.is_empty() {
        return Err(AppError::Forbidden(
            "ไม่มีสิทธิ์ดูภาพรวมสถานะครูในรอบนิเทศ".to_string(),
        ));
    }

    let mut items = services::cycle_teacher_status(&context.tenant.pool, access, id).await?;
    redact_teacher_status_results_for_actor(&context.actor, &mut items);

    Ok(
        Json(ApiResponse::ok(ItemsData::<SupervisionTeacherStatusRow> {
            items,
        }))
        .into_response(),
    )
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/cycles", get(list_cycles).post(create_cycle))
        .route("/cycles/{id}", patch(update_cycle))
        .route("/templates", get(list_templates).post(create_template))
        .route("/templates/{id}", get(get_template).patch(update_template))
        .route("/observations", get(list_observations))
        .route("/observations/requests", post(request_observation))
        .route(
            "/observations/{id}",
            get(get_observation).patch(update_observation),
        )
        .route("/observations/{id}/review", get(get_observation_review))
        .route(
            "/observations/{id}/evaluator-availability",
            get(evaluator_availability),
        )
        .route(
            "/observations/{id}/timetable-options",
            get(observation_timetable_options),
        )
        .route(
            "/observations/{id}/evaluators",
            put(replace_observation_evaluators),
        )
        .route("/observations/{id}/cancel", post(cancel_observation))
        .route(
            "/observations/{id}/request",
            patch(update_requested_observation).delete(cancel_requested_observation),
        )
        .route(
            "/observations/{id}/approve-request",
            post(approve_observation_request),
        )
        .route(
            "/observations/{id}/return-request",
            post(return_observation_request),
        )
        .route(
            "/observations/{id}/evaluations/me/submit",
            post(submit_my_evaluation),
        )
        .route("/observations/{id}/certify", post(certify_observation))
        .route("/observations/{id}/approve", post(approve_observation))
        .route(
            "/observations/{id}/acknowledge",
            post(acknowledge_observation),
        )
        .route("/reports/cycles/{id}/progress", get(cycle_progress))
        .route(
            "/reports/cycles/{id}/teacher-status",
            get(teacher_status_overview),
        )
}
