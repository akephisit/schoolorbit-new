use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::parents::services as parent_service;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

/// GET /api/parent/profile - ผู้ปกครองดูข้อมูลตนเองและบุตรหลาน
pub async fn get_own_parent_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let profile = parent_service::get_own_parent_profile(&pool, actor.user_id).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(profile))))
}

/// GET /api/parent/students/:student_id - ผู้ปกครองดูรายละเอียดบุตรหลาน
pub async fn get_child_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let student = parent_service::get_child_profile(&pool, actor.user_id, student_id).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(student))))
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
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let entries = parent_service::get_child_timetable(
        &pool,
        actor.user_id,
        student_id,
        query.academic_semester_id,
    )
    .await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(entries))))
}

/// GET /api/parent/students/:student_id/calendar/events
pub async fn get_child_calendar_events(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
    Query(query): Query<crate::modules::calendar::models::CalendarEventQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let events = parent_service::get_child_calendar_events(
        &context.tenant.pool,
        context.actor.user_id,
        student_id,
        query,
    )
    .await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(events))))
}
