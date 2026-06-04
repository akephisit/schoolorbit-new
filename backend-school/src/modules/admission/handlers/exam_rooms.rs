use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::db::school_mapping::get_school_database_url;
use crate::error::AppError;
use crate::middleware::permission::check_permission;
use crate::modules::admission::services::exam_room_service;
use crate::permissions::registry::codes;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain)
        .await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    state
        .pool_manager
        .get_pool(&db_url, &subdomain)
        .await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AddExamRoomRequest {
    room_id: Option<Uuid>,
    custom_name: Option<String>,
    capacity_override: Option<i32>,
    display_order: Option<i32>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExamConfigRequest {
    exam_id_type: Option<String>,
    exam_id_prefix: Option<String>,
    sort_order: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssignExamSeatsRequest {
    exam_id_type: Option<String>,
    exam_id_prefix: Option<String>,
    sort_order: Option<String>,
    mode: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExamRoomRequest {
    capacity_override: Option<i32>,
    display_order: Option<i32>,
    custom_name: Option<String>,
}

pub async fn list_exam_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ADMISSION_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let result = exam_room_service::list_exam_rooms(&pool, round_id).await?;
    Ok(Json(json!({ "success": true, "data": {
            "rooms": result.rooms,
            "totalCapacity": result.total_capacity,
            "totalAssigned": result.total_assigned,
        } }))
    .into_response())
}

pub async fn add_exam_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<AddExamRoomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ADMISSION_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    exam_room_service::add_exam_room(
        &pool,
        round_id,
        payload.room_id,
        payload.custom_name,
        payload.capacity_override,
        payload.display_order,
    )
    .await?;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

pub async fn update_exam_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((round_id, room_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateExamRoomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ADMISSION_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    exam_room_service::update_exam_room(
        &pool,
        round_id,
        room_id,
        payload.capacity_override,
        payload.display_order,
        payload.custom_name,
    )
    .await?;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

pub async fn remove_exam_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((round_id, room_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ADMISSION_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    exam_room_service::remove_exam_room(&pool, round_id, room_id).await?;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

pub async fn copy_exam_rooms_from_round(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((round_id, from_round_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ADMISSION_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let n = exam_room_service::copy_exam_rooms_from_round(&pool, round_id, from_round_id).await?;
    Ok(Json(
        json!({ "success": true, "data": {}, "message": format!("copy ห้องสอบ {} ห้องเรียบร้อย", n) }),
    )
    .into_response())
}

pub async fn update_exam_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<UpdateExamConfigRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ADMISSION_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    exam_room_service::update_exam_config(
        &pool,
        round_id,
        payload.exam_id_type,
        payload.exam_id_prefix,
        payload.sort_order,
    )
    .await?;
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

pub async fn get_exam_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ADMISSION_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let config = exam_room_service::get_exam_config(&pool, round_id).await?;
    Ok(Json(json!({ "success": true, "data": config })).into_response())
}

pub async fn assign_exam_seats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<AssignExamSeatsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = match check_permission(
        &headers,
        &pool,
        codes::ADMISSION_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    let result = exam_room_service::assign_exam_seats(
        &pool,
        round_id,
        user_id,
        payload.exam_id_type,
        payload.exam_id_prefix,
        payload.sort_order,
        payload.mode,
    )
    .await?;
    Ok(Json(json!({ "success": true, "data": { "assignedCount": result.assigned_count, "rooms": result.rooms }, "message": result.message })).into_response())
}

pub async fn get_exam_seats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ADMISSION_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let groups = exam_room_service::get_exam_seats(&pool, round_id).await?;
    Ok(Json(json!({ "success": true, "data": groups })).into_response())
}

pub async fn get_application_exam_seat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ADMISSION_READ_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let seat = exam_room_service::get_application_exam_seat(&pool, application_id).await?;
    Ok(Json(json!({ "success": true, "data": seat })).into_response())
}
