use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::{
    CreateExamRoundRequest, DayRoomAssignmentView, ExamDayDetail, ExamInvigilatorStaffOption,
    ExamInvigilatorWorkspace, ExamRound, ExamScheduleWorkspace, ExamSessionView, ExamSourcePreview,
    GenerateSeatsRequest, PlaceExamSessionRequest, SeatAssignmentView, SyncExamSourcesRequest,
    SyncExamSourcesResult, UpdateExamInvigilatorsRequest, UpdateExamRoundRequest,
    UpsertDayRoomAssignmentRequest, UpsertExamDayRequest,
};
use crate::modules::academic::services::exam_schedule_service;
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::permissions::registry::codes;
use crate::utils::request_context::{
    actor_tenant_context_from_session, current_user_tenant_context_from_session,
};
use crate::AppState;
use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExamRoundQuery {
    pub academic_term_id: Uuid,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersonalExamScheduleQuery {
    pub academic_term_id: Uuid,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InvigilatorStaffOptionsQuery {
    pub search: Option<String>,
    pub limit: Option<i64>,
}

/// GET /api/academic/exam-schedules
#[utoipa::path(
    get,
    path = "/api/academic/exam-schedules",
    operation_id = "listExamRounds",
    tag = "academic",
    params(ExamRoundQuery),
    responses(
        (status = 200, description = "Exam rounds for the selected term", body = ApiResponse<Vec<ExamRound>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_rounds(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<ExamRoundQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL)?;

    let rounds = exam_schedule_service::list_rounds(&pool, query.academic_term_id).await?;
    Ok(Json(ApiResponse::ok(rounds)).into_response())
}

/// POST /api/academic/exam-schedules
#[utoipa::path(
    post,
    path = "/api/academic/exam-schedules",
    operation_id = "createExamRound",
    tag = "academic",
    request_body = CreateExamRoundRequest,
    responses(
        (status = 201, description = "Exam round created", body = ApiResponse<ExamRound>),
        (status = 400, description = "Invalid exam round", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn create_round(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<CreateExamRoundRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let round = exam_schedule_service::create_round(&pool, payload, actor.user_id).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(round))).into_response())
}

/// PATCH /api/academic/exam-schedules/{round_id}
#[utoipa::path(
    patch,
    path = "/api/academic/exam-schedules/{round_id}",
    operation_id = "updateExamRound",
    tag = "academic",
    params(("round_id" = Uuid, Path, description = "Exam round ID")),
    request_body = UpdateExamRoundRequest,
    responses(
        (status = 200, description = "Exam round updated", body = ApiResponse<ExamRound>),
        (status = 400, description = "Invalid exam round", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse),
        (status = 404, description = "Exam round not found", body = ApiErrorResponse)
    )
)]
pub async fn update_round(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<UpdateExamRoundRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let round =
        exam_schedule_service::update_round(&pool, round_id, payload, actor.user_id).await?;
    Ok(Json(ApiResponse::ok(round)).into_response())
}

/// GET /api/academic/exam-schedules/{round_id}
#[utoipa::path(
    get,
    path = "/api/academic/exam-schedules/{round_id}",
    operation_id = "getExamScheduleWorkspace",
    tag = "academic",
    params(("round_id" = Uuid, Path, description = "Exam round ID")),
    responses(
        (status = 200, description = "Exam scheduling workspace", body = ApiResponse<ExamScheduleWorkspace>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse),
        (status = 404, description = "Exam round not found", body = ApiErrorResponse)
    )
)]
pub async fn get_workspace(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL)?;

    let workspace = exam_schedule_service::get_workspace(&pool, round_id).await?;
    Ok(Json(ApiResponse::ok(workspace)).into_response())
}

