use crate::error::AppError;
use crate::middleware::permission::load_actor_context;
use crate::modules::achievement::models::*;
use crate::modules::achievement::services as achievement_service;
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

async fn get_db_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    resolve_tenant_pool(state, headers).await
}

pub async fn list_achievements(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<AchievementListFilter>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_db_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let items = achievement_service::list_achievements(&pool, &actor, filter).await?;

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
    let pool = get_db_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let achievement = achievement_service::create_achievement(&pool, &actor, payload).await?;

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
    let pool = get_db_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    let achievement = achievement_service::update_achievement(&pool, &actor, id, payload).await?;

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
    let pool = get_db_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    achievement_service::delete_achievement(&pool, &actor, id).await?;

    Ok((StatusCode::OK, Json(json!({ "success": true, "data": {} }))).into_response())
}
