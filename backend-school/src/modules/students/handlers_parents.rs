use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::api_response::ApiResponse;
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
