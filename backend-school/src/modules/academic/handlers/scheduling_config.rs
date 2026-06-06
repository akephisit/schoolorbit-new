use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::academic::models::scheduling::TimeSlot;
use crate::modules::academic::services::scheduling_config_service;
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

#[derive(Deserialize)]
pub struct UpdateInstructorConstraintRequest {
    pub hard_unavailable_slots: Option<Vec<TimeSlot>>,
    pub max_periods_per_day: Option<i32>,
    pub preferred_slots: Option<Vec<TimeSlot>>,
    pub assigned_room_id: Option<Uuid>,
    pub clear_assigned_room: Option<bool>,
    pub priority: Option<i32>,
}

#[derive(Deserialize)]
pub struct ReorderInstructorPriorityRequest {
    pub instructor_ids: Vec<Uuid>,
}

#[derive(Serialize)]
pub struct SchedulerSettingsView {
    pub default_max_consecutive: i32,
}

#[derive(Deserialize)]
pub struct UpdateSchedulerSettingsRequest {
    pub default_max_consecutive: Option<i32>,
}

#[derive(Deserialize)]
pub struct UpdateSubjectConstraintRequest {
    pub min_consecutive_periods: Option<i32>,
    pub max_consecutive_periods: Option<i32>,
    pub allow_single_period: Option<bool>,
    pub allowed_period_ids: Option<Vec<Uuid>>,
    pub allowed_days: Option<Vec<String>>,
}

#[derive(Deserialize)]
pub struct UpdateClassroomCourseConstraintRequest {
    pub consecutive_pattern: Option<Vec<i32>>,
    pub same_day_unique: Option<bool>,
    pub hard_unavailable_slots: Option<Vec<TimeSlot>>,
}

#[derive(Deserialize)]
pub struct ListCcConstraintsQuery {
    pub instructor_id: Option<Uuid>,
}

#[derive(Deserialize)]
pub struct SetCcRoomsRequest {
    pub rooms: Vec<CcRoomItem>,
}

#[derive(Deserialize)]
pub struct CcRoomItem {
    pub room_id: Uuid,
    pub rank: i32,
    pub is_required: Option<bool>,
}

pub async fn list_classroom_course_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(q): Query<ListCcConstraintsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let rows = scheduling_config_service::list_classroom_course_constraints(&pool, q.instructor_id)
        .await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

pub async fn update_classroom_course_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(cc_id): Path<Uuid>,
    Json(payload): Json<UpdateClassroomCourseConstraintRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    if let Some(ref pattern) = payload.consecutive_pattern {
        scheduling_config_service::validate_consecutive_pattern(&pool, cc_id, pattern).await?;
    }
    scheduling_config_service::update_classroom_course_constraints(
        &pool,
        cc_id,
        payload.consecutive_pattern,
        payload.same_day_unique,
        payload.hard_unavailable_slots,
    )
    .await?;
    Ok(Json(ApiResponse::empty_with_message(
        "Updated classroom course constraints",
    ))
    .into_response())
}

pub async fn list_cc_preferred_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(cc_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let rows = scheduling_config_service::list_cc_preferred_rooms(&pool, cc_id).await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

pub async fn set_cc_preferred_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(cc_id): Path<Uuid>,
    Json(payload): Json<SetCcRoomsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    let rooms: Vec<(Uuid, i32, bool)> = payload
        .rooms
        .into_iter()
        .map(|r| (r.room_id, r.rank, r.is_required.unwrap_or(false)))
        .collect();
    let count = scheduling_config_service::set_cc_preferred_rooms(&pool, cc_id, rooms).await?;
    Ok(Json(ApiResponse::empty_with_message(format!(
        "Updated {} rooms",
        count
    )))
    .into_response())
}

pub async fn list_all_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let rows = scheduling_config_service::list_all_rooms(&pool).await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

pub async fn list_instructor_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let rows = scheduling_config_service::list_instructor_constraints(&pool).await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

pub async fn reorder_instructor_priority(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ReorderInstructorPriorityRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    let n = scheduling_config_service::reorder_instructor_priority(&pool, payload.instructor_ids)
        .await?;
    let msg = if n == 0 {
        "No changes".to_string()
    } else {
        format!("Reordered {} instructors", n)
    };
    Ok(Json(ApiResponse::empty_with_message(msg)).into_response())
}

pub async fn get_scheduler_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let default_max_consecutive = scheduling_config_service::get_scheduler_settings(&pool).await?;
    Ok(Json(ApiResponse::ok(SchedulerSettingsView {
        default_max_consecutive,
    }))
    .into_response())
}

pub async fn update_scheduler_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateSchedulerSettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    scheduling_config_service::update_scheduler_settings(&pool, payload.default_max_consecutive)
        .await?;
    Ok(Json(ApiResponse::empty_with_message(
        "Updated scheduler settings",
    ))
    .into_response())
}

pub async fn update_instructor_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(instructor_id): Path<Uuid>,
    Json(payload): Json<UpdateInstructorConstraintRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    scheduling_config_service::update_instructor_constraints(
        &pool,
        instructor_id,
        scheduling_config_service::InstructorConstraintUpdate {
            hard_unavailable_slots: payload.hard_unavailable_slots,
            max_periods_per_day: payload.max_periods_per_day,
            preferred_slots: payload.preferred_slots,
            priority: payload.priority,
            assigned_room_id: payload.assigned_room_id,
            clear_assigned_room: payload.clear_assigned_room.unwrap_or(false),
        },
    )
    .await?;
    Ok(Json(ApiResponse::empty_with_message(
        "Updated instructor constraints",
    ))
    .into_response())
}

pub async fn list_subject_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let rows = scheduling_config_service::list_subject_constraints(&pool).await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

pub async fn update_subject_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(subject_id): Path<Uuid>,
    Json(payload): Json<UpdateSubjectConstraintRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    scheduling_config_service::update_subject_constraints(
        &pool,
        subject_id,
        payload.min_consecutive_periods,
        payload.max_consecutive_periods,
        payload.allow_single_period,
        payload.allowed_period_ids,
        payload.allowed_days,
    )
    .await?;
    Ok(Json(ApiResponse::empty_with_message(
        "Updated subject constraints",
    ))
    .into_response())
}
