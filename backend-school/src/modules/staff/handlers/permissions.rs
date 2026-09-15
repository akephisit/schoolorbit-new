use crate::utils::request_context::actor_tenant_context_from_session;
use crate::AppState;
use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use school_auth::session_service::AuthenticatedSession;
use school_http::HttpError as AppError;
use school_http::{ApiErrorResponse, ApiResponse};
use school_permissions::registry::codes;
use school_staff::models::Permission;
use school_staff::services::permission_service;

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
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
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
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::SETTINGS_READ_ALL)?;

    let grouped = permission_service::list_permissions_by_module(&pool).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(grouped))).into_response())
}