/// GET /api/academic/exam-schedules/{round_id}/source-preview
#[utoipa::path(
    get,
    path = "/api/academic/exam-schedules/{round_id}/source-preview",
    operation_id = "previewExamSources",
    tag = "academic",
    params(("round_id" = Uuid, Path, description = "Exam round ID")),
    responses(
        (status = 200, description = "Assessment source change preview", body = ApiResponse<ExamSourcePreview>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn preview_sources(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL)?;

    let preview = exam_schedule_service::preview_exam_sources(&pool, round_id).await?;
    Ok(Json(ApiResponse::ok(preview)).into_response())
}

/// POST /api/academic/exam-schedules/{round_id}/source-sync
#[utoipa::path(
    post,
    path = "/api/academic/exam-schedules/{round_id}/source-sync",
    operation_id = "syncExamSources",
    tag = "academic",
    params(("round_id" = Uuid, Path, description = "Exam round ID")),
    request_body = SyncExamSourcesRequest,
    responses(
        (status = 200, description = "Selected assessment source changes synchronized", body = ApiResponse<SyncExamSourcesResult>),
        (status = 400, description = "Source synchronization rejected", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse),
        (status = 409, description = "Source preview is stale or conflicts with placement", body = ApiErrorResponse)
    )
)]
pub async fn sync_sources(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<SyncExamSourcesRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let result =
        exam_schedule_service::sync_exam_sources(&pool, round_id, actor.user_id, payload).await?;
    Ok(Json(ApiResponse::ok(result)).into_response())
}

/// POST /api/academic/exam-schedules/{round_id}/days
#[utoipa::path(
    post,
    path = "/api/academic/exam-schedules/{round_id}/days",
    operation_id = "upsertExamDay",
    tag = "academic",
    params(("round_id" = Uuid, Path, description = "Exam round ID")),
    request_body = UpsertExamDayRequest,
    responses(
        (status = 200, description = "Exam day saved", body = ApiResponse<ExamDayDetail>),
        (status = 400, description = "Invalid exam day", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn upsert_day(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<UpsertExamDayRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let day = exam_schedule_service::upsert_exam_day(&pool, round_id, payload).await?;
    Ok(Json(ApiResponse::ok(day)).into_response())
}

/// PATCH /api/academic/exam-schedules/days/{exam_day_id}
#[utoipa::path(
    patch,
    path = "/api/academic/exam-schedules/days/{exam_day_id}",
    operation_id = "updateExamDay",
    tag = "academic",
    params(("exam_day_id" = Uuid, Path, description = "Exam day ID")),
    request_body = UpsertExamDayRequest,
    responses(
        (status = 200, description = "Exam day updated", body = ApiResponse<ExamDayDetail>),
        (status = 400, description = "Invalid exam day", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse),
        (status = 404, description = "Exam day not found", body = ApiErrorResponse)
    )
)]
pub async fn update_day(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(exam_day_id): Path<Uuid>,
    Json(payload): Json<UpsertExamDayRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let day = exam_schedule_service::update_exam_day(&pool, exam_day_id, payload).await?;
    Ok(Json(ApiResponse::ok(day)).into_response())
}

