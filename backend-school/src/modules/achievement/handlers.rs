use crate::api_response::{ApiErrorResponse, ApiResponse};
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
use uuid::Uuid;

#[utoipa::path(
    get,
    path = "/api/achievements",
    operation_id = "listAchievements",
    tag = "achievement",
    params(AchievementListFilter),
    responses(
        (status = 200, description = "Scoped achievement list", body = ApiResponse<Vec<Achievement>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Achievement read permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_achievements(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<AchievementListFilter>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let items =
        achievement_service::list_achievements(&context.tenant.pool, &context.actor, filter)
            .await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(items))).into_response())
}

#[utoipa::path(
    post,
    path = "/api/achievements",
    operation_id = "createAchievement",
    tag = "achievement",
    request_body = CreateAchievementRequest,
    responses(
        (status = 201, description = "Achievement created", body = ApiResponse<Achievement>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Achievement creation permission denied", body = ApiErrorResponse)
    )
)]
pub async fn create_achievement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAchievementRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let achievement =
        achievement_service::create_achievement(&context.tenant.pool, &context.actor, payload)
            .await?;

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(achievement))).into_response())
}

#[utoipa::path(
    put,
    path = "/api/achievements/{id}",
    operation_id = "updateAchievement",
    tag = "achievement",
    params(("id" = Uuid, Path, description = "Achievement ID")),
    request_body = UpdateAchievementRequest,
    responses(
        (status = 200, description = "Achievement updated", body = ApiResponse<Achievement>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Achievement update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Achievement not found", body = ApiErrorResponse)
    )
)]
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

    Ok((StatusCode::OK, Json(ApiResponse::ok(achievement))).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/achievements/{id}",
    operation_id = "deleteAchievement",
    tag = "achievement",
    params(("id" = Uuid, Path, description = "Achievement ID")),
    responses(
        (status = 200, description = "Achievement deleted", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Achievement deletion permission denied", body = ApiErrorResponse),
        (status = 404, description = "Achievement not found", body = ApiErrorResponse)
    )
)]
pub async fn delete_achievement(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    achievement_service::delete_achievement(&context.tenant.pool, &context.actor, id).await?;

    Ok((StatusCode::OK, Json(ApiResponse::empty())).into_response())
}
