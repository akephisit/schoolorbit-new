use crate::modules::facility::models::{
    Building, CreateBuildingRequest, CreateRoomRequest, Room, RoomFilter, UpdateBuildingRequest,
    UpdateRoomRequest,
};
use crate::modules::facility::services;
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::AppState;
use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, put},
    Json, Router,
};
use school_auth::session_service::AuthenticatedSession;
use school_http::HttpError as AppError;
use school_http::{ApiErrorResponse, ApiResponse};
use school_permissions::registry::codes;
use uuid::Uuid;

// ----------------------
// Buildings
// ----------------------

#[utoipa::path(
    get, path = "/api/facilities/buildings",
    operation_id = "listFacilityBuildings", tag = "facility",
    responses(
        (status = 200, description = "List building result", body = ApiResponse<Vec<Building>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Facility action permission required", body = ApiErrorResponse)
    )
)]
pub async fn list_buildings(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context.actor.require_permission(codes::FACILITY_READ_ALL)?;
    let buildings = services::list_buildings(&context.tenant.pool).await?;

    Ok(Json(ApiResponse::ok(buildings)).into_response())
}

#[utoipa::path(
    post, path = "/api/facilities/buildings",
    operation_id = "createFacilityBuilding", tag = "facility",
    request_body = CreateBuildingRequest,
    responses(
        (status = 201, description = "Create building result", body = ApiResponse<Building>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Facility action permission required", body = ApiErrorResponse)
    )
)]
pub async fn create_building(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<CreateBuildingRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::FACILITY_CREATE_ALL)?;
    let building = services::create_building(&context.tenant.pool, payload).await?;

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(building))).into_response())
}

#[utoipa::path(
    put, path = "/api/facilities/buildings/{id}",
    operation_id = "updateFacilityBuilding", tag = "facility",
    request_body = UpdateBuildingRequest,
    params(("id" = Uuid, Path, description = "Resource ID")),
    responses(
        (status = 200, description = "Update building result", body = ApiResponse<Building>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Facility action permission required", body = ApiErrorResponse)
    )
)]
pub async fn update_building(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateBuildingRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::FACILITY_UPDATE_ALL)?;
    let building = services::update_building(&context.tenant.pool, id, payload).await?;

    Ok(Json(ApiResponse::ok(building)).into_response())
}

#[utoipa::path(
    delete, path = "/api/facilities/buildings/{id}",
    operation_id = "deleteFacilityBuilding", tag = "facility",
    params(("id" = Uuid, Path, description = "Resource ID")),
    responses(
        (status = 200, description = "Delete building result", body = ApiResponse<school_http::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Facility action permission required", body = ApiErrorResponse)
    )
)]
pub async fn delete_building(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::FACILITY_DELETE_ALL)?;
    services::delete_building(&context.tenant.pool, id).await?;

    Ok(Json(ApiResponse::empty()).into_response())
}

// ----------------------
// Rooms
// ----------------------

#[utoipa::path(
    get, path = "/api/facilities/rooms",
    operation_id = "listFacilityRooms", tag = "facility",
    params(("building_id" = Option<Uuid>, Query), ("room_type" = Option<String>, Query), ("search" = Option<String>, Query)),
    responses(
        (status = 200, description = "List room result", body = ApiResponse<Vec<Room>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse)
    )
)]
pub async fn list_rooms(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(filter): Query<RoomFilter>,
) -> Result<impl IntoResponse, AppError> {
    // Any authenticated staff can list rooms (used for timetable, exam rooms, etc.)
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let rooms = services::list_rooms(&context.tenant.pool, filter).await?;

    Ok(Json(ApiResponse::ok(rooms)).into_response())
}

#[utoipa::path(
    post, path = "/api/facilities/rooms",
    operation_id = "createFacilityRoom", tag = "facility",
    request_body = CreateRoomRequest,
    responses(
        (status = 201, description = "Create room result", body = ApiResponse<Room>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Facility action permission required", body = ApiErrorResponse)
    )
)]
pub async fn create_room(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<CreateRoomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::FACILITY_CREATE_ALL)?;
    let room = services::create_room(&context.tenant.pool, payload).await?;

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(room))).into_response())
}

#[utoipa::path(
    put, path = "/api/facilities/rooms/{id}",
    operation_id = "updateFacilityRoom", tag = "facility",
    request_body = UpdateRoomRequest,
    params(("id" = Uuid, Path, description = "Resource ID")),
    responses(
        (status = 200, description = "Update room result", body = ApiResponse<Room>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Facility action permission required", body = ApiErrorResponse)
    )
)]
pub async fn update_room(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateRoomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::FACILITY_UPDATE_ALL)?;
    let room = services::update_room(&context.tenant.pool, id, payload).await?;

    Ok(Json(ApiResponse::ok(room)).into_response())
}

#[utoipa::path(
    delete, path = "/api/facilities/rooms/{id}",
    operation_id = "deleteFacilityRoom", tag = "facility",
    params(("id" = Uuid, Path, description = "Resource ID")),
    responses(
        (status = 200, description = "Delete room result", body = ApiResponse<school_http::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Facility action permission required", body = ApiErrorResponse)
    )
)]
pub async fn delete_room(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::FACILITY_DELETE_ALL)?;
    services::delete_room(&context.tenant.pool, id).await?;

    Ok(Json(ApiResponse::empty()).into_response())
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
