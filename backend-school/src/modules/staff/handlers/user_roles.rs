use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use uuid::Uuid;

use crate::api_response::{ApiErrorResponse, ApiResponse, EmptyData, IdData, UuidIdData};
use crate::error::AppError;
use crate::modules::staff::models::*;
use crate::modules::staff::services::user_role_service::{self, AssignRoleOutcome};
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

#[utoipa::path(
    get,
    path = "/api/users/{id}/roles",
    operation_id = "getUserRoles",
    tag = "roles",
    params(("id" = Uuid, Path, description = "User ID")),
    responses(
        (status = 200, description = "User role assignments", body = ApiResponse<Vec<UserRoleAssignmentResponse>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn get_user_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_READ_ALL)?;
    Ok(
        match user_role_service::get_user_roles(&pool, user_id).await {
            Ok(roles) => (StatusCode::OK, Json(ApiResponse::ok(roles))).into_response(),
            Err(e) => return Err(e),
        },
    )
}

#[utoipa::path(
    post,
    path = "/api/users/{id}/roles",
    operation_id = "assignUserRole",
    tag = "roles",
    params(("id" = Uuid, Path, description = "User ID")),
    request_body = AssignRoleRequest,
    responses(
        (status = 201, description = "Role assigned", body = ApiResponse<UuidIdData>),
        (status = 400, description = "Role cannot be assigned", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse),
        (status = 404, description = "User or role not found", body = ApiErrorResponse)
    )
)]
pub async fn assign_user_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<AssignRoleRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let tenant = context.tenant.subdomain.clone();
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_ASSIGN_ALL)?;

    Ok(
        match user_role_service::assign_user_role(&pool, user_id, payload).await {
            Ok(AssignRoleOutcome::Created(id)) => {
                state.permission_cache.invalidate_user(&tenant, user_id);
                state.notify_permission_changed(&tenant, user_id);
                (
                    StatusCode::CREATED,
                    Json(ApiResponse::with_message(
                        IdData::new(id),
                        "มอบหมายบทบาทสำเร็จ",
                    )),
                )
                    .into_response()
            }
            Ok(AssignRoleOutcome::UserNotFound) => (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse::new("ไม่พบผู้ใช้")),
            )
                .into_response(),
            Ok(AssignRoleOutcome::RoleNotFound) => (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse::new("ไม่พบบทบาทหรือบทบาทไม่ active")),
            )
                .into_response(),
            Ok(AssignRoleOutcome::UserTypeMismatch(role_user_type)) => (
                StatusCode::BAD_REQUEST,
                Json(ApiErrorResponse::new(format!(
                    "ไม่สามารถมอบหมายบทบาทนี้ได้: บทบาทนี้สำหรับ {} เท่านั้น",
                    match role_user_type.as_str() {
                        "staff" => "บุคลากร",
                        "student" => "นักเรียน",
                        "parent" => "ผู้ปกครอง",
                        _ => "ผู้ใช้อื่น",
                    }
                ))),
            )
                .into_response(),
            Err(e) => return Err(e),
        },
    )
}

#[utoipa::path(
    delete,
    path = "/api/users/{id}/roles/{role_id}",
    operation_id = "removeUserRole",
    tag = "roles",
    params(
        ("id" = Uuid, Path, description = "User ID"),
        ("role_id" = Uuid, Path, description = "Role ID")
    ),
    responses(
        (status = 200, description = "Role assignment removed", body = ApiResponse<EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse),
        (status = 404, description = "Role assignment not found", body = ApiErrorResponse)
    )
)]
pub async fn remove_user_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, role_id)): Path<(Uuid, Uuid)>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let tenant = context.tenant.subdomain.clone();
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_REMOVE_ALL)?;

    Ok(
        match user_role_service::remove_user_role(&pool, user_id, role_id).await {
            Ok(true) => {
                state.permission_cache.invalidate_user(&tenant, user_id);
                state.notify_permission_changed(&tenant, user_id);
                (
                    StatusCode::OK,
                    Json(ApiResponse::empty_with_message("ลบบทบาทสำเร็จ")),
                )
                    .into_response()
            }
            Ok(false) => (
                StatusCode::NOT_FOUND,
                Json(ApiErrorResponse::new("ไม่พบการมอบหมายบทบาท")),
            )
                .into_response(),
            Err(e) => return Err(e),
        },
    )
}

#[utoipa::path(
    get,
    path = "/api/users/{id}/permissions",
    operation_id = "listUserEffectivePermissions",
    tag = "permissions",
    params(("id" = Uuid, Path, description = "User ID")),
    responses(
        (status = 200, description = "Effective permission codes", body = ApiResponse<Vec<String>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn get_user_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_READ_ALL)?;
    Ok(
        match user_role_service::get_user_permissions(&pool, user_id).await {
            Ok(perms) => (StatusCode::OK, Json(ApiResponse::ok(perms))).into_response(),
            Err(e) => return Err(e),
        },
    )
}
