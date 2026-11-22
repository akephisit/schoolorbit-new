use axum::{
    extract::{
        rejection::{JsonRejection, QueryRejection},
        Extension, Path, Query, State,
    },
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[cfg(feature = "api-docs")]
use utoipa::{IntoParams, ToSchema};

use crate::state::SupervisionHttpState;
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
#[cfg(feature = "api-docs")]
use school_http::ApiErrorResponse;
use school_http::ApiResponse;
use school_http::HttpError as AppError;
use school_supervision::access_policy as supervision_access_policy;
use school_supervision::models::{
    AcknowledgeObservationRequest, ApproveObservationRequest, CancelObservationRequest,
    CreateSupervisionCycleRequest, CreateSupervisionTemplateRequest, DeleteSupervisionCycleRequest,
    ReplaceObservationEvaluatorsRequest, RequestSupervisionObservationRequest,
    ResubmitSupervisionObservationRequest, ReturnObservationRequest, SaveEvaluationRequest,
    SupervisionBookingOptionsQuery, SupervisionCycle, SupervisionCycleQuery,
    SupervisionObservation, SupervisionObservationFilter, SupervisionObservationStatus,
    SupervisionObservationTimetableQuery, SupervisionTeacherStatusRow, SupervisionTemplate,
    SupervisionTemplateSummary, UpdateRequestedObservationRequest, UpdateSupervisionCycleRequest,
    UpdateSupervisionObservationRequest, UpdateSupervisionTemplateRequest,
};
#[cfg(feature = "api-docs")]
use school_supervision::models::{
    SupervisionBookingOptions, SupervisionCycleDeletionImpact,
    SupervisionEvaluatorAvailabilityResponse,
};
use school_supervision::services;

#[derive(Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(IntoParams, ToSchema))]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Query))]
#[serde(rename_all = "camelCase")]
pub struct ListObservationsQuery {
    pub academic_year_id: Uuid,
    pub academic_term_id: Option<Uuid>,
    pub cycle_id: Option<Uuid>,
    pub status: Option<SupervisionObservationStatus>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ItemsData<T> {
    pub items: Vec<T>,
}

fn parse_request_body<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, AppError> {
    match payload {
        Ok(Json(payload)) => Ok(payload),
        Err(JsonRejection::JsonDataError(_)) => {
            Err(AppError::ValidationError("ข้อมูลคำขอไม่ถูกต้อง".into()))
        }
        Err(rejection) if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            Err(AppError::PayloadTooLarge)
        }
        Err(_) => Err(AppError::BadRequest("รูปแบบคำขอไม่ถูกต้อง".into())),
    }
}

fn parse_booking_query<T>(query: Result<Query<T>, QueryRejection>) -> Result<T, AppError> {
    query
        .map(|Query(query)| query)
        .map_err(|_| AppError::BadRequest("ตัวกรองช่วงเวลานิเทศไม่ถูกต้อง".into()))
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    get, path = "/api/supervision/booking-options", operation_id = "getSupervisionBookingOptions", tag = "supervision",
    params(SupervisionBookingOptionsQuery),
    responses(
        (status = 422, description = "Request validation failed", body = ApiErrorResponse),
        (status=200,description="Current own weekly teaching periods",body=ApiResponse<SupervisionBookingOptions>),
        (status=400,description="Invalid week or academic context",body=ApiErrorResponse),
        (status=401,description="Authentication required",body=ApiErrorResponse),
        (status=403,description="Own booking permission required",body=ApiErrorResponse)
    )
))]
pub async fn booking_options(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    query: Result<Query<SupervisionBookingOptionsQuery>, QueryRejection>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_request_own(&context.actor)?;
    let result = services::booking_options(
        &context.tenant.pool,
        context.actor.user_id,
        parse_booking_query(query)?,
    )
    .await?;
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    get,path="/api/supervision/cycles/{id}/deletion-impact",operation_id="getSupervisionCycleDeletionImpact",tag="supervision",
    params(("id"=Uuid,Path,description="Cycle to permanently delete")),
    responses(
        (status=200,description="Complete cycle-owned graph deletion preview",body=ApiResponse<SupervisionCycleDeletionImpact>),
        (status=401,description="Authentication required",body=ApiErrorResponse),
        (status=403,description="School supervision manage permission required",body=ApiErrorResponse),
        (status=404,description="Cycle not found",body=ApiErrorResponse),
        (status=409,description="Malformed graph or unknown dependency",body=ApiErrorResponse)
    )
))]
pub async fn cycle_deletion_impact(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_manage_school(&context.actor)?;
    let result = services::cycle_deletion_impact(&context.tenant.pool, id).await?;
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    delete,path="/api/supervision/cycles/{id}",operation_id="deleteSupervisionCycle",tag="supervision",
    params(("id"=Uuid,Path,description="Cycle to permanently delete")),request_body=DeleteSupervisionCycleRequest,
    responses(
        (status=200,description="Cycle and complete owned graph permanently deleted",body=ApiResponse<SupervisionCycleDeletionImpact>),
        (status=401,description="Authentication required",body=ApiErrorResponse),
        (status=403,description="School supervision manage permission required",body=ApiErrorResponse),
        (status=404,description="Cycle not found",body=ApiErrorResponse),
        (status=409,description="Stale preview, closed context or malformed graph",body=ApiErrorResponse)
    )
))]
pub async fn delete_cycle(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<DeleteSupervisionCycleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_manage_school(&context.actor)?;
    let result = services::delete_cycle(&context.tenant.pool, id, payload).await?;
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    get,
    path = "/api/supervision/templates/{id}/summary",
    operation_id = "getSupervisionTemplateSummary",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Template summary ID")),
    responses(
        (status = 200, description = "Template summary", body = ApiResponse<SupervisionTemplateSummary>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision access denied", body = ApiErrorResponse),
        (status = 404, description = "Template summary not found", body = ApiErrorResponse)
    )
))]
pub async fn get_template_summary(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_supervision_access(&context.actor)?;
    let item = services::get_template_summary(&context.tenant.pool, id).await?;
    Ok(Json(ApiResponse::ok(item)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    get,
    path = "/api/supervision/cycles/{id}",
    operation_id = "getSupervisionCycle",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision cycle ID")),
    responses(
        (status = 200, description = "Supervision cycle", body = ApiResponse<SupervisionCycle>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision access denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision cycle not found", body = ApiErrorResponse)
    )
))]
pub async fn get_cycle(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_supervision_access(&context.actor)?;
    let item = services::get_cycle(&context.tenant.pool, id).await?;
    Ok(Json(ApiResponse::ok(item)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
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
))]
pub async fn list_cycles(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<SupervisionCycleQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_supervision_access(&context.actor)?;

    let items = services::list_cycles(&context.tenant.pool, query).await?;

    Ok(Json(ApiResponse::ok(ItemsData::<SupervisionCycle> { items })).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    post,
    path = "/api/supervision/cycles",
    operation_id = "createSupervisionCycle",
    tag = "supervision",
    request_body = CreateSupervisionCycleRequest,
    responses(
        (status = 422, description = "Request validation failed", body = ApiErrorResponse),
        (status = 201, description = "Supervision cycle created", body = ApiResponse<SupervisionCycle>),
        (status = 409, description = "Academic year or term is closed", body = ApiErrorResponse),
        (status = 400, description = "Invalid supervision cycle", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision management denied", body = ApiErrorResponse)
    )
))]
pub async fn create_cycle(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    payload: Result<Json<CreateSupervisionCycleRequest>, JsonRejection>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_manage_school(&context.actor)?;

    let cycle = services::create_cycle(
        &context.tenant.pool,
        parse_request_body(payload)?,
        context.actor.user_id,
    )
    .await?;

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(cycle))).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    patch,
    path = "/api/supervision/cycles/{id}",
    operation_id = "updateSupervisionCycle",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision cycle ID")),
    request_body = UpdateSupervisionCycleRequest,
    responses(
        (status = 422, description = "Request validation failed", body = ApiErrorResponse),
        (status = 200, description = "Supervision cycle updated", body = ApiResponse<SupervisionCycle>),
        (status = 409, description = "Academic lifecycle or cycle context conflict", body = ApiErrorResponse),
        (status = 400, description = "Invalid supervision cycle", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision cycle not found", body = ApiErrorResponse)
    )
))]
pub async fn update_cycle(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    payload: Result<Json<UpdateSupervisionCycleRequest>, JsonRejection>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_manage_school(&context.actor)?;

    let cycle =
        services::update_cycle(&context.tenant.pool, id, parse_request_body(payload)?).await?;

    Ok(Json(ApiResponse::ok(cycle)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    get,
    path = "/api/supervision/templates",
    operation_id = "listSupervisionTemplates",
    tag = "supervision",
    responses(
        (status = 200, description = "Supervision templates", body = ApiResponse<ItemsData<SupervisionTemplate>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision access denied", body = ApiErrorResponse)
    )
))]
pub async fn list_templates(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_supervision_access(&context.actor)?;

    let items = services::list_templates(&context.tenant.pool).await?;

    Ok(Json(ApiResponse::ok(ItemsData::<SupervisionTemplate> { items })).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    get,
    path = "/api/supervision/templates/summaries",
    operation_id = "listSupervisionTemplateSummaries",
    tag = "supervision",
    responses(
        (status = 200, description = "Template summaries without rubric contents", body = ApiResponse<ItemsData<SupervisionTemplateSummary>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision access denied", body = ApiErrorResponse)
    )
))]
pub async fn list_template_summaries(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_supervision_access(&context.actor)?;
    let items = services::list_template_summaries(&context.tenant.pool).await?;
    Ok(
        Json(ApiResponse::ok(ItemsData::<SupervisionTemplateSummary> {
            items,
        }))
        .into_response(),
    )
}

#[cfg_attr(feature = "api-docs", utoipa::path(
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
))]
pub async fn create_template(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<CreateSupervisionTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_manage_school(&context.actor)?;

    let template =
        services::create_template(&context.tenant.pool, payload, context.actor.user_id).await?;

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(template))).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
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
))]
pub async fn get_template(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_supervision_access(&context.actor)?;

    let template = services::get_template(&context.tenant.pool, id).await?;

    Ok(Json(ApiResponse::ok(template)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
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
))]
pub async fn update_template(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSupervisionTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_manage_school(&context.actor)?;

    let template = services::update_template(&context.tenant.pool, id, payload).await?;

    Ok(Json(ApiResponse::ok(template)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
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
))]
pub async fn list_observations(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<ListObservationsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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
    services::redact_observations_results_for_actor(
        &context.tenant.pool,
        &context.actor,
        &mut items,
    )
    .await?;

    Ok(Json(ApiResponse::ok(ItemsData::<SupervisionObservation> {
        items,
    }))
    .into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
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
))]
pub async fn get_observation(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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
    services::redact_observations_results_for_actor(
        &context.tenant.pool,
        &context.actor,
        std::slice::from_mut(&mut observation),
    )
    .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    get,
    path = "/api/supervision/observations/{id}/review",
    operation_id = "getSupervisionObservationReview",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    responses(
        (status = 200, description = "Supervision observation review", body = ApiResponse<school_supervision::models::SupervisionObservationReview>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation review access denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
))]
pub async fn get_observation_review(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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

    let mut review = services::get_observation_review(&context.tenant.pool, id).await?;
    review.observation = services::redacted_observation_for_actor(
        &context.tenant.pool,
        &context.actor,
        review.observation,
    )
    .await?;

    Ok(Json(ApiResponse::ok(review)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    get,
    path = "/api/supervision/observations/{id}/evaluator-availability",
    operation_id = "getSupervisionEvaluatorAvailability",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    responses(
        (status = 422, description = "Request validation failed", body = ApiErrorResponse),
        (status = 200, description = "Available supervision evaluators", body = ApiResponse<SupervisionEvaluatorAvailabilityResponse>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
))]
pub async fn evaluator_availability(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let observation = services::get_observation(&context.tenant.pool, id).await?;
    supervision_access_policy::require_observation_management_access(
        &context.tenant.pool,
        &context.actor,
        observation.observed_user_id,
    )
    .await?;

    let response =
        services::evaluator_availability(&context.tenant.pool, &context.actor, id).await?;
    Ok(Json(ApiResponse::ok(response)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    get,
    path = "/api/supervision/observations/{id}/timetable-options",
    operation_id = "getSupervisionObservationTimetableOptions",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID"), SupervisionObservationTimetableQuery),
    responses(
        (status = 422, description = "Request validation failed", body = ApiErrorResponse),
        (status = 200, description = "Timetable block-group options for the observation", body = ApiResponse<SupervisionBookingOptions>),
        (status = 400, description = "Invalid timetable query", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
))]
pub async fn observation_timetable_options(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    query: Result<Query<SupervisionObservationTimetableQuery>, QueryRejection>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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

    let items = services::observation_timetable_options(
        &context.tenant.pool,
        id,
        parse_booking_query(query)?.week_start_date,
    )
    .await?;

    Ok(Json(ApiResponse::ok(items)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    post,
    path = "/api/supervision/observations/requests",
    operation_id = "requestSupervisionObservation",
    tag = "supervision",
    request_body = RequestSupervisionObservationRequest,
    responses(
        (status = 422, description = "Request validation failed", body = ApiErrorResponse),
        (status = 201, description = "Supervision observation requested", body = ApiResponse<SupervisionObservation>),
        (status = 400, description = "Invalid supervision request", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision request denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision cycle or timetable entry not found", body = ApiErrorResponse),
        (status = 409, description = "Supervision booking conflict", body = ApiErrorResponse)
    )
))]
pub async fn request_observation(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    payload: Result<Json<RequestSupervisionObservationRequest>, JsonRejection>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_request_own(&context.actor)?;

    let observation = services::request_observation(
        &context.tenant.pool,
        context.actor.user_id,
        parse_request_body(payload)?,
    )
    .await?;
    let observation =
        services::redacted_observation_for_actor(&context.tenant.pool, &context.actor, observation)
            .await?;

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(observation))).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    post,path="/api/supervision/observations/{id}/resubmit-request",operation_id="resubmitSupervisionObservationRequest",tag="supervision",
    params(("id"=Uuid,Path,description="Returned observation with a frozen schedule")),
    request_body=ResubmitSupervisionObservationRequest,
    responses(
        (status = 422, description = "Request validation failed", body = ApiErrorResponse),
        (status=200,description="Saved observation request resubmitted without changing periods",body=ApiResponse<SupervisionObservation>),
        (status=400,description="Malformed JSON",body=ApiErrorResponse),
        (status=401,description="Authentication required",body=ApiErrorResponse),
        (status=403,description="Own request permission required",body=ApiErrorResponse),
        (status=404,description="Observation not found",body=ApiErrorResponse),
        (status=409,description="Academic lifecycle or booking conflict",body=ApiErrorResponse)
    )
))]
pub async fn resubmit_observation_request(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    payload: Result<Json<ResubmitSupervisionObservationRequest>, JsonRejection>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_request_own(&context.actor)?;
    parse_request_body(payload)?;
    let observation =
        services::resubmit_observation_request(&context.tenant.pool, context.actor.user_id, id)
            .await?;
    let observation =
        services::redacted_observation_for_actor(&context.tenant.pool, &context.actor, observation)
            .await?;
    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    patch,
    path = "/api/supervision/observations/{id}/request",
    operation_id = "updateRequestedSupervisionObservation",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    request_body = UpdateRequestedObservationRequest,
    responses(
        (status = 422, description = "Request validation failed", body = ApiErrorResponse),
        (status = 200, description = "Requested observation updated", body = ApiResponse<SupervisionObservation>),
        (status = 400, description = "Requested observation cannot be updated", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision request denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse),
        (status = 409, description = "Supervision booking conflict", body = ApiErrorResponse)
    )
))]
pub async fn update_requested_observation(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    payload: Result<Json<UpdateRequestedObservationRequest>, JsonRejection>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_request_own(&context.actor)?;

    let observation = services::update_requested_observation(
        &context.tenant.pool,
        context.actor.user_id,
        id,
        parse_request_body(payload)?,
    )
    .await?;
    let observation =
        services::redacted_observation_for_actor(&context.tenant.pool, &context.actor, observation)
            .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    delete,
    path = "/api/supervision/observations/{id}/request",
    operation_id = "cancelRequestedSupervisionObservation",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    responses(
        (status = 200, description = "Requested observation cancelled", body = ApiResponse<SupervisionObservation>),
        (status = 409, description = "Academic year or term is closed", body = ApiErrorResponse),
        (status = 400, description = "Requested observation cannot be cancelled", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision request denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
))]
pub async fn cancel_requested_observation(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_request_own(&context.actor)?;

    let observation =
        services::cancel_requested_observation(&context.tenant.pool, context.actor.user_id, id)
            .await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    let observation =
        services::redacted_observation_for_actor(&context.tenant.pool, &context.actor, observation)
            .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    patch,
    path = "/api/supervision/observations/{id}",
    operation_id = "updateSupervisionObservation",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    request_body = UpdateSupervisionObservationRequest,
    responses(
        (status = 422, description = "Request validation failed", body = ApiErrorResponse),
        (status = 200, description = "Supervision observation updated", body = ApiResponse<SupervisionObservation>),
        (status = 400, description = "Observation cannot be updated", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse),
        (status = 409, description = "Supervision booking conflict", body = ApiErrorResponse)
    )
))]
pub async fn update_observation(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    payload: Result<Json<UpdateSupervisionObservationRequest>, JsonRejection>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let current = services::get_observation(&context.tenant.pool, id).await?;
    supervision_access_policy::require_observation_management_access(
        &context.tenant.pool,
        &context.actor,
        current.observed_user_id,
    )
    .await?;

    let observation = services::update_observation(
        &context.tenant.pool,
        context.actor.user_id,
        id,
        parse_request_body(payload)?,
    )
    .await?;
    let observation =
        services::redacted_observation_for_actor(&context.tenant.pool, &context.actor, observation)
            .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    put,
    path = "/api/supervision/observations/{id}/evaluators",
    operation_id = "replaceSupervisionObservationEvaluators",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    request_body = ReplaceObservationEvaluatorsRequest,
    responses(
        (status = 422, description = "Request validation failed", body = ApiErrorResponse),
        (status = 200, description = "Supervision evaluators replaced", body = ApiResponse<SupervisionObservation>),
        (status = 400, description = "Invalid evaluator assignment", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse),
        (status = 409, description = "Evaluator schedule conflict", body = ApiErrorResponse)
    )
))]
pub async fn replace_observation_evaluators(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    payload: Result<Json<ReplaceObservationEvaluatorsRequest>, JsonRejection>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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
        parse_request_body(payload)?,
    )
    .await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    let observation =
        services::redacted_observation_for_actor(&context.tenant.pool, &context.actor, observation)
            .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    post,
    path = "/api/supervision/observations/{id}/cancel",
    operation_id = "cancelSupervisionObservation",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    request_body = CancelObservationRequest,
    responses(
        (status = 200, description = "Supervision observation cancelled", body = ApiResponse<SupervisionObservation>),
        (status = 409, description = "Academic year or term is closed", body = ApiErrorResponse),
        (status = 400, description = "Observation cannot be cancelled", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
))]
pub async fn cancel_observation(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<CancelObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    let observation =
        services::redacted_observation_for_actor(&context.tenant.pool, &context.actor, observation)
            .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    post,
    path = "/api/supervision/observations/{id}/approve-request",
    operation_id = "approveSupervisionObservationRequest",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    request_body = ApproveObservationRequest,
    responses(
        (status = 422, description = "Request validation failed", body = ApiErrorResponse),
        (status = 200, description = "Supervision request approved", body = ApiResponse<SupervisionObservation>),
        (status = 400, description = "Supervision request cannot be approved", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse),
        (status = 409, description = "Evaluator schedule conflict", body = ApiErrorResponse)
    )
))]
pub async fn approve_observation_request(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    payload: Result<Json<ApproveObservationRequest>, JsonRejection>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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
        parse_request_body(payload)?,
    )
    .await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    let observation =
        services::redacted_observation_for_actor(&context.tenant.pool, &context.actor, observation)
            .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    post,
    path = "/api/supervision/observations/{id}/return-request",
    operation_id = "returnSupervisionObservationRequest",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    request_body = ReturnObservationRequest,
    responses(
        (status = 200, description = "Supervision request returned", body = ApiResponse<SupervisionObservation>),
        (status = 409, description = "Academic year or term is closed", body = ApiErrorResponse),
        (status = 400, description = "Supervision request cannot be returned", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
))]
pub async fn return_observation_request(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<ReturnObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    let observation =
        services::redacted_observation_for_actor(&context.tenant.pool, &context.actor, observation)
            .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    post,
    path = "/api/supervision/observations/{id}/evaluations/me/submit",
    operation_id = "submitMySupervisionEvaluation",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    request_body = SaveEvaluationRequest,
    responses(
        (status = 200, description = "Supervision evaluation submitted", body = ApiResponse<SupervisionObservation>),
        (status = 409, description = "Academic year or term is closed", body = ApiErrorResponse),
        (status = 400, description = "Invalid supervision evaluation", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Evaluation submission denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
))]
pub async fn submit_my_evaluation(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<SaveEvaluationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_evaluate_assigned(&context.actor)?;

    let observation =
        services::submit_my_evaluation(&context.tenant.pool, context.actor.user_id, id, payload)
            .await?;
    let observation =
        services::redacted_observation_for_actor(&context.tenant.pool, &context.actor, observation)
            .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    post,
    path = "/api/supervision/observations/{id}/certify",
    operation_id = "certifySupervisionObservation",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    responses(
        (status = 200, description = "Supervision observation certified", body = ApiResponse<SupervisionObservation>),
        (status = 409, description = "Academic year or term is closed", body = ApiErrorResponse),
        (status = 400, description = "Observation cannot be certified", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Observation management denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
))]
pub async fn certify_observation(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let current = services::get_observation(&context.tenant.pool, id).await?;
    supervision_access_policy::require_observation_management_access(
        &context.tenant.pool,
        &context.actor,
        current.observed_user_id,
    )
    .await?;

    let observation =
        services::certify_observation(&context.tenant.pool, context.actor.user_id, id).await?;
    let observation =
        services::redacted_observation_for_actor(&context.tenant.pool, &context.actor, observation)
            .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    post,
    path = "/api/supervision/observations/{id}/approve",
    operation_id = "approveSupervisionObservation",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    responses(
        (status = 200, description = "Supervision observation approved", body = ApiResponse<SupervisionObservation>),
        (status = 409, description = "Academic year or term is closed", body = ApiErrorResponse),
        (status = 400, description = "Observation cannot be approved", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic approval denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
))]
pub async fn approve_observation(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_approve_school(&context.actor)?;

    let observation =
        services::approve_observation(&context.tenant.pool, context.actor.user_id, id).await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    let observation =
        services::redacted_observation_for_actor(&context.tenant.pool, &context.actor, observation)
            .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    post,
    path = "/api/supervision/observations/{id}/acknowledge",
    operation_id = "acknowledgeSupervisionObservation",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision observation ID")),
    request_body = AcknowledgeObservationRequest,
    responses(
        (status = 200, description = "Supervision observation acknowledged", body = ApiResponse<SupervisionObservation>),
        (status = 409, description = "Academic year or term is closed", body = ApiErrorResponse),
        (status = 400, description = "Observation cannot be acknowledged", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Acknowledgement denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision observation not found", body = ApiErrorResponse)
    )
))]
pub async fn acknowledge_observation(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AcknowledgeObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_supervision_access(&context.actor)?;

    let observation =
        services::acknowledge_observation(&context.tenant.pool, context.actor.user_id, id, payload)
            .await?;
    let observation =
        services::redacted_observation_for_actor(&context.tenant.pool, &context.actor, observation)
            .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    get,
    path = "/api/supervision/reports/cycles/{id}/progress",
    operation_id = "getSupervisionCycleProgress",
    tag = "supervision",
    params(("id" = Uuid, Path, description = "Supervision cycle ID")),
    responses(
        (status = 200, description = "Supervision cycle progress", body = ApiResponse<school_supervision::models::SupervisionCycleProgress>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Supervision report access denied", body = ApiErrorResponse),
        (status = 404, description = "Supervision cycle not found", body = ApiErrorResponse)
    )
))]
pub async fn cycle_progress(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    supervision_access_policy::require_school_report_access(&context.actor)?;

    let progress = services::cycle_progress(&context.tenant.pool, &context.actor, id).await?;

    Ok(Json(ApiResponse::ok(progress)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
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
))]
pub async fn teacher_status_overview(
    State(state): State<SupervisionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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
    services::redact_teacher_status_results_for_actor(
        &context.tenant.pool,
        &context.actor,
        &mut items,
    )
    .await?;

    Ok(
        Json(ApiResponse::ok(ItemsData::<SupervisionTeacherStatusRow> {
            items,
        }))
        .into_response(),
    )
}
