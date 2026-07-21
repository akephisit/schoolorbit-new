use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::staff::models::Permission;
use crate::modules::staff::services::permission_service;
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};

// ===================================================================
// List All Permissions
// ===================================================================

#[utoipa::path(
    get,
    path = "/api/permissions",
    operation_id = "listPermissions",
    tag = "permissions",
    responses(
        (status = 200, description = "Permissions", body = ApiResponse<Vec<Permission>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::SETTINGS_READ_ALL)?;

    let permissions = permission_service::list_permissions(&pool).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(permissions))).into_response())
}

// ===================================================================
// List Permissions Grouped by Module
// ===================================================================

#[utoipa::path(
    get,
    path = "/api/permissions/modules",
    operation_id = "listPermissionsByModule",
    tag = "permissions",
    responses(
        (
            status = 200,
            description = "Permissions grouped by module",
            body = ApiResponse<std::collections::HashMap<String, Vec<Permission>>>
        ),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_permissions_by_module(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::SETTINGS_READ_ALL)?;

    let grouped = permission_service::list_permissions_by_module(&pool).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(grouped))).into_response())
}
