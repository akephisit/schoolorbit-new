use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::parents::models::ParentProfile;
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
#[utoipa::path(
    get,
    path = "/api/parent/profile",
    operation_id = "getParentProfile",
    tag = "parent",
    responses(
        (status = 200, description = "Current parent profile and linked children", body = ApiResponse<ParentProfile>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Parent account required", body = ApiErrorResponse),
        (status = 404, description = "Parent profile not found", body = ApiErrorResponse)
    )
)]
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
#[utoipa::path(
    get,
    path = "/api/parent/students/{student_id}",
    operation_id = "getParentChildProfile",
    tag = "parent",
    params(("student_id" = Uuid, Path, description = "Linked student user ID")),
    responses(
        (status = 200, description = "Linked child's profile", body = ApiResponse<crate::modules::students::models::StudentProfile>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Parent-child access denied", body = ApiErrorResponse),
        (status = 404, description = "Child profile not found", body = ApiErrorResponse)
    )
)]
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

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ChildTimetableQuery {
    pub academic_semester_id: Option<Uuid>,
}

#[derive(Debug, serde::Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ChildExamScheduleQuery {
    pub academic_semester_id: Option<Uuid>,
}

/// GET /api/parent/students/:student_id/timetable
/// ผู้ปกครองดูตารางเรียนของบุตรหลาน — verify ownership ผ่าน student_parents
#[utoipa::path(
    get,
    path = "/api/parent/students/{student_id}/timetable",
    operation_id = "getParentChildTimetable",
    tag = "parent",
    params(
        ("student_id" = Uuid, Path, description = "Linked student user ID"),
        ChildTimetableQuery
    ),
    responses(
        (status = 200, description = "Linked child's timetable entries", body = ApiResponse<Vec<crate::modules::academic::models::timetable::TimetableEntry>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Parent-child access denied", body = ApiErrorResponse)
    )
)]
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

/// GET /api/parent/students/:student_id/exam-schedules
/// ผู้ปกครองดูตารางสอบของบุตรหลาน — service verifies ownership ผ่าน student_parents
#[utoipa::path(
    get,
    path = "/api/parent/students/{student_id}/exam-schedules",
    operation_id = "getParentChildExamSchedule",
    tag = "parent",
    params(
        ("student_id" = Uuid, Path, description = "Linked student user ID"),
        ChildExamScheduleQuery
    ),
    responses(
        (status = 200, description = "Linked child's published exam schedule", body = ApiResponse<Vec<crate::modules::academic::models::exam_schedule::PersonalExamScheduleRound>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Parent-child access denied", body = ApiErrorResponse)
    )
)]
pub async fn get_child_exam_schedule(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
    Query(query): Query<ChildExamScheduleQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let schedule = parent_service::get_child_exam_schedule(
        &pool,
        actor.user_id,
        student_id,
        query.academic_semester_id,
    )
    .await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(schedule))))
}

/// GET /api/parent/students/:student_id/calendar/events
#[utoipa::path(
    get,
    path = "/api/parent/students/{student_id}/calendar/events",
    operation_id = "getParentChildCalendarEvents",
    tag = "parent",
    params(
        ("student_id" = Uuid, Path, description = "Linked student user ID"),
        ("from" = Option<chrono::NaiveDate>, Query, description = "Inclusive range start"),
        ("to" = Option<chrono::NaiveDate>, Query, description = "Inclusive range end"),
        ("category_id" = Option<Uuid>, Query, description = "Calendar category ID"),
        ("tag_id" = Option<Uuid>, Query, description = "Calendar tag ID"),
        ("audience" = Option<String>, Query, description = "Audience: all, staff, student, or parent"),
        ("visibility" = Option<String>, Query, description = "Visibility: public or private"),
        ("q" = Option<String>, Query, description = "Title or description search")
    ),
    responses(
        (status = 200, description = "Calendar events visible for the linked child", body = ApiResponse<Vec<crate::modules::calendar::models::CalendarViewerEvent>>),
        (status = 400, description = "Invalid date range", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Parent-child access denied", body = ApiErrorResponse)
    )
)]
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
