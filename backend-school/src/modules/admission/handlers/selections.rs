use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::db::school_mapping::get_school_database_url;
use crate::error::AppError;
use crate::middleware::permission::check_permission;
use crate::modules::admission::models::applications::{AssignRoomsGlobalRequest, AssignRoomsRequest};
use crate::modules::admission::models::rounds::UpdateSelectionSettingsRequest;
use crate::modules::admission::services::selection_service;
use crate::permissions::registry::codes;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
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
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        return Ok(r);
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
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        return Ok(r);
    }
    let data = selection_service::get_track_ranking(
        &pool, track_id, params.selection_subject_ids, params.room_assignment_method,
    ).await?;
    Ok(Json(json!({ "success": true, "data": data })).into_response())
}

pub async fn assign_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<AssignRoomsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = match check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    let assigned_count = selection_service::assign_rooms(&pool, round_id, payload, user_id).await?;
    Ok(Json(json!({ "success": true, "data": { "assigned_count": assigned_count }, "message": format!("จัดห้องสำเร็จ {} คน", assigned_count) })).into_response())
}

pub async fn reset_all_room_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        return Ok(r);
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
    let user_id = match check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    let assigned_count = selection_service::assign_rooms_global(&pool, round_id, payload, user_id).await?;
    Ok(Json(json!({ "success": true, "data": { "assigned_count": assigned_count }, "message": format!("จัดห้องรวมสำเร็จ {} คน", assigned_count) })).into_response())
}

pub async fn get_global_ranking(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        return Ok(r);
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
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        return Ok(r);
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
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_SCORES, &state.permission_cache).await {
        return Ok(r);
    }
    selection_service::update_selection_settings(&pool, round_id, payload).await?;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}
