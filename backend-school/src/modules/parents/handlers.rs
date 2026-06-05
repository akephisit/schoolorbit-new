use crate::error::AppError;
use crate::middleware::permission::load_actor_context_or_error;
use crate::modules::parents::services as parent_service;
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    resolve_tenant_pool(state, headers).await
}

/// GET /api/parent/profile - ผู้ปกครองดูข้อมูลตนเองและบุตรหลาน
pub async fn get_own_parent_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = load_actor_context_or_error(&headers, &pool, &state.permission_cache).await?;
    let profile = parent_service::get_own_parent_profile(&pool, actor.user_id).await?;

    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": profile })),
    ))
}

/// GET /api/parent/students/:student_id - ผู้ปกครองดูรายละเอียดบุตรหลาน
pub async fn get_child_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = load_actor_context_or_error(&headers, &pool, &state.permission_cache).await?;
    let student = parent_service::get_child_profile(&pool, actor.user_id, student_id).await?;

    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": student })),
    ))
}

#[derive(Debug, serde::Deserialize)]
pub struct ChildTimetableQuery {
    pub academic_semester_id: Option<Uuid>,
}

/// GET /api/parent/students/:student_id/timetable
/// ผู้ปกครองดูตารางเรียนของบุตรหลาน — verify ownership ผ่าน student_parents
pub async fn get_child_timetable(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
    Query(query): Query<ChildTimetableQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let actor = load_actor_context_or_error(&headers, &pool, &state.permission_cache).await?;
    let entries = parent_service::get_child_timetable(
        &pool,
        actor.user_id,
        student_id,
        query.academic_semester_id,
    )
    .await?;

    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": entries })),
    ))
}
