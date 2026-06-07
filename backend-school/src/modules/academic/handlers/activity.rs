use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::academic::models::activity::*;
use crate::modules::academic::services::activity_service;
use crate::permissions::registry::codes;
use crate::policies::activity_access_policy;
use crate::utils::request_context::{
    actor_tenant_context, current_user_tenant_context_from_headers,
};
use crate::AppState;

#[derive(Debug, Serialize)]
struct InsertedData<T> {
    inserted: T,
}

#[derive(Debug, Serialize)]
struct AddedData<T> {
    added: T,
}

#[derive(Debug, Serialize)]
struct DeletedCountData<T> {
    deleted_count: T,
}

#[derive(Debug, Serialize)]
struct CountData<T> {
    count: T,
}

#[derive(Debug, Deserialize)]
pub struct AddSlotInstructorRequest {
    user_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct AddSlotInstructorsBatchRequest {
    user_ids: Vec<Uuid>,
}

// ============================================
// Activity Slots
// ============================================

pub async fn list_activity_slots(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<ActivitySlotFilter>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let access = activity_access_policy::resolve_activity_list_access(&actor)?;
    let slots = activity_service::list_slots(&pool, filter, access).await?;
    Ok(Json(ApiResponse::ok(slots)).into_response())
}

pub async fn update_activity_slot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateActivitySlotRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACTIVITY_MANAGE_ALL)?;
    let row = activity_service::update_slot(&pool, id, body).await?;
    Ok(Json(ApiResponse::ok(row)).into_response())
}

pub async fn delete_activity_slot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACTIVITY_MANAGE_ALL)?;
    activity_service::delete_slot(&pool, id).await?;
    Ok(Json(ApiResponse::empty_with_message("ลบช่องกิจกรรมแล้ว")).into_response())
}

// ============================================
// Activity Groups
// ============================================

pub async fn list_activity_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<ActivityGroupFilter>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let access = activity_access_policy::resolve_activity_list_access(&actor)?;
    let groups = activity_service::list_groups(&pool, filter, access).await?;
    Ok(Json(ApiResponse::ok(groups)).into_response())
}

pub async fn create_activity_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateActivityGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let outcome = activity_service::create_group(&pool, &actor, body).await?;
    match outcome {
        activity_service::CreateGroupOutcome::Created(row) => {
            Ok(Json(ApiResponse::ok(*row)).into_response())
        }
        activity_service::CreateGroupOutcome::SlotClosed => {
            Ok(Json(ApiErrorResponse::new("ช่องกิจกรรมนี้ยังไม่เปิดให้ลงทะเบียน")).into_response())
        }
        activity_service::CreateGroupOutcome::InstructorNotInSlot => {
            Ok(Json(ApiErrorResponse::new("ครูคนนี้ไม่ได้อยู่ในรายชื่อครูของช่องกิจกรรมนี้")).into_response())
        }
    }
}

pub async fn update_activity_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateActivityGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let row = activity_service::update_group(&pool, &actor, id, body).await?;
    Ok(Json(ApiResponse::ok(row)).into_response())
}

pub async fn delete_activity_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACTIVITY_MANAGE_ALL)?;
    activity_service::delete_group(&pool, id).await?;
    Ok(Json(ApiResponse::empty_with_message("ลบกลุ่มกิจกรรมแล้ว")).into_response())
}

// ============================================
// Members
// ============================================

pub async fn list_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    activity_access_policy::can_read_activity_group(&pool, &actor, group_id).await?;
    let members = activity_service::list_members(&pool, group_id).await?;
    Ok(Json(ApiResponse::ok(members)).into_response())
}

