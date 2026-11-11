use axum::{
    extract::{Extension, Path, Query, State},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
#[cfg(feature = "api-docs")]
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::applications::*;
use crate::http::state::AdmissionHttpState;
use crate::scores as score_service;
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
#[cfg(feature = "api-docs")]
use school_http::ApiErrorResponse;
use school_http::ApiResponse;
use school_http::HttpError as AppError;
use school_permissions::registry::codes;

#[derive(Debug, Serialize)]
struct UpdatedCountData<T> {
    updated_count: T,
}

pub async fn get_all_scores(
    State(state): State<AdmissionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    let scores = score_service::get_all_scores(&pool, round_id).await?;
    Ok(Json(ApiResponse::ok(scores)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
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
))]
pub async fn get_score_room_roster(
    State(state): State<AdmissionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    let groups = score_service::get_score_room_roster(&context.tenant.pool, round_id).await?;
    Ok(Json(ApiResponse::ok(groups)).into_response())
}

#[derive(Deserialize)]
#[cfg_attr(feature = "api-docs", derive(IntoParams, ToSchema))]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Query))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScoreTrackRosterQuery {
    pub track_id: Uuid,
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    get, path="/api/admission/rounds/{round_id}/score-track-roster", operation_id="getAdmissionScoreTrackRoster", tag="admission",
    params(("round_id"=Uuid,Path,description="Admission round ID"), ScoreTrackRosterQuery),
    responses(
        (status=200,description="Eligible score-entry applicants in the exact round and track, without private application fields",body=ApiResponse<Vec<score_service::ScoreTrackApplicant>>),
        (status=401,description="Authentication required",body=ApiErrorResponse),
        (status=403,description="Admission score permission required",body=ApiErrorResponse),
        (status=404,description="Track does not belong to the admission round",body=ApiErrorResponse),
        (status=422,description="Score track roster exceeds the complete supported size",body=ApiErrorResponse)
    )
))]
pub async fn get_score_track_roster(
    State(state): State<AdmissionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
    Query(query): Query<ScoreTrackRosterQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    context
        .actor
        .require_permission(codes::ADMISSION_SCORES_ALL)?;
    let applicants =
        score_service::get_score_track_roster(&context.tenant.pool, round_id, query.track_id)
            .await?;
    Ok(Json(ApiResponse::ok(applicants)).into_response())
}

pub async fn get_application_scores(
    State(state): State<AdmissionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    let scores = score_service::get_application_scores(&pool, id).await?;
    Ok(Json(ApiResponse::ok(scores)).into_response())
}

pub async fn update_scores(
    State(state): State<AdmissionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateApplicationScoresRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_SCORES_ALL)?;
    let user_id = actor.user_id;
    score_service::update_application_scores(&pool, id, user_id, &payload.scores).await?;
    Ok(Json(ApiResponse::empty_with_message("อัปเดตคะแนนแล้ว")).into_response())
}

pub async fn bulk_update_scores(
    State(state): State<AdmissionHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<BulkUpdateScoresRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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
