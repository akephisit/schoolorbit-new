use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::db::school_mapping::get_school_database_url;
use crate::error::AppError;
use crate::middleware::permission::check_permission;
use crate::modules::admission::models::rounds::*;
use crate::modules::admission::services::round_service;
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

// ----- Public -----

pub async fn list_public_rounds(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let rounds = round_service::list_public_rounds(&pool).await?;
    Ok(Json(json!({ "success": true, "data": rounds })))
}

pub async fn get_public_round_info(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let (round, tracks) = round_service::get_public_round_info(&pool, id).await?;
    Ok(Json(json!({ "success": true, "data": { "round": round, "tracks": tracks } })))
}

// ----- Rounds CRUD -----

pub async fn list_rounds(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let rounds = round_service::list_rounds(&pool).await?;
    Ok(Json(json!({ "success": true, "data": rounds })).into_response())
}

pub async fn get_round(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let round = round_service::get_round(&pool, id).await?;
    Ok(Json(json!({ "success": true, "data": round })).into_response())
}

pub async fn create_round(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAdmissionRoundRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let round = round_service::create_round(&pool, payload).await?;
    Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": round }))).into_response())
}

pub async fn update_round(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAdmissionRoundRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let round = round_service::update_round(&pool, id, payload).await?;
    Ok(Json(json!({ "success": true, "data": round })).into_response())
}

pub async fn update_round_status(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateRoundStatusRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    round_service::update_round_status(&pool, id, &payload.status).await?;
    Ok(Json(json!({ "success": true, "message": format!("อัปเดตสถานะเป็น '{}'", payload.status) })).into_response())
}

pub async fn delete_round(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    round_service::delete_round(&pool, id).await?;
    Ok(Json(json!({ "success": true, "message": "ลบรอบรับสมัครและใบสมัครที่เกี่ยวข้องเรียบร้อยแล้ว" })).into_response())
}

pub async fn toggle_round_visibility(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateRoundVisibilityRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let updated = round_service::toggle_round_visibility(&pool, id, payload.is_visible).await?;
    Ok(Json(json!({ "success": true, "data": { "isVisible": updated } })).into_response())
}

// ----- Exam Subjects CRUD -----

pub async fn list_subjects(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let subjects = round_service::list_exam_subjects(&pool, round_id).await?;
    Ok(Json(json!({ "success": true, "data": subjects })).into_response())
}

pub async fn create_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<CreateExamSubjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let subject = round_service::create_exam_subject(&pool, round_id, payload).await?;
    Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": subject }))).into_response())
}

pub async fn update_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateExamSubjectRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let subject = round_service::update_exam_subject(&pool, id, payload).await?;
    Ok(Json(json!({ "success": true, "data": subject })).into_response())
}

pub async fn delete_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    round_service::delete_exam_subject(&pool, id).await?;
    Ok(Json(json!({ "success": true, "message": "ลบวิชาแล้ว" })).into_response())
}

// ----- Tracks CRUD -----

pub async fn list_tracks(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let tracks = round_service::list_tracks(&pool, round_id).await?;
    Ok(Json(json!({ "success": true, "data": tracks })).into_response())
}

pub async fn create_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<CreateAdmissionTrackRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let track = round_service::create_track(&pool, round_id, payload).await?;
    Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": track }))).into_response())
}

pub async fn update_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAdmissionTrackRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let track = round_service::update_track(&pool, id, payload).await?;
    Ok(Json(json!({ "success": true, "data": track })).into_response())
}

pub async fn delete_track(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_MANAGE_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    round_service::delete_track(&pool, id).await?;
    Ok(Json(json!({ "success": true, "message": "ลบสายการเรียนแล้ว" })).into_response())
}

pub async fn get_track_capacity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(&headers, &pool, codes::ADMISSION_READ_ALL, &state.permission_cache).await {
        return Ok(r);
    }
    let rooms = round_service::get_track_capacity(&pool, id).await?;
    let room_count = rooms.len();
    Ok(Json(json!({
        "success": true,
        "data": { "rooms": rooms, "room_count": room_count }
    })).into_response())
}