pub async fn add_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
    Json(body): Json<AddMembersRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACTIVITY_MANAGE_MEMBERS_ALL)?;
    match activity_service::add_members(&pool, group_id, body.student_ids).await? {
        activity_service::AddMembersOutcome::Inserted(n) => {
            Ok(Json(ApiResponse::ok(InsertedData { inserted: n })).into_response())
        }
        activity_service::AddMembersOutcome::OverCapacity(cap) => {
            Ok(Json(ApiErrorResponse::new(format!("จำนวนเกินที่รับได้ ({cap} คน)"))).into_response())
        }
    }
}

pub async fn my_enrollments(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_headers(&state, &headers).await?;
    let pool = context.tenant.pool;
    let user_id = context.user_id;
    let ids = activity_service::my_enrollments(&pool, user_id).await?;
    Ok(Json(ApiResponse::ok(ids)).into_response())
}

pub async fn self_enroll(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_headers(&state, &headers).await?;
    let pool = context.tenant.pool;
    let user_id = context.user_id;

    match activity_service::self_enroll(&pool, group_id, user_id).await? {
        activity_service::SelfEnrollOutcome::Enrolled => {
            Ok(Json(ApiResponse::empty_with_message("ลงทะเบียนสำเร็จ")).into_response())
        }
        activity_service::SelfEnrollOutcome::AlreadyEnrolled => {
            Ok(Json(ApiErrorResponse::new("ลงทะเบียนแล้วก่อนหน้านี้")).into_response())
        }
        activity_service::SelfEnrollOutcome::NotSelfRegistrationType => {
            Ok(Json(ApiErrorResponse::new("กลุ่มนี้ไม่เปิดให้ลงทะเบียนด้วยตนเอง")).into_response())
        }
        activity_service::SelfEnrollOutcome::NotOpen => {
            Ok(Json(ApiErrorResponse::new("ยังไม่เปิดรับสมัคร")).into_response())
        }
        activity_service::SelfEnrollOutcome::Full => {
            Ok(Json(ApiErrorResponse::new("กลุ่มเต็มแล้ว")).into_response())
        }
        activity_service::SelfEnrollOutcome::ClassroomNotAllowed => {
            Ok(Json(ApiErrorResponse::new("ห้องเรียนของคุณไม่อยู่ในห้องที่รับ")).into_response())
        }
    }
}

pub async fn self_unenroll(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_headers(&state, &headers).await?;
    let pool = context.tenant.pool;
    let user_id = context.user_id;
    activity_service::self_unenroll(&pool, group_id, user_id).await?;
    Ok(Json(ApiResponse::empty_with_message("ยกเลิกลงทะเบียนแล้ว")).into_response())
}

pub async fn remove_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((group_id, student_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACTIVITY_MANAGE_MEMBERS_ALL)?;
    activity_service::remove_member(&pool, group_id, student_id).await?;
    Ok(Json(ApiResponse::empty_with_message("ลบสมาชิกแล้ว")).into_response())
}

pub async fn update_member_result(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(member_id): Path<Uuid>,
    Json(body): Json<UpdateMemberResultRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACTIVITY_MANAGE_MEMBERS_ALL)?;
    activity_service::update_member_result(&pool, member_id, &body.result).await?;
    Ok(Json(ApiResponse::empty_with_message("บันทึกผลแล้ว")).into_response())
}

// ============================================
// Group Instructors
// ============================================

#[derive(serde::Deserialize)]
pub struct InstructorRoleRequest {
    pub instructor_id: Uuid,
    pub role: Option<String>,
}

pub async fn list_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    activity_access_policy::can_read_activity_group(&pool, &actor, group_id).await?;
    let rows = activity_service::list_group_instructors(&pool, group_id).await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

pub async fn add_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
    Json(body): Json<InstructorRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let role = body.role.unwrap_or_else(|| "assistant".to_string());
    activity_service::add_group_instructor(&pool, &actor, group_id, body.instructor_id, &role)
        .await?;
    Ok(Json(ApiResponse::empty_with_message("เพิ่มครูแล้ว")).into_response())
}

