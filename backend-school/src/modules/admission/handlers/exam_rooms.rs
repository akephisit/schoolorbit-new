use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::admission::services::exam_room_service;
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ListExamRoomsData {
    rooms: Vec<exam_room_service::ExamRoomRow>,
    total_capacity: i64,
    total_assigned: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AssignExamSeatsData {
    assigned_count: usize,
    rooms: Vec<exam_room_service::AssignSeatsRoomSummary>,
}

pub async fn list_exam_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    let result = exam_room_service::list_exam_rooms(&pool, round_id).await?;
    Ok(Json(ApiResponse::ok(ListExamRoomsData {
        rooms: result.rooms,
        total_capacity: result.total_capacity,
        total_assigned: result.total_assigned,
    }))
    .into_response())
}

pub async fn add_exam_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<AddExamRoomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    exam_room_service::add_exam_room(
        &pool,
        round_id,
        payload.room_id,
        payload.custom_name,
        payload.capacity_override,
        payload.display_order,
    )
    .await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

pub async fn update_exam_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((round_id, room_id)): Path<(Uuid, Uuid)>,
    Json(payload): Json<UpdateExamRoomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    exam_room_service::update_exam_room(
        &pool,
        round_id,
        room_id,
        payload.capacity_override,
        payload.display_order,
        payload.custom_name,
    )
    .await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

pub async fn remove_exam_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((round_id, room_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    exam_room_service::remove_exam_room(&pool, round_id, room_id).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

pub async fn copy_exam_rooms_from_round(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((round_id, from_round_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    let n = exam_room_service::copy_exam_rooms_from_round(&pool, round_id, from_round_id).await?;
    Ok(Json(ApiResponse::empty_with_message(format!(
        "copy ห้องสอบ {} ห้องเรียบร้อย",
        n
    )))
    .into_response())
}

pub async fn update_exam_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<UpdateExamConfigRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    exam_room_service::update_exam_config(
        &pool,
        round_id,
        payload.exam_id_type,
        payload.exam_id_prefix,
        payload.sort_order,
    )
    .await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

pub async fn get_exam_config(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    let config = exam_room_service::get_exam_config(&pool, round_id).await?;
    Ok(Json(ApiResponse::ok(config)).into_response())
}

pub async fn assign_exam_seats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<AssignExamSeatsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    let user_id = actor.user_id;
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
    Ok(Json(ApiResponse::with_message(
        AssignExamSeatsData {
            assigned_count: result.assigned_count,
            rooms: result.rooms,
        },
        result.message,
    ))
    .into_response())
}

pub async fn get_exam_seats(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    let groups = exam_room_service::get_exam_seats(&pool, round_id).await?;
    Ok(Json(ApiResponse::ok(groups)).into_response())
}

pub async fn get_application_exam_seat(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(application_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_READ_ALL)?;
    let seat = exam_room_service::get_application_exam_seat(&pool, application_id).await?;
    Ok(Json(ApiResponse::ok(seat)).into_response())
}
