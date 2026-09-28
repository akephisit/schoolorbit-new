use axum::{
    extract::{Extension, Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::utils::request_context::actor_tenant_context_from_session;
use crate::AppState;
use school_admission::applications::{AssignRoomsGlobalRequest, AssignRoomsRequest};
use school_admission::rounds::UpdateSelectionSettingsRequest;
use school_admission::selections as selection_service;
use school_auth::session_service::AuthenticatedSession;
use school_http::HttpError as AppError;
use school_http::{ApiErrorResponse, ApiResponse};
use school_permissions::registry::codes;

#[derive(serde::Deserialize)]
pub struct RankingQuery {
    pub selection_subject_ids: Option<String>,
    pub room_assignment_method: Option<String>,
}

#[derive(Debug, Serialize)]
struct AssignedCountData<T> {
    assigned_count: T,
}

#[derive(Debug, Serialize)]
struct DeletedData<T> {
    deleted: T,
}

#[utoipa::path(
    get,
    path = "/api/admission/rounds/{id}/ranking",
    operation_id = "getAdmissionRoundRanking",
    tag = "admission",
    params(("id" = Uuid, Path, description = "Admission round ID")),
    responses(
        (status = 200, description = "Round ranking without national IDs", body = ApiResponse<Vec<selection_service::RoundRankingResult>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Admission score permission required", body = ApiErrorResponse)
    )
)]
pub async fn get_ranking(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    let data = selection_service::get_round_ranking(&pool, round_id).await?;
    Ok(Json(ApiResponse::ok(data)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/admission/tracks/{id}/ranking",
    operation_id = "getAdmissionTrackRanking",
    tag = "admission",
    params(
        ("id" = Uuid, Path, description = "Admission track ID"),
        ("selection_subject_ids" = Option<String>, Query, description = "Comma-separated subject IDs"),
        ("room_assignment_method" = Option<String>, Query, description = "Room assignment method")
    ),
    responses(
        (status = 200, description = "Track ranking without national IDs", body = ApiResponse<selection_service::TrackRankingResult>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Admission score permission required", body = ApiErrorResponse)
    )
)]
pub async fn get_track_ranking(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(track_id): Path<Uuid>,
    Query(params): Query<RankingQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    let data = selection_service::get_track_ranking(
        &pool,
        track_id,
        params.selection_subject_ids,
        params.room_assignment_method,
    )
    .await?;
    Ok(Json(ApiResponse::ok(data)).into_response())
}

pub async fn assign_rooms(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<AssignRoomsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    let user_id = actor.user_id;
    let assigned_count = selection_service::assign_rooms(&pool, round_id, payload, user_id).await?;
    Ok(Json(ApiResponse::with_message(
        AssignedCountData { assigned_count },
        format!("จัดห้องสำเร็จ {} คน", assigned_count),
    ))
    .into_response())
}

pub async fn reset_all_room_assignments(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    let deleted = selection_service::reset_all_room_assignments(&pool, round_id).await?;
    Ok(Json(ApiResponse::ok(DeletedData { deleted })).into_response())
}

pub async fn assign_rooms_global(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<AssignRoomsGlobalRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    let user_id = actor.user_id;
    let assigned_count =
        selection_service::assign_rooms_global(&pool, round_id, payload, user_id).await?;
    Ok(Json(ApiResponse::with_message(
        AssignedCountData { assigned_count },
        format!("จัดห้องรวมสำเร็จ {} คน", assigned_count),
    ))
    .into_response())
}

#[utoipa::path(
    get,
    path = "/api/admission/rounds/{id}/global-ranking",
    operation_id = "getAdmissionGlobalRanking",
    tag = "admission",
    params(("id" = Uuid, Path, description = "Admission round ID")),
    responses(
        (status = 200, description = "Global ranking without national IDs", body = ApiResponse<selection_service::GlobalRankingResult>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Admission score permission required", body = ApiErrorResponse)
    )
)]
pub async fn get_global_ranking(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    let data = selection_service::get_global_ranking(&pool, round_id).await?;
    Ok(Json(ApiResponse::ok(data)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/admission/rounds/{id}/rooms",
    operation_id = "getAdmissionRoundRooms",
    tag = "admission",
    params(("id" = Uuid, Path, description = "Admission round ID")),
    responses(
        (status = 200, description = "Round rooms for assignment", body = ApiResponse<Vec<selection_service::RoomBasic>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Admission score permission required", body = ApiErrorResponse)
    )
)]
pub async fn get_round_rooms(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    let rooms = selection_service::get_round_rooms(&pool, round_id).await?;
    Ok(Json(ApiResponse::ok(rooms)).into_response())
}

pub async fn update_selection_settings(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<UpdateSelectionSettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    selection_service::update_selection_settings(&pool, round_id, payload).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}
