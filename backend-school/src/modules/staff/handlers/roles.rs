use crate::error::AppError;
use crate::middleware::permission::load_actor_context;
use crate::modules::staff::models::*;
use crate::modules::staff::services::{department_service, role_service};
use crate::permissions::registry::codes;
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    resolve_tenant_pool(state, headers).await
}

// ============================================
// Roles
// ============================================

pub async fn list_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ROLES_READ_ALL) {
        return Ok(response);
    }

    let roles = role_service::list_roles(&pool).await?;
    Ok(Json(json!({ "success": true, "data": roles })).into_response())
}

pub async fn get_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(role_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ROLES_READ_ALL) {
        return Ok(response);
    }

    let role = role_service::get_role(&pool, role_id).await?;
    Ok(Json(json!({ "success": true, "data": role })).into_response())
}

pub async fn create_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ROLES_CREATE_ALL) {
        return Ok(response);
    }

    let role_id = role_service::create_role(&pool, payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": { "id": role_id }, "message": "สร้างบทบาทสำเร็จ" })),
    )
        .into_response())
}

pub async fn update_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(role_id): Path<Uuid>,
    Json(payload): Json<UpdateRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ROLES_UPDATE_ALL) {
        return Ok(response);
    }

    role_service::update_role(&pool, role_id, payload).await?;

    // Role permissions changed — every user with this role has stale cache
    state.permission_cache.clear_all();
    state.notify_all_permissions_changed();

    Ok(Json(json!({ "success": true, "data": {}, "message": "อัปเดตบทบาทสำเร็จ" })).into_response())
}

// ============================================
// Departments
// ============================================

pub async fn list_departments(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ROLES_READ_ALL) {
        return Ok(response);
    }

    let departments = department_service::list_departments(&pool).await?;
    Ok(Json(json!({ "success": true, "data": departments })).into_response())
}

pub async fn get_department(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(dept_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ROLES_READ_ALL) {
        return Ok(response);
    }

    let department = department_service::get_department(&pool, dept_id).await?;
    Ok(Json(json!({ "success": true, "data": department })).into_response())
}

pub async fn create_department(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateDepartmentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ROLES_CREATE_ALL) {
        return Ok(response);
    }

    let dept_id = department_service::create_department(&pool, payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": { "id": dept_id }, "message": "สร้างฝ่ายสำเร็จ" })),
    )
        .into_response())
}

pub async fn update_department(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(dept_id): Path<Uuid>,
    Json(payload): Json<UpdateDepartmentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return Ok(response),
    };
    if let Err(response) = actor.require_permission(codes::ROLES_UPDATE_ALL) {
        return Ok(response);
    }

    department_service::update_department(&pool, dept_id, payload).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "อัปเดตฝ่ายสำเร็จ" })).into_response())
}
