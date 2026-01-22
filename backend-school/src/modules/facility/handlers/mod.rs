use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
    response::IntoResponse,
    http::HeaderMap,
    routing::{get, post, put, delete},
    Router,
};
use serde_json::json;
use crate::middleware::permission::check_permission;
use crate::modules::facility::models::{
    Building, CreateBuildingRequest, UpdateBuildingRequest,
    Room, CreateRoomRequest, UpdateRoomRequest, RoomFilter
};
use uuid::Uuid;
use crate::permissions::registry::codes;
use crate::AppState;
use crate::error::AppError;
use crate::db::school_mapping::get_school_database_url;
use crate::utils::subdomain::extract_subdomain_from_request;

/// Helper
async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    
    let db_url = get_school_database_url(&state.admin_pool, &subdomain).await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
        
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

// ----------------------
// Buildings
// ----------------------

pub async fn list_buildings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    // Permission: READ
    // Using a general FACILITY_READ code or similar. Let's use ACADEMIC_READ for now or create new codes.
    // For now, let's assume if they can access academic, they can see rooms.
    // Or better, create codes::FACILITY_READ
    
    // For now using ACADEMIC_READ as fallback if FACILITY not defined, but user requested new codes.
    // I will use placeholders and define codes later.
    if let Err(response) = check_permission(&headers, &pool, codes::FACILITY_READ_ALL).await {
        return Ok(response);
    }

    let buildings = sqlx::query_as::<_, Building>(
        "SELECT * FROM buildings ORDER BY name_th ASC"
    )
    .fetch_all(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to fetch buildings".to_string()))?;

    Ok(Json(json!({ "success": true, "data": buildings })).into_response())
}

pub async fn create_building(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateBuildingRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(response) = check_permission(&headers, &pool, codes::FACILITY_CREATE_ALL).await {
        return Ok(response);
    }

    let building = sqlx::query_as::<_, Building>(
        r#"
        INSERT INTO buildings (name_th, name_en, code, description)
        VALUES ($1, $2, $3, $4)
        RETURNING *
        "#
    )
    .bind(payload.name_th)
    .bind(payload.name_en)
    .bind(payload.code)
    .bind(payload.description)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("Create Building Error: {}", e);
        AppError::InternalServerError("Failed to create building".to_string())
    })?;

    Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": building }))).into_response())
}

pub async fn update_building(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateBuildingRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    match check_permission(&headers, &pool, codes::FACILITY_UPDATE_ALL).await {
       Ok(_) => {},
       Err(r) => return Ok(r)
    }

    let building = sqlx::query_as::<_, Building>(
        r#"
        UPDATE buildings SET
            name_th = COALESCE($1, name_th),
            name_en = COALESCE($2, name_en),
            code = COALESCE($3, code),
            description = COALESCE($4, description),
            updated_at = NOW()
        WHERE id = $5
        RETURNING *
        "#
    )
    .bind(payload.name_th)
    .bind(payload.name_en)
    .bind(payload.code)
    .bind(payload.description)
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to update building".to_string()))?;

    Ok(Json(json!({ "success": true, "data": building })).into_response())
}

pub async fn delete_building(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    match check_permission(&headers, &pool, codes::FACILITY_DELETE_ALL).await {
       Ok(_) => {},
       Err(r) => return Ok(r)
    }

    sqlx::query("DELETE FROM buildings WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|_| AppError::InternalServerError("Failed to delete building".to_string()))?;

    Ok(Json(json!({ "success": true })).into_response())
}


// ----------------------
// Rooms
// ----------------------

pub async fn list_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<RoomFilter>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(response) = check_permission(&headers, &pool, codes::FACILITY_READ_ALL).await {
        return Ok(response);
    }

    let mut sql = String::from(
        r#"
        SELECT r.*, b.name_th as building_name
        FROM rooms r
        LEFT JOIN buildings b ON r.building_id = b.id
        WHERE 1=1
        "#
    );

    if let Some(bid) = filter.building_id {
        sql.push_str(&format!(" AND r.building_id = '{}'", bid));
    }
    if let Some(rtype) = &filter.room_type {
        sql.push_str(&format!(" AND r.room_type = '{}'", rtype));
    }
    if let Some(search) = &filter.search {
        let s = format!("%{}%", search);
        sql.push_str(&format!(" AND (r.name_th ILIKE '{}' OR r.code ILIKE '{}')", s, s));
    }

    sql.push_str(" ORDER BY b.code NULLS LAST, r.floor NULLS FIRST, r.code ASC");

    let rooms = sqlx::query_as::<_, Room>(&sql)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            eprintln!("List Rooms Error: {}", e);
            AppError::InternalServerError("Failed to fetch rooms".to_string())
        })?;

    Ok(Json(json!({ "success": true, "data": rooms })).into_response())
}

pub async fn create_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateRoomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(response) = check_permission(&headers, &pool, codes::FACILITY_CREATE_ALL).await {
        return Ok(response);
    }

    let room = sqlx::query_as::<_, Room>(
        r#"
        INSERT INTO rooms (
            building_id, name_th, name_en, code, room_type, 
            capacity, floor, status, description
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
        RETURNING *, (SELECT name_th FROM buildings WHERE id = $1) as building_name
        "#
    )
    .bind(payload.building_id)
    .bind(payload.name_th)
    .bind(payload.name_en)
    .bind(payload.code)
    .bind(payload.room_type)
    .bind(payload.capacity.unwrap_or(40))
    .bind(payload.floor)
    .bind(payload.status.unwrap_or("ACTIVE".to_string()))
    .bind(payload.description)
    .fetch_one(&pool)
    .await
    .map_err(|e| {
        eprintln!("Create Room Error: {}", e);
        AppError::InternalServerError("Failed to create room".to_string())
    })?;

    Ok((StatusCode::CREATED, Json(json!({ "success": true, "data": room }))).into_response())
}

pub async fn update_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateRoomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(response) = check_permission(&headers, &pool, codes::FACILITY_UPDATE_ALL).await {
        return Ok(response);
    }

    let room = sqlx::query_as::<_, Room>(
        r#"
        UPDATE rooms SET
            building_id = COALESCE($1, building_id),
            name_th = COALESCE($2, name_th),
            name_en = COALESCE($3, name_en),
            code = COALESCE($4, code),
            room_type = COALESCE($5, room_type),
            capacity = COALESCE($6, capacity),
            floor = COALESCE($7, floor),
            status = COALESCE($8, status),
            description = COALESCE($9, description),
            updated_at = NOW()
        WHERE id = $10
        RETURNING *, (SELECT name_th FROM buildings WHERE id = rooms.building_id) as building_name
        "#
    )
    .bind(payload.building_id)
    .bind(payload.name_th)
    .bind(payload.name_en)
    .bind(payload.code)
    .bind(payload.room_type)
    .bind(payload.capacity)
    .bind(payload.floor)
    .bind(payload.status)
    .bind(payload.description)
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("Failed to update room".to_string()))?;

    Ok(Json(json!({ "success": true, "data": room })).into_response())
}

pub async fn delete_room(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(response) = check_permission(&headers, &pool, codes::FACILITY_DELETE_ALL).await {
        return Ok(response);
    }

    sqlx::query("DELETE FROM rooms WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|_| AppError::InternalServerError("Failed to delete room".to_string()))?;

    Ok(Json(json!({ "success": true })).into_response())
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/buildings", get(list_buildings).post(create_building))
        .route("/buildings/:id", put(update_building).delete(delete_building))
        .route("/rooms", get(list_rooms).post(create_room))
        .route("/rooms/:id", put(update_room).delete(delete_room))
}
