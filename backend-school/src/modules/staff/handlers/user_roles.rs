use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::check_permission;
use crate::modules::staff::models::*;
use crate::modules::staff::services::user_role_service::{self, AssignRoleOutcome};
use crate::permissions::registry::codes;
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, Response> {
    resolve_tenant_pool(state, headers)
        .await
        .map_err(IntoResponse::into_response)
}

fn err_response<E: Into<AppError>>(e: E) -> Response {
    e.into().into_response()
}

pub async fn get_user_roles(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
) -> Response {
    let pool = match get_pool(&state, &headers).await {
        Ok(p) => p,
        Err(r) => return r,
    };
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ROLES_READ_ALL,
        &state.permission_cache,
    )
    .await
    {
        return r;
    }
    match user_role_service::get_user_roles(&pool, user_id).await {
        Ok(roles) => (
            StatusCode::OK,
            Json(json!({ "success": true, "data": roles })),
        )
            .into_response(),
        Err(e) => err_response(e),
    }
}

pub async fn assign_user_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
    Json(payload): Json<AssignRoleRequest>,
) -> Response {
    let pool = match get_pool(&state, &headers).await {
        Ok(p) => p,
        Err(r) => return r,
    };
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ROLES_ASSIGN_ALL,
        &state.permission_cache,
    )
    .await
    {
        return r;
    }

    match user_role_service::assign_user_role(&pool, user_id, payload).await {
        Ok(AssignRoleOutcome::Created(id)) => {
            state.permission_cache.invalidate(&user_id);
            (StatusCode::CREATED, Json(json!({ "success": true, "data": { "id": id }, "message": "มอบหมายบทบาทสำเร็จ" }))).into_response()
        }
        Ok(AssignRoleOutcome::UserNotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "ไม่พบผู้ใช้" })),
        )
            .into_response(),
        Ok(AssignRoleOutcome::RoleNotFound) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "ไม่พบบทบาทหรือบทบาทไม่ active" })),
        )
            .into_response(),
        Ok(AssignRoleOutcome::UserTypeMismatch(role_user_type)) => (
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": format!(
                    "ไม่สามารถมอบหมายบทบาทนี้ได้: บทบาทนี้สำหรับ {} เท่านั้น",
                    match role_user_type.as_str() {
                        "staff" => "บุคลากร",
                        "student" => "นักเรียน",
                        "parent" => "ผู้ปกครอง",
                        _ => "ผู้ใช้อื่น"
                    }
                ) })),
        )
            .into_response(),
        Err(e) => err_response(e),
    }
}

pub async fn remove_user_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((user_id, role_id)): Path<(Uuid, Uuid)>,
) -> Response {
    let pool = match get_pool(&state, &headers).await {
        Ok(p) => p,
        Err(r) => return r,
    };
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ROLES_REMOVE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return r;
    }

    match user_role_service::remove_user_role(&pool, user_id, role_id).await {
        Ok(true) => {
            state.permission_cache.invalidate(&user_id);
            (
                StatusCode::OK,
                Json(json!({ "success": true, "data": {}, "message": "ลบบทบาทสำเร็จ" })),
            )
                .into_response()
        }
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "ไม่พบการมอบหมายบทบาท" })),
        )
            .into_response(),
        Err(e) => err_response(e),
    }
}

pub async fn get_user_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(user_id): Path<Uuid>,
) -> Response {
    let pool = match get_pool(&state, &headers).await {
        Ok(p) => p,
        Err(r) => return r,
    };
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ROLES_READ_ALL,
        &state.permission_cache,
    )
    .await
    {
        return r;
    }
    match user_role_service::get_user_permissions(&pool, user_id).await {
        Ok(perms) => (
            StatusCode::OK,
            Json(json!({ "success": true, "data": perms })),
        )
            .into_response(),
        Err(e) => err_response(e),
    }
}
