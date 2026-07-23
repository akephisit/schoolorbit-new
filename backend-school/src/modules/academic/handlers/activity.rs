use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
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

#[derive(Debug, Serialize, ToSchema)]
pub struct ActivityInsertedCountData {
    pub inserted: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ActivityAddedCountData {
    pub added: usize,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ActivityDeletedCountData {
    pub deleted_count: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct ActivityProcessedCountData {
    pub count: usize,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddSlotInstructorRequest {
    pub user_id: Uuid,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddSlotInstructorsBatchRequest {
    pub user_ids: Vec<Uuid>,
}

// ============================================
// Activity Slots
// ============================================

#[utoipa::path(
    get,
    path = "/api/academic/activity-slots",
    operation_id = "listActivitySlots",
    tag = "academic",
    params(ActivitySlotFilter),
    responses(
        (status = 200, description = "Activity slots visible to the caller", body = ApiResponse<Vec<ActivitySlot>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Activity read permission denied", body = ApiErrorResponse),
        (status = 500, description = "Activity slots could not be loaded", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    get,
    path = "/api/academic/activity-slots/timetable-context",
    operation_id = "getActivitySlotTimetableContext",
    tag = "academic",
    params(ActivityTimetableContextQuery),
    responses(
        (status = 200, description = "Semester activity timetable context", body = ApiResponse<ActivitySlotTimetableContextResponse>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable or activity read permission denied", body = ApiErrorResponse),
        (status = 500, description = "Activity timetable context could not be loaded", body = ApiErrorResponse)
    )
)]
pub async fn get_timetable_context(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ActivityTimetableContextQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let access = activity_access_policy::resolve_activity_list_access(&context.actor)?;
    let data =
        activity_service::get_timetable_context(&context.tenant.pool, query.semester_id, access)
            .await?;
    Ok(Json(ApiResponse::ok(data)).into_response())
}

#[utoipa::path(
    put,
    path = "/api/academic/activity-slots/{id}",
    operation_id = "updateActivitySlot",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity slot ID")),
    request_body = UpdateActivitySlotRequest,
    responses(
        (status = 200, description = "Activity slot updated", body = ApiResponse<ActivitySlot>),
        (status = 400, description = "Activity registration type is invalid", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School-wide activity management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity slot not found", body = ApiErrorResponse),
        (status = 500, description = "Activity slot could not be updated", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    delete,
    path = "/api/academic/activity-slots/{id}",
    operation_id = "deleteActivitySlot",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity slot ID")),
    responses(
        (status = 200, description = "Activity slot deleted", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School-wide activity management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity slot not found", body = ApiErrorResponse),
        (status = 500, description = "Activity slot could not be deleted", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    get,
    path = "/api/academic/activities",
    operation_id = "listActivityGroups",
    tag = "academic",
    params(ActivityGroupFilter),
    responses(
        (status = 200, description = "Activity groups visible to the caller", body = ApiResponse<Vec<ActivityGroup>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Activity read permission denied", body = ApiErrorResponse),
        (status = 500, description = "Activity groups could not be loaded", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    post,
    path = "/api/academic/activities",
    operation_id = "createActivityGroup",
    tag = "academic",
    request_body = CreateActivityGroupRequest,
    responses(
        (status = 200, description = "Activity group created", body = ApiResponse<ActivityGroup>),
        (status = 400, description = "Slot is closed, instructor is invalid, or classroom scope is invalid", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Activity management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Slot, instructor, or classroom not found", body = ApiErrorResponse),
        (status = 500, description = "Activity group could not be created", body = ApiErrorResponse)
    )
)]
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
        activity_service::CreateGroupOutcome::SlotClosed => Err(AppError::BadRequest(
            "ช่องกิจกรรมนี้ยังไม่เปิดให้ลงทะเบียน".to_string(),
        )),
        activity_service::CreateGroupOutcome::InstructorNotInSlot => Err(AppError::BadRequest(
            "ครูคนนี้ไม่ได้อยู่ในรายชื่อครูของช่องกิจกรรมนี้".to_string(),
        )),
    }
}

#[utoipa::path(
    put,
    path = "/api/academic/activities/{id}",
    operation_id = "updateActivityGroup",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity group ID")),
    request_body = UpdateActivityGroupRequest,
    responses(
        (status = 200, description = "Activity group updated", body = ApiResponse<ActivityGroup>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Activity management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Group, instructor, or classroom not found", body = ApiErrorResponse),
        (status = 500, description = "Activity group could not be updated", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    delete,
    path = "/api/academic/activities/{id}",
    operation_id = "deleteActivityGroup",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity group ID")),
    responses(
        (status = 200, description = "Activity group deleted", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Activity management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity group not found", body = ApiErrorResponse),
        (status = 500, description = "Activity group could not be deleted", body = ApiErrorResponse)
    )
)]
pub async fn delete_activity_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    activity_service::delete_group(&pool, &actor, id).await?;
    Ok(Json(ApiResponse::empty_with_message("ลบกลุ่มกิจกรรมแล้ว")).into_response())
}

// ============================================
// Members
// ============================================

#[utoipa::path(
    get,
    path = "/api/academic/activities/{id}/members",
    operation_id = "listActivityGroupMembers",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity group ID")),
    responses(
        (status = 200, description = "Activity group members", body = ApiResponse<Vec<ActivityGroupMember>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Activity group read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity group not found", body = ApiErrorResponse),
        (status = 500, description = "Activity members could not be loaded", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    post,
    path = "/api/academic/activities/{id}/members",
    operation_id = "addActivityGroupMembers",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity group ID")),
    request_body = AddMembersRequest,
    responses(
        (status = 200, description = "Members added", body = ApiResponse<ActivityInsertedCountData>),
        (status = 400, description = "Group capacity would be exceeded", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Member management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity group or user not found", body = ApiErrorResponse),
        (status = 500, description = "Activity members could not be added", body = ApiErrorResponse)
    )
)]
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
            Ok(Json(ApiResponse::ok(ActivityInsertedCountData { inserted: n })).into_response())
        }
        activity_service::AddMembersOutcome::OverCapacity(cap) => {
            Err(AppError::BadRequest(format!("จำนวนเกินที่รับได้ ({cap} คน)")))
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/academic/activities/my-enrollments",
    operation_id = "listMyActivityEnrollments",
    tag = "academic",
    responses(
        (status = 200, description = "Current user's activity group IDs", body = ApiResponse<Vec<Uuid>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 500, description = "Activity enrollments could not be loaded", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    post,
    path = "/api/academic/activities/{id}/enroll",
    operation_id = "selfEnrollActivityGroup",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity group ID")),
    responses(
        (status = 200, description = "Current student enrolled", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 400, description = "Self-enrollment is unavailable, full, or outside classroom scope", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 404, description = "Activity group not found", body = ApiErrorResponse),
        (status = 409, description = "Current student is already enrolled", body = ApiErrorResponse),
        (status = 500, description = "Self-enrollment could not be completed", body = ApiErrorResponse)
    )
)]
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
            Err(AppError::Conflict("ลงทะเบียนแล้วก่อนหน้านี้".to_string()))
        }
        activity_service::SelfEnrollOutcome::NotSelfRegistrationType => Err(AppError::BadRequest(
            "กลุ่มนี้ไม่เปิดให้ลงทะเบียนด้วยตนเอง".to_string(),
        )),
        activity_service::SelfEnrollOutcome::NotOpen => {
            Err(AppError::BadRequest("ยังไม่เปิดรับสมัคร".to_string()))
        }
        activity_service::SelfEnrollOutcome::Full => {
            Err(AppError::BadRequest("กลุ่มเต็มแล้ว".to_string()))
        }
        activity_service::SelfEnrollOutcome::ClassroomNotAllowed => {
            Err(AppError::BadRequest("ห้องเรียนของคุณไม่อยู่ในห้องที่รับ".to_string()))
        }
    }
}

#[utoipa::path(
    delete,
    path = "/api/academic/activities/{id}/enroll",
    operation_id = "selfUnenrollActivityGroup",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity group ID")),
    responses(
        (status = 200, description = "Current user's enrollment removed", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 404, description = "Current user's activity enrollment not found", body = ApiErrorResponse),
        (status = 500, description = "Self-unenrollment could not be completed", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    delete,
    path = "/api/academic/activities/{id}/members/{student_id}",
    operation_id = "removeActivityGroupMember",
    tag = "academic",
    params(
        ("id" = Uuid, Path, description = "Activity group ID"),
        ("student_id" = Uuid, Path, description = "Student user ID")
    ),
    responses(
        (status = 200, description = "Activity member removed", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Member management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity member not found", body = ApiErrorResponse),
        (status = 500, description = "Activity member could not be removed", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    put,
    path = "/api/academic/activities/members/{member_id}/result",
    operation_id = "updateActivityGroupMemberResult",
    tag = "academic",
    params(("member_id" = Uuid, Path, description = "Activity membership ID")),
    request_body = UpdateMemberResultRequest,
    responses(
        (status = 200, description = "Activity member result updated", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 400, description = "Result must be pass or fail", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Member management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity member not found", body = ApiErrorResponse),
        (status = 500, description = "Activity result could not be updated", body = ApiErrorResponse)
    )
)]
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

#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct InstructorRoleRequest {
    pub instructor_id: Uuid,
    #[schema(value_type = Option<ActivityGroupInstructorRole>)]
    pub role: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/academic/activities/{id}/instructors",
    operation_id = "listActivityGroupInstructors",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity group ID")),
    responses(
        (status = 200, description = "Activity group instructors", body = ApiResponse<Vec<InstructorInfo>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Activity group read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity group not found", body = ApiErrorResponse),
        (status = 500, description = "Activity instructors could not be loaded", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    post,
    path = "/api/academic/activities/{id}/instructors",
    operation_id = "addActivityGroupInstructor",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity group ID")),
    request_body = InstructorRoleRequest,
    responses(
        (status = 200, description = "Activity group instructor added", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 400, description = "Instructor role is invalid", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Activity group management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity group or instructor not found", body = ApiErrorResponse),
        (status = 500, description = "Activity instructor could not be added", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    delete,
    path = "/api/academic/activities/{id}/instructors/{instructor_id}",
    operation_id = "removeActivityGroupInstructor",
    tag = "academic",
    params(
        ("id" = Uuid, Path, description = "Activity group ID"),
        ("instructor_id" = Uuid, Path, description = "Instructor user ID")
    ),
    responses(
        (status = 200, description = "Activity group instructor removed", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Activity group management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity instructor assignment not found", body = ApiErrorResponse),
        (status = 500, description = "Activity instructor could not be removed", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    get,
    path = "/api/academic/activity-slots/{id}/instructors",
    operation_id = "listActivitySlotInstructors",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity slot ID")),
    responses(
        (status = 200, description = "Activity slot instructors", body = ApiResponse<Vec<SlotInstructorInfo>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Activity slot read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity slot not found", body = ApiErrorResponse),
        (status = 500, description = "Slot instructors could not be loaded", body = ApiErrorResponse)
    )
)]
pub async fn list_slot_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    activity_access_policy::can_read_activity_slot(&pool, &actor, slot_id).await?;
    let rows = activity_service::list_slot_instructors(&pool, slot_id).await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/activity-slots/{id}/instructors",
    operation_id = "addActivitySlotInstructor",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity slot ID")),
    request_body = AddSlotInstructorRequest,
    responses(
        (status = 200, description = "Activity slot instructor added", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School-wide activity management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity slot or instructor not found", body = ApiErrorResponse),
        (status = 500, description = "Slot instructor could not be added", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    post,
    path = "/api/academic/activity-slots/{id}/instructors/batch",
    operation_id = "addActivitySlotInstructorsBatch",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity slot ID")),
    request_body = AddSlotInstructorsBatchRequest,
    responses(
        (status = 200, description = "Activity slot instructors added", body = ApiResponse<ActivityAddedCountData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School-wide activity management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity slot or instructor not found", body = ApiErrorResponse),
        (status = 500, description = "Slot instructors could not be added", body = ApiErrorResponse)
    )
)]
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
    let is_empty = body.user_ids.is_empty();
    let added = activity_service::add_slot_instructors_batch(&pool, slot_id, body.user_ids).await?;
    let message = if is_empty {
        "ไม่มีครูที่จะเพิ่ม"
    } else {
        "เพิ่มครูแล้ว"
    };
    Ok(Json(ApiResponse::with_message(
        ActivityAddedCountData { added },
        message,
    ))
    .into_response())
}

#[utoipa::path(
    delete,
    path = "/api/academic/activity-slots/{id}/instructors/{user_id}",
    operation_id = "removeActivitySlotInstructor",
    tag = "academic",
    params(
        ("id" = Uuid, Path, description = "Activity slot ID"),
        ("user_id" = Uuid, Path, description = "Instructor user ID")
    ),
    responses(
        (status = 200, description = "Activity slot instructor removed", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School-wide activity management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity slot or instructor assignment not found", body = ApiErrorResponse),
        (status = 500, description = "Slot instructor could not be removed", body = ApiErrorResponse)
    )
)]
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

#[utoipa::path(
    delete,
    path = "/api/academic/activity-slots/{id}/timetable-entries",
    operation_id = "deleteActivitySlotTimetableEntries",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity slot ID")),
    responses(
        (status = 200, description = "Slot timetable entries deleted", body = ApiResponse<ActivityDeletedCountData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Course-plan management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity slot not found", body = ApiErrorResponse),
        (status = 500, description = "Slot timetable entries could not be deleted", body = ApiErrorResponse)
    )
)]
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
        ActivityDeletedCountData { deleted_count: n },
        "ลบรายการตารางสอนแล้ว",
    ))
    .into_response())
}

#[utoipa::path(
    delete,
    path = "/api/academic/activity-slots/{id}/groups",
    operation_id = "deleteAllActivitySlotGroups",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity slot ID")),
    responses(
        (status = 200, description = "All groups in the slot deleted", body = ApiResponse<ActivityDeletedCountData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School-wide activity management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity slot not found", body = ApiErrorResponse),
        (status = 500, description = "Slot groups could not be deleted", body = ApiErrorResponse)
    )
)]
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
        ActivityDeletedCountData { deleted_count: n },
        "ลบกิจกรรมทั้งหมดแล้ว",
    ))
    .into_response())
}

#[utoipa::path(
    delete,
    path = "/api/academic/activity-slots/{id}/instructors/all",
    operation_id = "removeAllActivitySlotInstructors",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity slot ID")),
    responses(
        (status = 200, description = "All slot instructors removed", body = ApiResponse<ActivityDeletedCountData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School-wide activity management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity slot not found", body = ApiErrorResponse),
        (status = 500, description = "Slot instructors could not be removed", body = ApiErrorResponse)
    )
)]
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
        ActivityDeletedCountData { deleted_count: n },
        "ลบครูทั้งหมดแล้ว",
    ))
    .into_response())
}

// ============================================
// Slot Classroom Assignments
// ============================================

#[utoipa::path(
    get,
    path = "/api/academic/activity-slots/{id}/classroom-assignments",
    operation_id = "listActivitySlotClassroomAssignments",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity slot ID")),
    responses(
        (status = 200, description = "Slot classroom assignments", body = ApiResponse<Vec<SlotClassroomAssignment>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Activity slot read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity slot not found", body = ApiErrorResponse),
        (status = 500, description = "Classroom assignments could not be loaded", body = ApiErrorResponse)
    )
)]
pub async fn list_slot_classroom_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    activity_access_policy::can_read_activity_slot(&pool, &actor, slot_id).await?;
    let rows = activity_service::list_slot_classroom_assignments(&pool, slot_id).await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/activity-slots/{id}/classroom-assignments",
    operation_id = "upsertActivitySlotClassroomAssignments",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity slot ID")),
    request_body = BatchUpsertSlotClassroomAssignmentsRequest,
    responses(
        (status = 200, description = "Slot classroom assignments saved", body = ApiResponse<ActivityProcessedCountData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School-wide activity management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity slot, classroom, or instructor not found", body = ApiErrorResponse),
        (status = 500, description = "Classroom assignments could not be saved", body = ApiErrorResponse)
    )
)]
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
        ActivityProcessedCountData { count: n },
        "บันทึกสำเร็จ",
    ))
    .into_response())
}

#[utoipa::path(
    delete,
    path = "/api/academic/activity-slots/{id}/classroom-assignments/all",
    operation_id = "deleteAllActivitySlotClassroomAssignments",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity slot ID")),
    responses(
        (status = 200, description = "All slot classroom assignments deleted", body = ApiResponse<ActivityDeletedCountData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School-wide activity management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity slot not found", body = ApiErrorResponse),
        (status = 500, description = "Classroom assignments could not be deleted", body = ApiErrorResponse)
    )
)]
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
        ActivityDeletedCountData { deleted_count: n },
        "ลบครูประจำห้องทั้งหมดแล้ว",
    ))
    .into_response())
}

#[utoipa::path(
    delete,
    path = "/api/academic/activity-slots/{id}/classroom-assignments/{assignment_id}",
    operation_id = "deleteActivitySlotClassroomAssignment",
    tag = "academic",
    params(
        ("id" = Uuid, Path, description = "Activity slot ID"),
        ("assignment_id" = Uuid, Path, description = "Classroom assignment ID")
    ),
    responses(
        (status = 200, description = "Slot classroom assignment deleted", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "School-wide activity management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity slot or classroom assignment not found", body = ApiErrorResponse),
        (status = 500, description = "Classroom assignment could not be deleted", body = ApiErrorResponse)
    )
)]
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
