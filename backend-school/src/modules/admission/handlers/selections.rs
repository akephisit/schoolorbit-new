use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::load_actor_context;
use crate::modules::admission::models::applications::{
    AssignRoomsGlobalRequest, AssignRoomsRequest,
};
use crate::modules::admission::models::rounds::UpdateSelectionSettingsRequest;
use crate::modules::admission::services::selection_service;
use crate::permissions::registry::codes;
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    resolve_tenant_pool(state, headers).await
}

#[derive(serde::Deserialize)]
pub struct RankingQuery {
    pub selection_subject_ids: Option<String>,
    pub room_assignment_method: Option<String>,
}

pub async fn get_ranking(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_SCORES) {
        return Ok(response);
    }
    let data = selection_service::get_round_ranking(&pool, round_id).await?;
    Ok(Json(json!({ "success": true, "data": data })).into_response())
}

pub async fn get_track_ranking(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(track_id): Path<Uuid>,
    Query(params): Query<RankingQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_SCORES) {
        return Ok(response);
    }
    let data = selection_service::get_track_ranking(
        &pool,
        track_id,
        params.selection_subject_ids,
        params.room_assignment_method,
    )
    .await?;
    Ok(Json(json!({ "success": true, "data": data })).into_response())
}

pub async fn assign_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<AssignRoomsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_SCORES) {
        return Ok(response);
    }
    let user_id = actor.user_id;
    let assigned_count = selection_service::assign_rooms(&pool, round_id, payload, user_id).await?;
    Ok(Json(json!({ "success": true, "data": { "assigned_count": assigned_count }, "message": format!("จัดห้องสำเร็จ {} คน", assigned_count) })).into_response())
}

pub async fn reset_all_room_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_SCORES) {
        return Ok(response);
    }
    let deleted = selection_service::reset_all_room_assignments(&pool, round_id).await?;
    Ok(Json(json!({ "success": true, "data": { "deleted": deleted } })).into_response())
}

pub async fn assign_rooms_global(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<AssignRoomsGlobalRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_SCORES) {
        return Ok(response);
    }
    let user_id = actor.user_id;
    let assigned_count =
        selection_service::assign_rooms_global(&pool, round_id, payload, user_id).await?;
    Ok(Json(json!({ "success": true, "data": { "assigned_count": assigned_count }, "message": format!("จัดห้องรวมสำเร็จ {} คน", assigned_count) })).into_response())
}

pub async fn get_global_ranking(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_SCORES) {
        return Ok(response);
    }
    let data = selection_service::get_global_ranking(&pool, round_id).await?;
    Ok(Json(json!({ "success": true, "data": data })).into_response())
}

pub async fn get_round_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_SCORES) {
        return Ok(response);
    }
    let rooms = selection_service::get_round_rooms(&pool, round_id).await?;
    Ok(Json(json!({ "success": true, "data": rooms })).into_response())
}

pub async fn update_selection_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<UpdateSelectionSettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ADMISSION_SCORES) {
        return Ok(response);
    }
    selection_service::update_selection_settings(&pool, round_id, payload).await?;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}