pub async fn remove_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((group_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    activity_service::remove_group_instructor(&pool, &actor, group_id, instructor_id).await?;
    Ok(Json(ApiResponse::empty_with_message("ลบครูแล้ว")).into_response())
}

// ============================================
// Slot Instructors
// ============================================

pub async fn list_slot_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACTIVITY_READ_ALL)?;
    let rows = activity_service::list_slot_instructors(&pool, slot_id).await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

pub async fn add_slot_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
    Json(body): Json<AddSlotInstructorRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACTIVITY_MANAGE_ALL)?;
    activity_service::add_slot_instructor(&pool, slot_id, body.user_id).await?;
    Ok(Json(ApiResponse::empty_with_message("เพิ่มครูแล้ว")).into_response())
}

pub async fn add_slot_instructors_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
    Json(body): Json<AddSlotInstructorsBatchRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACTIVITY_MANAGE_ALL)?;
    if body.user_ids.is_empty() {
        return Ok(Json(ApiResponse::with_message(
            AddedData { added: 0 },
            "ไม่มีครูที่จะเพิ่ม",
        ))
        .into_response());
    }

    let added = activity_service::add_slot_instructors_batch(&pool, slot_id, body.user_ids).await?;
    Ok(Json(ApiResponse::with_message(AddedData { added }, "เพิ่มครูแล้ว")).into_response())
}

pub async fn remove_slot_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((slot_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACTIVITY_MANAGE_ALL)?;
    activity_service::remove_slot_instructor(&pool, slot_id, user_id).await?;
    Ok(Json(ApiResponse::empty_with_message("ลบครูแล้ว")).into_response())
}

pub async fn delete_slot_timetable_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    let n = activity_service::delete_slot_timetable_entries(&pool, slot_id).await?;
    Ok(Json(ApiResponse::with_message(
        DeletedCountData { deleted_count: n },
        "ลบรายการตารางสอนแล้ว",
    ))
    .into_response())
}

pub async fn delete_all_slot_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACTIVITY_MANAGE_ALL)?;
    let n = activity_service::delete_all_slot_groups(&pool, slot_id).await?;
    Ok(Json(ApiResponse::with_message(
        DeletedCountData { deleted_count: n },
        "ลบกิจกรรมทั้งหมดแล้ว",
    ))
    .into_response())
}

pub async fn remove_all_slot_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACTIVITY_MANAGE_ALL)?;
    let n = activity_service::remove_all_slot_instructors(&pool, slot_id).await?;
    Ok(Json(ApiResponse::with_message(
        DeletedCountData { deleted_count: n },
        "ลบครูทั้งหมดแล้ว",
    ))
    .into_response())
}

// ============================================
// Slot Classroom Assignments
// ============================================

pub async fn list_slot_classroom_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACTIVITY_READ_ALL)?;
    let rows = activity_service::list_slot_classroom_assignments(&pool, slot_id).await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

pub async fn batch_upsert_slot_classroom_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
    Json(body): Json<BatchUpsertSlotClassroomAssignmentsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACTIVITY_MANAGE_ALL)?;
    let n = activity_service::batch_upsert_slot_classroom_assignments(&pool, slot_id, body).await?;
    Ok(Json(ApiResponse::with_message(
        CountData { count: n },
        "บันทึกสำเร็จ",
    ))
    .into_response())
}

pub async fn delete_all_slot_classroom_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACTIVITY_MANAGE_ALL)?;
    let n = activity_service::delete_all_slot_classroom_assignments(&pool, slot_id).await?;
    Ok(Json(ApiResponse::with_message(
        DeletedCountData { deleted_count: n },
        "ลบครูประจำห้องทั้งหมดแล้ว",
    ))
    .into_response())
}

pub async fn delete_slot_classroom_assignment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((slot_id, assignment_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACTIVITY_MANAGE_ALL)?;
    activity_service::delete_slot_classroom_assignment(&pool, slot_id, assignment_id).await?;
    Ok(Json(ApiResponse::empty_with_message("ลบสำเร็จ")).into_response())
}
