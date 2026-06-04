use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::db::school_mapping::get_school_database_url;
use crate::error::AppError;
use crate::middleware::permission::check_permission;
use crate::modules::admission::models::applications::*;
use crate::modules::admission::services::score_service;
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

pub async fn get_all_scores(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ADMISSION_SCORES,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let scores = score_service::get_all_scores(&pool, round_id).await?;
    Ok(Json(json!({ "success": true, "data": scores })).into_response())
}

pub async fn get_application_scores(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ADMISSION_SCORES,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let scores = score_service::get_application_scores(&pool, id).await?;
    Ok(Json(json!({ "success": true, "data": scores })).into_response())
}

pub async fn update_scores(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateApplicationScoresRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = match check_permission(
        &headers,
        &pool,
        codes::ADMISSION_SCORES,
        &state.permission_cache,
    )
    .await
    {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    score_service::update_application_scores(&pool, id, user_id, &payload.scores).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "อัปเดตคะแนนแล้ว" })).into_response())
}

pub async fn bulk_update_scores(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(round_id): Path<Uuid>,
    Json(payload): Json<BulkUpdateScoresRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = match check_permission(
        &headers,
        &pool,
        codes::ADMISSION_SCORES,
        &state.permission_cache,
    )
    .await
    {
        Ok(u) => u,
        Err(r) => return Ok(r),
    };
    let updated =
        score_service::bulk_update_scores(&pool, round_id, user_id, &payload.entries).await?;
    Ok(Json(json!({ "success": true, "data": { "updated_count": updated }, "message": format!("อัปเดต {} รายการ", updated) })).into_response())
}