/// DELETE /api/academic/exam-schedules/days/{exam_day_id}
#[utoipa::path(
    delete,
    path = "/api/academic/exam-schedules/days/{exam_day_id}",
    operation_id = "deleteExamDay",
    tag = "academic",
    params(("exam_day_id" = Uuid, Path, description = "Exam day ID")),
    responses(
        (status = 200, description = "Exam day deleted", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse),
        (status = 404, description = "Exam day not found", body = ApiErrorResponse)
    )
)]
pub async fn delete_day(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(exam_day_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    exam_schedule_service::delete_exam_day(&pool, exam_day_id).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

/// GET /api/academic/exam-schedules/days/{exam_day_id}/room-assignments
#[utoipa::path(
    get,
    path = "/api/academic/exam-schedules/days/{exam_day_id}/room-assignments",
    operation_id = "listExamDayRoomAssignments",
    tag = "academic",
    params(("exam_day_id" = Uuid, Path, description = "Exam day ID")),
    responses(
        (status = 200, description = "Exam room assignments", body = ApiResponse<Vec<DayRoomAssignmentView>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_day_room_assignments(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(exam_day_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL)?;

    let assignments = exam_schedule_service::list_day_room_assignments(&pool, exam_day_id).await?;
    Ok(Json(ApiResponse::ok(assignments)).into_response())
}

/// GET /api/academic/exam-schedules/{round_id}/invigilators
#[utoipa::path(
    get,
    path = "/api/academic/exam-schedules/{round_id}/invigilators",
    operation_id = "getExamInvigilatorWorkspace",
    tag = "academic",
    params(("round_id" = Uuid, Path, description = "Exam round ID")),
    responses(
        (status = 200, description = "Exam invigilation workspace", body = ApiResponse<ExamInvigilatorWorkspace>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn get_invigilator_workspace(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_READ_SCHOOL)?;

    let workspace = exam_schedule_service::get_invigilator_workspace(&pool, round_id).await?;
    Ok(Json(ApiResponse::ok(workspace)).into_response())
}

/// GET /api/academic/exam-schedules/{round_id}/invigilator-staff-options
#[utoipa::path(
    get,
    path = "/api/academic/exam-schedules/{round_id}/invigilator-staff-options",
    operation_id = "listExamInvigilatorStaffOptions",
    tag = "academic",
    params(
        ("round_id" = Uuid, Path, description = "Exam round ID"),
        InvigilatorStaffOptionsQuery
    ),
    responses(
        (status = 200, description = "Staff options for invigilation", body = ApiResponse<Vec<ExamInvigilatorStaffOption>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn get_invigilator_staff_options(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
    Query(query): Query<InvigilatorStaffOptionsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let options = exam_schedule_service::list_invigilator_staff_options(
        &pool,
        round_id,
        query.search,
        query.limit,
    )
    .await?;
    Ok(Json(ApiResponse::ok(options)).into_response())
}

/// POST /api/academic/exam-schedules/days/{exam_day_id}/room-assignments
#[utoipa::path(
    post,
    path = "/api/academic/exam-schedules/days/{exam_day_id}/room-assignments",
    operation_id = "upsertExamDayRoomAssignment",
    tag = "academic",
    params(("exam_day_id" = Uuid, Path, description = "Exam day ID")),
    request_body = UpsertDayRoomAssignmentRequest,
    responses(
        (status = 200, description = "Exam room assignment saved", body = ApiResponse<DayRoomAssignmentView>),
        (status = 400, description = "Invalid room assignment", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn upsert_day_room_assignment(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(exam_day_id): Path<Uuid>,
    Json(payload): Json<UpsertDayRoomAssignmentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
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

/// PUT /api/academic/exam-schedules/room-assignments/{assignment_id}/invigilators
#[utoipa::path(
    put,
    path = "/api/academic/exam-schedules/room-assignments/{assignment_id}/invigilators",
    operation_id = "updateExamAssignmentInvigilators",
    tag = "academic",
    params(("assignment_id" = Uuid, Path, description = "Room assignment ID")),
    request_body = UpdateExamInvigilatorsRequest,
    responses(
        (status = 200, description = "Invigilators updated", body = ApiResponse<DayRoomAssignmentView>),
        (status = 400, description = "Invalid invigilator assignment", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn update_assignment_invigilators(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(assignment_id): Path<Uuid>,
    Json(payload): Json<UpdateExamInvigilatorsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let assignment = exam_schedule_service::update_assignment_invigilators(
        &pool,
        assignment_id,
        payload,
        actor.user_id,
    )
    .await?;
    Ok(Json(ApiResponse::ok(assignment)).into_response())
}

/// PUT /api/academic/exam-schedules/room-assignments/{assignment_id}/invigilators/{staff_id}
#[utoipa::path(
    put,
    path = "/api/academic/exam-schedules/room-assignments/{assignment_id}/invigilators/{staff_id}",
    operation_id = "assignExamAssignmentInvigilator",
    tag = "academic",
    params(
        ("assignment_id" = Uuid, Path, description = "Room assignment ID"),
        ("staff_id" = Uuid, Path, description = "Staff ID")
    ),
    responses(
        (status = 200, description = "Invigilator assigned", body = ApiResponse<ExamInvigilatorWorkspace>),
        (status = 400, description = "Invalid invigilator assignment", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn assign_assignment_invigilator(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((assignment_id, staff_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let workspace = exam_schedule_service::assign_invigilator_to_assignment(
        &pool,
        assignment_id,
        staff_id,
        actor.user_id,
    )
    .await?;
    Ok(Json(ApiResponse::ok(workspace)).into_response())
}

/// DELETE /api/academic/exam-schedules/room-assignments/{assignment_id}/invigilators/{staff_id}
#[utoipa::path(
    delete,
    path = "/api/academic/exam-schedules/room-assignments/{assignment_id}/invigilators/{staff_id}",
    operation_id = "removeExamAssignmentInvigilator",
    tag = "academic",
    params(
        ("assignment_id" = Uuid, Path, description = "Room assignment ID"),
        ("staff_id" = Uuid, Path, description = "Staff ID")
    ),
    responses(
        (status = 200, description = "Invigilator removed", body = ApiResponse<ExamInvigilatorWorkspace>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn remove_assignment_invigilator(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((assignment_id, staff_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let workspace = exam_schedule_service::remove_invigilator_from_assignment(
        &pool,
        assignment_id,
        staff_id,
        actor.user_id,
    )
    .await?;
    Ok(Json(ApiResponse::ok(workspace)).into_response())
}

/// POST /api/academic/exam-schedules/room-assignments/{assignment_id}/seats
#[utoipa::path(
    post,
    path = "/api/academic/exam-schedules/room-assignments/{assignment_id}/seats",
    operation_id = "generateExamSeats",
    tag = "academic",
    params(("assignment_id" = Uuid, Path, description = "Room assignment ID")),
    request_body = GenerateSeatsRequest,
    responses(
        (status = 200, description = "Exam seats generated", body = ApiResponse<Vec<SeatAssignmentView>>),
        (status = 400, description = "Seat generation rejected", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn generate_seats(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(assignment_id): Path<Uuid>,
    Json(payload): Json<GenerateSeatsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
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
#[utoipa::path(
    post,
    path = "/api/academic/exam-schedules/sessions",
    operation_id = "placeExamSession",
    tag = "academic",
    request_body = PlaceExamSessionRequest,
    responses(
        (status = 200, description = "Exam session placed", body = ApiResponse<ExamSessionView>),
        (status = 400, description = "Exam session conflicts or is invalid", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn place_session(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<PlaceExamSessionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    let session = exam_schedule_service::place_exam_session(&pool, payload, actor.user_id).await?;
    Ok(Json(ApiResponse::ok(session)).into_response())
}

/// DELETE /api/academic/exam-schedules/sessions/{session_id}
#[utoipa::path(
    delete,
    path = "/api/academic/exam-schedules/sessions/{session_id}",
    operation_id = "deleteExamSession",
    tag = "academic",
    params(("session_id" = Uuid, Path, description = "Exam session ID")),
    responses(
        (status = 200, description = "Exam session deleted", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse),
        (status = 404, description = "Exam session not found", body = ApiErrorResponse)
    )
)]
pub async fn delete_session(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(session_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_MANAGE_SCHOOL)?;

    exam_schedule_service::delete_exam_session(&pool, session_id, actor.user_id).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

/// POST /api/academic/exam-schedules/{round_id}/publish
#[utoipa::path(
    post,
    path = "/api/academic/exam-schedules/{round_id}/publish",
    operation_id = "publishExamRound",
    tag = "academic",
    params(("round_id" = Uuid, Path, description = "Exam round ID")),
    responses(
        (status = 200, description = "Exam round published", body = ApiResponse<ExamRound>),
        (status = 400, description = "Exam round is not ready", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse),
        (status = 404, description = "Exam round not found", body = ApiErrorResponse)
    )
)]
pub async fn publish_round(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_EXAM_SCHEDULE_PUBLISH_SCHOOL)?;

    let round = exam_schedule_service::publish_round(&pool, round_id, actor.user_id).await?;
    Ok(Json(ApiResponse::ok(round)).into_response())
}

/// GET /api/me/exam-schedules
#[utoipa::path(
    get,
    path = "/api/me/exam-schedules",
    operation_id = "listMyExamSchedules",
    tag = "academic",
    params(PersonalExamScheduleQuery),
    responses(
        (status = 200, description = "Current student's published exam schedules", body = ApiResponse<Vec<crate::modules::academic::models::exam_schedule::PersonalExamScheduleRound>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Active student account required", body = ApiErrorResponse)
    )
)]
pub async fn list_my_exam_schedule(
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<PersonalExamScheduleQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_session(&session);
    let schedule = exam_schedule_service::list_my_published_exam_schedule(
        &context.tenant.pool,
        context.user_id,
        query.academic_term_id,
    )
    .await?;

    Ok(Json(ApiResponse::ok(schedule)).into_response())
}

/// GET /api/staff/exam-schedules
#[utoipa::path(
    get,
    path = "/api/staff/exam-schedules",
    operation_id = "listStaffExamSchedules",
    tag = "academic",
    params(PersonalExamScheduleQuery),
    responses(
        (status = 200, description = "Published school exam schedules for staff", body = ApiResponse<Vec<crate::modules::academic::models::exam_schedule::StaffPublishedExamScheduleRound>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Active staff account required", body = ApiErrorResponse)
    )
)]
pub async fn list_staff_exam_schedule(
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<PersonalExamScheduleQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_session(&session);
    let schedule = exam_schedule_service::list_staff_published_exam_schedule(
        &context.tenant.pool,
        context.user_id,
        query.academic_term_id,
    )
    .await?;

    Ok(Json(ApiResponse::ok(schedule)).into_response())
}
