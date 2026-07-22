use crate::api_response::{ApiErrorResponse, ApiResponse, EmptyData, IdData, UuidIdData};
use crate::error::AppError;
use crate::modules::staff::models::*;
use crate::modules::staff::services::{organization_unit_service, role_service};
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use utoipa::IntoParams;
use uuid::Uuid;

#[derive(Debug, Default, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListManagedResourcesQuery {
    pub include_inactive: Option<bool>,
}

// ============================================
// Roles
// ============================================

#[utoipa::path(
    get,
    path = "/api/roles",
    operation_id = "listRoles",
    tag = "roles",
    params(ListManagedResourcesQuery),
    responses(
        (status = 200, description = "Roles", body = ApiResponse<Vec<Role>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListManagedResourcesQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_READ_ALL)?;

    let roles = role_service::list_roles(&pool, query.include_inactive.unwrap_or(false)).await?;
    Ok(Json(ApiResponse::ok(roles)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/roles/{id}",
    operation_id = "getRole",
    tag = "roles",
    params(("id" = Uuid, Path, description = "Role ID")),
    responses(
        (status = 200, description = "Role", body = ApiResponse<Role>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse),
        (status = 404, description = "Role not found", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    post,
    path = "/api/roles",
    operation_id = "createRole",
    tag = "roles",
    request_body = CreateRoleRequest,
    responses(
        (status = 201, description = "Role created", body = ApiResponse<UuidIdData>),
        (status = 400, description = "Invalid role", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    put,
    path = "/api/roles/{id}",
    operation_id = "updateRole",
    tag = "roles",
    params(("id" = Uuid, Path, description = "Role ID")),
    request_body = UpdateRoleRequest,
    responses(
        (status = 200, description = "Role updated", body = ApiResponse<EmptyData>),
        (status = 400, description = "Invalid role", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse),
        (status = 404, description = "Role not found", body = ApiErrorResponse),
        (status = 409, description = "Protected role or invalid status transition", body = ApiErrorResponse)
    )
)]
pub async fn update_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(role_id): Path<Uuid>,
    Json(payload): Json<UpdateRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let tenant = context.tenant.subdomain.clone();
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_UPDATE_ALL)?;
    if payload.is_active == Some(false) {
        actor.require_permission(codes::ROLES_DELETE_ALL)?;
    }

    let permissions_changed = payload.permissions.is_some();
    let status_outcome = role_service::update_role(&pool, role_id, payload, actor.user_id).await?;

    if permissions_changed || status_outcome.changed() {
        state.permission_cache.invalidate_tenant(&tenant);
        state.notify_all_permissions_changed(&tenant);
    }

    Ok(Json(ApiResponse::empty_with_message("อัปเดตบทบาทสำเร็จ")).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/roles/{id}",
    operation_id = "deleteRole",
    tag = "roles",
    params(("id" = Uuid, Path, description = "Role ID")),
    responses(
        (status = 200, description = "Role deactivated", body = ApiResponse<EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse),
        (status = 404, description = "Role not found", body = ApiErrorResponse),
        (status = 409, description = "System role cannot be deactivated", body = ApiErrorResponse)
    )
)]
pub async fn deactivate_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(role_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let tenant = context.tenant.subdomain.clone();
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_DELETE_ALL)?;

    let outcome = role_service::set_role_active(&pool, role_id, false, actor.user_id).await?;
    if outcome.changed() {
        state.permission_cache.invalidate_tenant(&tenant);
        state.notify_all_permissions_changed(&tenant);
    }

    Ok(Json(ApiResponse::empty_with_message("ปิดใช้งานบทบาทสำเร็จ")).into_response())
}

// ============================================
// Organization Units
// ============================================

#[utoipa::path(
    get,
    path = "/api/organization/units",
    operation_id = "listOrganizationUnits",
    tag = "organization",
    params(ListManagedResourcesQuery),
    responses(
        (status = 200, description = "Organization units", body = ApiResponse<Vec<OrganizationUnit>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_organization_units(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListManagedResourcesQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_READ_ALL)?;

    let units = organization_unit_service::list_organization_units(
        &pool,
        query.include_inactive.unwrap_or(false),
    )
    .await?;
    Ok(Json(ApiResponse::ok(units)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/organization/units/{id}",
    operation_id = "getOrganizationUnit",
    tag = "organization",
    params(("id" = Uuid, Path, description = "Organization unit ID")),
    responses(
        (status = 200, description = "Organization unit", body = ApiResponse<OrganizationUnit>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse),
        (status = 404, description = "Organization unit not found", body = ApiErrorResponse)
    )
)]
pub async fn get_organization_unit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(unit_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_READ_ALL)?;

    let unit = organization_unit_service::get_organization_unit(&pool, unit_id).await?;
    Ok(Json(ApiResponse::ok(unit)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/organization/units",
    operation_id = "createOrganizationUnit",
    tag = "organization",
    request_body = CreateOrganizationUnitRequest,
    responses(
        (status = 201, description = "Organization unit created", body = ApiResponse<UuidIdData>),
        (status = 400, description = "Invalid organization unit", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn create_organization_unit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateOrganizationUnitRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_CREATE_ALL)?;

    let unit_id = organization_unit_service::create_organization_unit(&pool, payload).await?;
    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::with_message(
            IdData::new(unit_id),
            "สร้างหน่วยงานสำเร็จ",
        )),
    )
        .into_response())
}

#[utoipa::path(
    put,
    path = "/api/organization/units/{id}",
    operation_id = "updateOrganizationUnit",
    tag = "organization",
    params(("id" = Uuid, Path, description = "Organization unit ID")),
    request_body = UpdateOrganizationUnitRequest,
    responses(
        (status = 200, description = "Organization unit updated", body = ApiResponse<EmptyData>),
        (status = 400, description = "Invalid organization unit", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse),
        (status = 404, description = "Organization unit not found", body = ApiErrorResponse),
        (status = 409, description = "Protected unit or invalid hierarchy transition", body = ApiErrorResponse)
    )
)]
pub async fn update_organization_unit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(unit_id): Path<Uuid>,
    Json(payload): Json<UpdateOrganizationUnitRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let tenant = context.tenant.subdomain.clone();
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_UPDATE_ALL)?;
    if payload.is_active == Some(false) {
        actor.require_permission(codes::ROLES_DELETE_ALL)?;
    }

    let status_outcome =
        organization_unit_service::update_organization_unit(&pool, unit_id, payload, actor.user_id)
            .await?;
    if status_outcome.changed() {
        state.permission_cache.invalidate_tenant(&tenant);
        state.notify_all_permissions_changed(&tenant);
    }
    Ok(Json(ApiResponse::empty_with_message("อัปเดตหน่วยงานสำเร็จ")).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/organization/units/{id}",
    operation_id = "deactivateOrganizationUnit",
    tag = "organization",
    params(("id" = Uuid, Path, description = "Organization unit ID")),
    responses(
        (status = 200, description = "Organization unit deactivated", body = ApiResponse<EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse),
        (status = 404, description = "Organization unit not found", body = ApiErrorResponse),
        (status = 409, description = "System unit or active child prevents deactivation", body = ApiErrorResponse)
    )
)]
pub async fn deactivate_organization_unit(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(unit_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let tenant = context.tenant.subdomain.clone();
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_DELETE_ALL)?;

    let outcome = organization_unit_service::set_organization_unit_active(
        &pool,
        unit_id,
        false,
        actor.user_id,
    )
    .await?;
    if outcome.changed() {
        state.permission_cache.invalidate_tenant(&tenant);
        state.notify_all_permissions_changed(&tenant);
    }

    Ok(Json(ApiResponse::empty_with_message("ปิดใช้งานหน่วยงานสำเร็จ")).into_response())
}
