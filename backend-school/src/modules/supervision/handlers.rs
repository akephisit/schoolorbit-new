use axum::http::HeaderMap;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, patch, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::supervision::models::{
    AcknowledgeObservationRequest, ApproveObservationRequest, CancelObservationRequest,
    CreateSupervisionCycleRequest, CreateSupervisionTemplateRequest,
    ReplaceObservationEvaluatorsRequest, RequestSupervisionObservationRequest,
    ReturnObservationRequest, SaveEvaluationRequest, SupervisionCycle, SupervisionObservation,
    SupervisionObservationFilter, SupervisionObservationStatus, SupervisionTemplate,
    UpdateRequestedObservationRequest, UpdateSupervisionCycleRequest,
    UpdateSupervisionObservationRequest, UpdateSupervisionTemplateRequest,
};
use crate::modules::supervision::services;
use crate::policies::supervision_access_policy;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListObservationsQuery {
    pub cycle_id: Option<Uuid>,
    pub status: Option<SupervisionObservationStatus>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ItemsData<T> {
    items: Vec<T>,
}

pub async fn list_cycles(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    supervision_access_policy::require_supervision_access(&context.actor)?;

    let items = services::list_cycles(&context.tenant.pool).await?;

    Ok(Json(ApiResponse::ok(ItemsData::<SupervisionCycle> { items })).into_response())
}

pub async fn create_cycle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateSupervisionCycleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    supervision_access_policy::require_manage_school(&context.actor)?;

    let cycle =
        services::create_cycle(&context.tenant.pool, payload, context.actor.user_id).await?;

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(cycle))).into_response())
}

pub async fn update_cycle(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSupervisionCycleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    supervision_access_policy::require_manage_school(&context.actor)?;

    let cycle = services::update_cycle(&context.tenant.pool, id, payload).await?;

    Ok(Json(ApiResponse::ok(cycle)).into_response())
}

pub async fn list_templates(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    supervision_access_policy::require_supervision_access(&context.actor)?;

    let items = services::list_templates(&context.tenant.pool).await?;

    Ok(Json(ApiResponse::ok(ItemsData::<SupervisionTemplate> { items })).into_response())
}

pub async fn create_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateSupervisionTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    supervision_access_policy::require_manage_school(&context.actor)?;

    let template =
        services::create_template(&context.tenant.pool, payload, context.actor.user_id).await?;

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(template))).into_response())
}

pub async fn get_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    supervision_access_policy::require_supervision_access(&context.actor)?;

    let template = services::get_template(&context.tenant.pool, id).await?;

    Ok(Json(ApiResponse::ok(template)).into_response())
}

pub async fn update_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSupervisionTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    supervision_access_policy::require_manage_school(&context.actor)?;

    let template = services::update_template(&context.tenant.pool, id, payload).await?;

    Ok(Json(ApiResponse::ok(template)).into_response())
}

pub async fn list_observations(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListObservationsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let access = supervision_access_policy::resolve_observation_list_access(
        &context.tenant.pool,
        &context.actor,
    )
    .await?;
    let items = services::list_observations(
        &context.tenant.pool,
        access,
        SupervisionObservationFilter {
            cycle_id: query.cycle_id,
            status: query.status,
        },
    )
    .await?;

    Ok(Json(ApiResponse::ok(ItemsData::<SupervisionObservation> {
        items,
    }))
    .into_response())
}

pub async fn get_observation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let observation = services::get_observation(&context.tenant.pool, id).await?;
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

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

pub async fn request_observation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<RequestSupervisionObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    supervision_access_policy::require_request_own(&context.actor)?;

    let observation =
        services::request_observation(&context.tenant.pool, context.actor.user_id, payload).await?;

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(observation))).into_response())
}

pub async fn update_requested_observation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateRequestedObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    supervision_access_policy::require_request_own(&context.actor)?;

    let observation = services::update_requested_observation(
        &context.tenant.pool,
        context.actor.user_id,
        id,
        payload,
    )
    .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

pub async fn cancel_requested_observation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    supervision_access_policy::require_request_own(&context.actor)?;

    let observation =
        services::cancel_requested_observation(&context.tenant.pool, context.actor.user_id, id)
            .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

pub async fn update_observation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSupervisionObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
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

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

pub async fn replace_observation_evaluators(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<ReplaceObservationEvaluatorsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
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

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

pub async fn cancel_observation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<CancelObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
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

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

pub async fn approve_observation_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<ApproveObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
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

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

pub async fn return_observation_request(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<ReturnObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
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

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

pub async fn save_my_evaluation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<SaveEvaluationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    supervision_access_policy::require_evaluate_assigned(&context.actor)?;

    let observation =
        services::save_my_evaluation(&context.tenant.pool, context.actor.user_id, id, payload)
            .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

pub async fn submit_my_evaluation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<SaveEvaluationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    supervision_access_policy::require_evaluate_assigned(&context.actor)?;

    let observation =
        services::submit_my_evaluation(&context.tenant.pool, context.actor.user_id, id, payload)
            .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

pub async fn submit_observation_for_review(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let current = services::get_observation(&context.tenant.pool, id).await?;
    supervision_access_policy::require_observation_management_access(
        &context.tenant.pool,
        &context.actor,
        current.observed_user_id,
    )
    .await?;

    let observation =
        services::submit_observation_for_review(&context.tenant.pool, context.actor.user_id, id)
            .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

pub async fn approve_observation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    supervision_access_policy::require_approve_school(&context.actor)?;

    let observation =
        services::approve_observation(&context.tenant.pool, context.actor.user_id, id).await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

pub async fn return_observation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<ReturnObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    supervision_access_policy::require_approve_school(&context.actor)?;

    let observation =
        services::return_observation(&context.tenant.pool, context.actor.user_id, id, payload)
            .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

pub async fn publish_observation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    supervision_access_policy::require_approve_school(&context.actor)?;

    let observation =
        services::publish_observation(&context.tenant.pool, context.actor.user_id, id).await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

pub async fn acknowledge_observation(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<AcknowledgeObservationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    supervision_access_policy::require_supervision_access(&context.actor)?;

    let observation =
        services::acknowledge_observation(&context.tenant.pool, context.actor.user_id, id, payload)
            .await?;

    Ok(Json(ApiResponse::ok(observation)).into_response())
}

pub async fn cycle_progress(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    supervision_access_policy::require_school_report_access(&context.actor)?;

    let progress = services::cycle_progress(&context.tenant.pool, id).await?;

    Ok(Json(ApiResponse::ok(progress)).into_response())
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
        .route("/observations/{id}/evaluations/me", put(save_my_evaluation))
        .route(
            "/observations/{id}/evaluations/me/submit",
            post(submit_my_evaluation),
        )
        .route(
            "/observations/{id}/submit-review",
            post(submit_observation_for_review),
        )
        .route("/observations/{id}/approve", post(approve_observation))
        .route("/observations/{id}/return", post(return_observation))
        .route("/observations/{id}/publish", post(publish_observation))
        .route(
            "/observations/{id}/acknowledge",
            post(acknowledge_observation),
        )
        .route("/reports/cycles/{id}/progress", get(cycle_progress))
}
