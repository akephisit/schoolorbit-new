use crate::error::AppError;
use crate::modules::achievement::models::*;
use crate::modules::achievement::services as achievement_service;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

pub async fn list_achievements(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<AchievementListFilter>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let items =
        achievement_service::list_achievements(&context.tenant.pool, &context.actor, filter)
            .await?;

    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": items })),
    )
        .into_response())
}

pub async fn create_achievement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAchievementRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let achievement =
        achievement_service::create_achievement(&context.tenant.pool, &context.actor, payload)
            .await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": achievement })),
    )
        .into_response())
}

pub async fn update_achievement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAchievementRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let achievement =
        achievement_service::update_achievement(&context.tenant.pool, &context.actor, id, payload)
            .await?;

    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": achievement })),
    )
        .into_response())
}

pub async fn delete_achievement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    achievement_service::delete_achievement(&context.tenant.pool, &context.actor, id).await?;

    Ok((StatusCode::OK, Json(json!({ "success": true, "data": {} }))).into_response())
}
