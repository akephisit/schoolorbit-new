use crate::error::AppError;
use crate::modules::facility::models::{
    CreateBuildingRequest, CreateRoomRequest, RoomFilter, UpdateBuildingRequest, UpdateRoomRequest,
};
use crate::modules::facility::services;
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, put},
    Json, Router,
};
use serde_json::json;
use uuid::Uuid;

// ----------------------
// Buildings
// ----------------------

pub async fn list_buildings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context.actor.require_permission(codes::FACILITY_READ_ALL)?;
    let buildings = services::list_buildings(&context.tenant.pool).await?;

    Ok(Json(json!({ "success": true, "data": buildings })).into_response())
}

pub async fn create_building(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateBuildingRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::FACILITY_CREATE_ALL)?;
    let building = services::create_building(&context.tenant.pool, payload).await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": building })),
    )
        .into_response())
}

pub async fn update_building(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateBuildingRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::FACILITY_UPDATE_ALL)?;
    let building = services::update_building(&context.tenant.pool, id, payload).await?;

    Ok(Json(json!({ "success": true, "data": building })).into_response())
}

pub async fn delete_building(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::FACILITY_DELETE_ALL)?;
    services::delete_building(&context.tenant.pool, id).await?;

    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

// ----------------------
// Rooms
// ----------------------

pub async fn list_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<RoomFilter>,
) -> Result<impl IntoResponse, AppError> {
    // Any authenticated staff can list rooms (used for timetable, exam rooms, etc.)
    let context = actor_tenant_context(&state, &headers).await?;
    let rooms = services::list_rooms(&context.tenant.pool, filter).await?;

    Ok(Json(json!({ "success": true, "data": rooms })).into_response())
}

pub async fn create_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateRoomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::FACILITY_CREATE_ALL)?;
    let room = services::create_room(&context.tenant.pool, payload).await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": room })),
    )
        .into_response())
}

pub async fn update_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateRoomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::FACILITY_UPDATE_ALL)?;
    let room = services::update_room(&context.tenant.pool, id, payload).await?;

    Ok(Json(json!({ "success": true, "data": room })).into_response())
}

pub async fn delete_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::FACILITY_DELETE_ALL)?;
    services::delete_room(&context.tenant.pool, id).await?;

    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/buildings", get(list_buildings).post(create_building))
        .route(
            "/buildings/{id}",
            put(update_building).delete(delete_building),
        )
        .route("/rooms", get(list_rooms).post(create_room))
        .route("/rooms/{id}", put(update_room).delete(delete_room))
}
