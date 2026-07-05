use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::{
    CreateExamRoundRequest, GenerateSeatsRequest, ImportExamItemsRequest, PlaceExamSessionRequest,
    UpdateExamRoundRequest, UpsertDayRoomAssignmentRequest, UpsertExamDayRequest,
};
use crate::modules::academic::services::exam_schedule_service;
use crate::permissions::registry::codes;
use crate::utils::request_context::{
    actor_tenant_context, current_user_tenant_context_from_headers,
};
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct ExamRoundQuery {
    pub academic_semester_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct PersonalExamScheduleQuery {
    pub academic_semester_id: Option<Uuid>,
}

/// GET /api/academic/exam-schedules
pub async fn list_rounds(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ExamRoundQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL)?;

    let rounds = exam_schedule_service::list_rounds(&pool, query.academic_semester_id).await?;
    Ok(Json(ApiResponse::ok(rounds)).into_response())
}

/// POST /api/academic/exam-schedules
pub async fn create_round(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateExamRoundRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let round = exam_schedule_service::create_round(&pool, payload, actor.user_id).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(round))).into_response())
}

/// PATCH /api/academic/exam-schedules/{round_id}
pub async fn update_round(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<UpdateExamRoundRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let round =
        exam_schedule_service::update_round(&pool, round_id, payload, actor.user_id).await?;
    Ok(Json(ApiResponse::ok(round)).into_response())
}

/// GET /api/academic/exam-schedules/{round_id}
pub async fn get_workspace(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL)?;

    let workspace = exam_schedule_service::get_workspace(&pool, round_id).await?;
    Ok(Json(ApiResponse::ok(workspace)).into_response())
}

/// POST /api/academic/exam-schedules/{round_id}/import-items
pub async fn import_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<ImportExamItemsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let result =
        exam_schedule_service::import_exam_items(&pool, round_id, payload, actor.user_id).await?;
    Ok(Json(ApiResponse::ok(result)).into_response())
}

/// POST /api/academic/exam-schedules/{round_id}/days
pub async fn upsert_day(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<UpsertExamDayRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let day = exam_schedule_service::upsert_exam_day(&pool, round_id, payload).await?;
    Ok(Json(ApiResponse::ok(day)).into_response())
}

/// DELETE /api/academic/exam-schedules/days/{exam_day_id}
pub async fn delete_day(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(exam_day_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    exam_schedule_service::delete_exam_day(&pool, exam_day_id).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

/// GET /api/academic/exam-schedules/days/{exam_day_id}/room-assignments
pub async fn list_day_room_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(exam_day_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL)?;

    let assignments = exam_schedule_service::list_day_room_assignments(&pool, exam_day_id).await?;
    Ok(Json(ApiResponse::ok(assignments)).into_response())
}

/// POST /api/academic/exam-schedules/days/{exam_day_id}/room-assignments
pub async fn upsert_day_room_assignment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(exam_day_id): Path<Uuid>,
    Json(payload): Json<UpsertDayRoomAssignmentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let assignment = exam_schedule_service::upsert_day_room_assignment(
        &pool,
        exam_day_id,
        payload,
        actor.user_id,
    )
    .await?;
    Ok(Json(ApiResponse::ok(assignment)).into_response())
}

/// POST /api/academic/exam-schedules/room-assignments/{assignment_id}/seats
pub async fn generate_seats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(assignment_id): Path<Uuid>,
    Json(payload): Json<GenerateSeatsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let seats = exam_schedule_service::generate_seats_for_assignment(
        &pool,
        assignment_id,
        payload,
        actor.user_id,
    )
    .await?;
    Ok(Json(ApiResponse::ok(seats)).into_response())
}

/// POST /api/academic/exam-schedules/sessions
pub async fn place_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<PlaceExamSessionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let session = exam_schedule_service::place_exam_session(&pool, payload, actor.user_id).await?;
    Ok(Json(ApiResponse::ok(session)).into_response())
}

/// DELETE /api/academic/exam-schedules/sessions/{session_id}
pub async fn delete_session(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(session_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    exam_schedule_service::delete_exam_session(&pool, session_id, actor.user_id).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

/// POST /api/academic/exam-schedules/{round_id}/publish
pub async fn publish_round(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_PUBLISH_SCHOOL)?;

    let round = exam_schedule_service::publish_round(&pool, round_id, actor.user_id).await?;
    Ok(Json(ApiResponse::ok(round)).into_response())
}

/// GET /api/me/exam-schedules
pub async fn list_my_exam_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PersonalExamScheduleQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_headers(&state, &headers).await?;
    let schedule = exam_schedule_service::list_my_published_exam_schedule(
        &context.tenant.pool,
        context.user_id,
        query.academic_semester_id,
    )
    .await?;

    Ok(Json(ApiResponse::ok(schedule)).into_response())
}
