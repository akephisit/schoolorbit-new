use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::admission::models::rounds::*;
use crate::modules::admission::services::round_service;
use crate::permissions::registry::codes;
use crate::utils::request_context::{actor_tenant_context, tenant_pool};
use crate::AppState;

// ----- Public -----

pub async fn list_public_rounds(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let rounds = round_service::list_public_rounds(&pool).await?;
    Ok(Json(json!({ "success": true, "data": rounds })))
}

pub async fn get_public_round_info(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let (round, tracks) = round_service::get_public_round_info(&pool, id).await?;
    Ok(Json(
        json!({ "success": true, "data": { "round": round, "tracks": tracks } }),
    ))
}

// ----- Rounds CRUD -----

pub async fn list_rounds(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_READ_ALL)?;
    let rounds = round_service::list_rounds(&pool).await?;
    Ok(Json(json!({ "success": true, "data": rounds })).into_response())
}

pub async fn get_round(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_READ_ALL)?;
    let round = round_service::get_round(&pool, id).await?;
    Ok(Json(json!({ "success": true, "data": round })).into_response())
}

pub async fn create_round(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAdmissionRoundRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    let round = round_service::create_round(&pool, payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": round })),
    )
        .into_response())
}

pub async fn update_round(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAdmissionRoundRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    let round = round_service::update_round(&pool, id, payload).await?;
    Ok(Json(json!({ "success": true, "data": round })).into_response())
}

pub async fn update_round_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateRoundStatusRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    round_service::update_round_status(&pool, id, &payload.status).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": format!("อัปเดตสถานะเป็น '{}'", payload.status) })).into_response())
}

pub async fn delete_round(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    round_service::delete_round(&pool, id).await?;
    Ok(Json(
        json!({ "success": true, "data": {}, "message": "ลบรอบรับสมัครและใบสมัครที่เกี่ยวข้องเรียบร้อยแล้ว" }),
    )
    .into_response())
}

pub async fn toggle_round_visibility(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateRoundVisibilityRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    let updated = round_service::toggle_round_visibility(&pool, id, payload.is_visible).await?;
    Ok(Json(json!({ "success": true, "data": { "isVisible": updated } })).into_response())
}

// ----- Exam Subjects CRUD -----

pub async fn list_subjects(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_READ_ALL)?;
    let subjects = round_service::list_exam_subjects(&pool, round_id).await?;
    Ok(Json(json!({ "success": true, "data": subjects })).into_response())
}

pub async fn create_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<CreateExamSubjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    let subject = round_service::create_exam_subject(&pool, round_id, payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": subject })),
    )
        .into_response())
}

pub async fn update_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateExamSubjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    let subject = round_service::update_exam_subject(&pool, id, payload).await?;
    Ok(Json(json!({ "success": true, "data": subject })).into_response())
}

pub async fn delete_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    round_service::delete_exam_subject(&pool, id).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "ลบวิชาแล้ว" })).into_response())
}

// ----- Tracks CRUD -----

pub async fn list_tracks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_READ_ALL)?;
    let tracks = round_service::list_tracks(&pool, round_id).await?;
    Ok(Json(json!({ "success": true, "data": tracks })).into_response())
}

pub async fn create_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<CreateAdmissionTrackRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    let track = round_service::create_track(&pool, round_id, payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": track })),
    )
        .into_response())
}

pub async fn update_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAdmissionTrackRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    let track = round_service::update_track(&pool, id, payload).await?;
    Ok(Json(json!({ "success": true, "data": track })).into_response())
}

pub async fn delete_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_MANAGE_ALL)?;
    round_service::delete_track(&pool, id).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "ลบสายการเรียนแล้ว" })).into_response())
}

pub async fn get_track_capacity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ADMISSION_READ_ALL)?;
    let rooms = round_service::get_track_capacity(&pool, id).await?;
    let room_count = rooms.len();
    Ok(
        Json(json!({ "success": true, "data": { "rooms": rooms, "room_count": room_count } }))
            .into_response(),
    )
}
