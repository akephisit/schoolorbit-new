use crate::api_response::{ApiResponse, IdData};
use crate::error::AppError;
use crate::modules::staff::models::*;
use crate::modules::staff::services::{department_service, role_service};
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

// ============================================
// Roles
// ============================================

pub async fn list_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_READ_ALL)?;

    let roles = role_service::list_roles(&pool).await?;
    Ok(Json(ApiResponse::ok(roles)).into_response())
}

pub async fn get_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(role_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_READ_ALL)?;

    let role = role_service::get_role(&pool, role_id).await?;
    Ok(Json(ApiResponse::ok(role)).into_response())
}

pub async fn create_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_CREATE_ALL)?;

    let role_id = role_service::create_role(&pool, payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::with_message(
            IdData::new(role_id),
            "สร้างบทบาทสำเร็จ",
        )),
    )
        .into_response())
}

pub async fn update_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(role_id): Path<Uuid>,
    Json(payload): Json<UpdateRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_UPDATE_ALL)?;

    role_service::update_role(&pool, role_id, payload).await?;

    // Role permissions changed — every user with this role has stale cache
    state.permission_cache.clear_all();
    state.notify_all_permissions_changed();

    Ok(Json(ApiResponse::empty_with_message("อัปเดตบทบาทสำเร็จ")).into_response())
}

// ============================================
// Departments
// ============================================

pub async fn list_departments(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_READ_ALL)?;

    let departments = department_service::list_departments(&pool).await?;
    Ok(Json(ApiResponse::ok(departments)).into_response())
}

pub async fn get_department(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(dept_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_READ_ALL)?;

    let department = department_service::get_department(&pool, dept_id).await?;
    Ok(Json(ApiResponse::ok(department)).into_response())
}

pub async fn create_department(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateDepartmentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_CREATE_ALL)?;

    let dept_id = department_service::create_department(&pool, payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::with_message(
            IdData::new(dept_id),
            "สร้างฝ่ายสำเร็จ",
        )),
    )
        .into_response())
}

pub async fn update_department(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(dept_id): Path<Uuid>,
    Json(payload): Json<UpdateDepartmentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_UPDATE_ALL)?;

    department_service::update_department(&pool, dept_id, payload).await?;
    Ok(Json(ApiResponse::empty_with_message("อัปเดตฝ่ายสำเร็จ")).into_response())
}
