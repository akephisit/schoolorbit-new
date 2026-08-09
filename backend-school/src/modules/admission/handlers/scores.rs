use axum::{
    extract::{Extension, Path, State},
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use uuid::Uuid;

use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::admission::models::applications::*;
use crate::modules::admission::services::score_service;
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::AppState;

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
