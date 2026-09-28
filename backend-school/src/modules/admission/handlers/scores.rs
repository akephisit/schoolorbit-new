use axum::{
    extract::{Extension, Path, State},
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::utils::request_context::actor_tenant_context_from_session;
use crate::AppState;
use school_admission::applications::*;
use school_admission::scores as score_service;
use school_auth::session_service::AuthenticatedSession;
use school_http::HttpError as AppError;
use school_http::{ApiErrorResponse, ApiResponse};
use school_permissions::registry::codes;

#[derive(Debug, Serialize)]
struct UpdatedCountData<T> {
    updated_count: T,
}

pub async fn get_all_scores(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    let scores = score_service::get_all_scores(&pool, round_id).await?;
    Ok(Json(ApiResponse::ok(scores)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/admission/rounds/{round_id}/score-room-roster",
    operation_id = "getAdmissionScoreRoomRoster",
    tag = "admission",
    params(("round_id" = Uuid, Path, description = "Admission round ID")),
    responses(
        (status = 200, description = "Score-entry room roster without national IDs", body = ApiResponse<Vec<score_service::ScoreRoomGroup>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Admission score permission required", body = ApiErrorResponse)
    )
)]
pub async fn get_score_room_roster(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    let groups = score_service::get_score_room_roster(&context.tenant.pool, round_id).await?;
    Ok(Json(ApiResponse::ok(groups)).into_response())
}

pub async fn get_application_scores(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    let scores = score_service::get_application_scores(&pool, id).await?;
    Ok(Json(ApiResponse::ok(scores)).into_response())
}

pub async fn update_scores(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateApplicationScoresRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    let user_id = actor.user_id;
    score_service::update_application_scores(&pool, id, user_id, &payload.scores).await?;
    Ok(Json(ApiResponse::empty_with_message("อัปเดตคะแนนแล้ว")).into_response())
}

pub async fn bulk_update_scores(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<BulkUpdateScoresRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    let user_id = actor.user_id;
    let updated =
        score_service::bulk_update_scores(&pool, round_id, user_id, &payload.entries).await?;
    Ok(Json(ApiResponse::with_message(
        UpdatedCountData {
            updated_count: updated,
        },
        format!("อัปเดต {} รายการ", updated),
    ))
    .into_response())
}
