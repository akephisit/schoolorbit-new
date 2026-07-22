use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::students::services as student_service;
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

use super::models::CreateParentRequest;

// -----------------------------------------------------------------------------
// Parent Management Handlers (New)
// -----------------------------------------------------------------------------

/// POST /api/students/:id/parents - เพิ่มผู้ปกครองให้นักเรียนที่มีอยู่
#[utoipa::path(
    post,
    path = "/api/students/{id}/parents",
    operation_id = "addStudentParent",
    tag = "student",
    params(("id" = Uuid, Path, description = "Student user ID")),
    request_body = CreateParentRequest,
    responses(
        (status = 200, description = "Parent linked to student", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Student not found", body = ApiErrorResponse)
    )
)]
pub async fn add_parent_to_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
    Json(payload): Json<CreateParentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::STUDENT_UPDATE_ALL)?;

    student_service::add_parent_to_student(&pool, student_id, payload).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("เพิ่มผู้ปกครองสำเร็จ")),
    )
        .into_response())
}

/// DELETE /api/students/:id/parents/:parentId - ลบความสัมพันธ์ผู้ปกครอง
#[utoipa::path(
    delete,
    path = "/api/students/{id}/parents/{parent_id}",
    operation_id = "removeStudentParent",
    tag = "student",
    params(
        ("id" = Uuid, Path, description = "Student user ID"),
        ("parent_id" = Uuid, Path, description = "Parent user ID")
    ),
    responses(
        (status = 200, description = "Parent link removed", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Parent relationship not found", body = ApiErrorResponse)
    )
)]
pub async fn remove_parent_from_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((student_id, parent_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::STUDENT_UPDATE_ALL)?;

    student_service::remove_parent_from_student(&pool, student_id, parent_id).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("ลบผู้ปกครองสำเร็จ")),
    )
        .into_response())
}
