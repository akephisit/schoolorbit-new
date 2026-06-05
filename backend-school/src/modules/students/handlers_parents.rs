use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::load_actor_context;
use crate::modules::students::services as student_service;
use crate::permissions::registry::codes;
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;

use super::models::CreateParentRequest;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    resolve_tenant_pool(state, headers).await
}

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
    let pool = get_pool(&state, &headers).await?;

    let actor = load_actor_context(&headers, &pool, &state.permission_cache).await?;
    actor.require_permission(codes::STUDENT_UPDATE_ALL)?;

    student_service::add_parent_to_student(&pool, student_id, payload).await?;

    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": {}, "message": "เพิ่มผู้ปกครองสำเร็จ" })),
    )
        .into_response())
}

/// DELETE /api/students/:id/parents/:parentId - ลบความสัมพันธ์ผู้ปกครอง
pub async fn remove_parent_from_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((student_id, parent_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let actor = load_actor_context(&headers, &pool, &state.permission_cache).await?;
    actor.require_permission(codes::STUDENT_UPDATE_ALL)?;

    student_service::remove_parent_from_student(&pool, student_id, parent_id).await?;

    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": {}, "message": "ลบผู้ปกครองสำเร็จ" })),
    )
        .into_response())
}
