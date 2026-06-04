use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use crate::db::school_mapping::get_school_database_url;
use crate::error::AppError;
use crate::middleware::permission::check_permission;
use crate::modules::academic::models::activity::*;
use crate::modules::academic::services::activity_service;
use crate::permissions::registry::codes;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain)
        .await
        .map_err(|_| AppError::NotFound("School not found".to_string()))?;
    state
        .pool_manager
        .get_pool(&db_url, &subdomain)
        .await
        .map_err(|_| AppError::InternalServerError("Database connection failed".to_string()))
}

// ============================================
// Activity Slots
// ============================================

pub async fn list_activity_slots(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<ActivitySlotFilter>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_READ_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let slots = activity_service::list_slots(&pool, filter).await?;
    Ok(Json(json!({ "success": true, "data": slots })).into_response())
}

pub async fn update_activity_slot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateActivitySlotRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let row = activity_service::update_slot(&pool, id, body).await?;
    Ok(Json(json!({ "success": true, "data": row })).into_response())
}

pub async fn delete_activity_slot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    activity_service::delete_slot(&pool, id).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "ลบช่องกิจกรรมแล้ว" })).into_response())
}

// ============================================
// Activity Groups
// ============================================

pub async fn list_activity_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<ActivityGroupFilter>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_READ_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let groups = activity_service::list_groups(&pool, filter).await?;
    Ok(Json(json!({ "success": true, "data": groups })).into_response())
}

pub async fn create_activity_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateActivityGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let has_manage_all = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    .is_ok();
    let has_manage_own = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MANAGE_OWN,
        &state.permission_cache,
    )
    .await
    .is_ok();
    if !has_manage_all && !has_manage_own {
        return Err(AppError::Forbidden("ไม่มีสิทธิ์".to_string()));
    }

    let outcome = activity_service::create_group(&pool, body, has_manage_all).await?;
    match outcome {
        activity_service::CreateGroupOutcome::Created(row) => {
            Ok(Json(json!({ "success": true, "data": row })).into_response())
        }
        activity_service::CreateGroupOutcome::SlotClosed => Ok(Json(
            json!({ "success": false, "error": "ช่องกิจกรรมนี้ยังไม่เปิดให้ลงทะเบียน" }),
        )
        .into_response()),
        activity_service::CreateGroupOutcome::InstructorNotInSlot => Ok(Json(
            json!({ "success": false, "error": "ครูคนนี้ไม่ได้อยู่ในรายชื่อครูของช่องกิจกรรมนี้" }),
        )
        .into_response()),
    }
}

pub async fn update_activity_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<UpdateActivityGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        if check_permission(
            &headers,
            &pool,
            codes::ACTIVITY_MANAGE_OWN,
            &state.permission_cache,
        )
        .await
        .is_err()
        {
            return Ok(r);
        }
    }
    let row = activity_service::update_group(&pool, id, body).await?;
    Ok(Json(json!({ "success": true, "data": row })).into_response())
}

pub async fn delete_activity_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    activity_service::delete_group(&pool, id).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "ลบกลุ่มกิจกรรมแล้ว" })).into_response())
}

// ============================================
// Members
// ============================================

pub async fn list_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_READ_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let members = activity_service::list_members(&pool, group_id).await?;
    Ok(Json(json!({ "success": true, "data": members })).into_response())
}

pub async fn add_members(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
    Json(body): Json<AddMembersRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MEMBERS_MANAGE,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    match activity_service::add_members(&pool, group_id, body.student_ids).await? {
        activity_service::AddMembersOutcome::Inserted(n) => {
            Ok(Json(json!({ "success": true, "data": { "inserted": n } })).into_response())
        }
        activity_service::AddMembersOutcome::OverCapacity(cap) => Ok(Json(
            json!({ "success": false, "error": format!("จำนวนเกินที่รับได้ ({cap} คน)") }),
        )
        .into_response()),
    }
}

pub async fn my_enrollments(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
        .await
        .map_err(AppError::AuthError)?;
    let ids = activity_service::my_enrollments(&pool, user_id).await?;
    Ok(Json(json!({ "success": true, "data": ids })).into_response())
}

pub async fn self_enroll(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
        .await
        .map_err(AppError::AuthError)?;

    match activity_service::self_enroll(&pool, group_id, user_id).await? {
        activity_service::SelfEnrollOutcome::Enrolled => Ok(Json(
            json!({ "success": true, "data": {}, "message": "ลงทะเบียนสำเร็จ" }),
        )
        .into_response()),
        activity_service::SelfEnrollOutcome::AlreadyEnrolled => {
            Ok(Json(json!({ "success": false, "error": "ลงทะเบียนแล้วก่อนหน้านี้" })).into_response())
        }
        activity_service::SelfEnrollOutcome::NotSelfRegistrationType => Ok(Json(
            json!({ "success": false, "error": "กลุ่มนี้ไม่เปิดให้ลงทะเบียนด้วยตนเอง" }),
        )
        .into_response()),
        activity_service::SelfEnrollOutcome::NotOpen => {
            Ok(Json(json!({ "success": false, "error": "ยังไม่เปิดรับสมัคร" })).into_response())
        }
        activity_service::SelfEnrollOutcome::Full => {
            Ok(Json(json!({ "success": false, "error": "กลุ่มเต็มแล้ว" })).into_response())
        }
        activity_service::SelfEnrollOutcome::ClassroomNotAllowed => Ok(Json(
            json!({ "success": false, "error": "ห้องเรียนของคุณไม่อยู่ในห้องที่รับ" }),
        )
        .into_response()),
    }
}

pub async fn self_unenroll(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
        .await
        .map_err(AppError::AuthError)?;
    activity_service::self_unenroll(&pool, group_id, user_id).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "ยกเลิกลงทะเบียนแล้ว" })).into_response())
}

pub async fn remove_member(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((group_id, student_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MEMBERS_MANAGE,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    activity_service::remove_member(&pool, group_id, student_id).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "ลบสมาชิกแล้ว" })).into_response())
}

pub async fn update_member_result(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(member_id): Path<Uuid>,
    Json(body): Json<UpdateMemberResultRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MEMBERS_MANAGE,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    activity_service::update_member_result(&pool, member_id, &body.result).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "บันทึกผลแล้ว" })).into_response())
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
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_READ_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let rows = activity_service::list_group_instructors(&pool, group_id).await?;
    Ok(Json(json!({ "success": true, "data": rows })).into_response())
}

pub async fn add_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(group_id): Path<Uuid>,
    Json(body): Json<InstructorRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        if check_permission(
            &headers,
            &pool,
            codes::ACTIVITY_MANAGE_OWN,
            &state.permission_cache,
        )
        .await
        .is_err()
        {
            return Ok(r);
        }
    }
    let role = body.role.unwrap_or_else(|| "assistant".to_string());
    activity_service::add_group_instructor(&pool, group_id, body.instructor_id, &role).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "เพิ่มครูแล้ว" })).into_response())
}

pub async fn remove_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((group_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        if check_permission(
            &headers,
            &pool,
            codes::ACTIVITY_MANAGE_OWN,
            &state.permission_cache,
        )
        .await
        .is_err()
        {
            return Ok(r);
        }
    }
    activity_service::remove_group_instructor(&pool, group_id, instructor_id).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "ลบครูแล้ว" })).into_response())
}

// ============================================
// Slot Instructors
// ============================================

pub async fn list_slot_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_READ_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let rows = activity_service::list_slot_instructors(&pool, slot_id).await?;
    Ok(Json(json!({ "success": true, "data": rows })).into_response())
}

pub async fn add_slot_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let user_id = body
        .get("user_id")
        .and_then(|v| v.as_str())
        .and_then(|s| Uuid::parse_str(s).ok())
        .ok_or_else(|| AppError::BadRequest("user_id required".to_string()))?;

    activity_service::add_slot_instructor(&pool, slot_id, user_id).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "เพิ่มครูแล้ว" })).into_response())
}

pub async fn add_slot_instructors_batch(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
    Json(body): Json<serde_json::Value>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let user_ids: Vec<Uuid> = body
        .get("user_ids")
        .and_then(|v| v.as_array())
        .ok_or_else(|| AppError::BadRequest("user_ids array required".to_string()))?
        .iter()
        .filter_map(|v| v.as_str().and_then(|s| Uuid::parse_str(s).ok()))
        .collect();

    if user_ids.is_empty() {
        return Ok(Json(
            json!({ "success": true, "data": { "added": 0 }, "message": "ไม่มีครูที่จะเพิ่ม" }),
        )
        .into_response());
    }

    let added = activity_service::add_slot_instructors_batch(&pool, slot_id, user_ids).await?;
    Ok(
        Json(json!({ "success": true, "data": { "added": added }, "message": "เพิ่มครูแล้ว" }))
            .into_response(),
    )
}

pub async fn remove_slot_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((slot_id, user_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    activity_service::remove_slot_instructor(&pool, slot_id, user_id).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "ลบครูแล้ว" })).into_response())
}

pub async fn delete_slot_timetable_entries(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let n = activity_service::delete_slot_timetable_entries(&pool, slot_id).await?;
    Ok(Json(json!({ "success": true, "data": { "deleted_count": n }, "message": "ลบรายการตารางสอนแล้ว" })).into_response())
}

pub async fn delete_all_slot_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let n = activity_service::delete_all_slot_groups(&pool, slot_id).await?;
    Ok(Json(
        json!({ "success": true, "data": { "deleted_count": n }, "message": "ลบกิจกรรมทั้งหมดแล้ว" }),
    )
    .into_response())
}

pub async fn remove_all_slot_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let n = activity_service::remove_all_slot_instructors(&pool, slot_id).await?;
    Ok(
        Json(json!({ "success": true, "data": { "deleted_count": n }, "message": "ลบครูทั้งหมดแล้ว" }))
            .into_response(),
    )
}

// ============================================
// Slot Classroom Assignments
// ============================================

pub async fn list_slot_classroom_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_READ_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let rows = activity_service::list_slot_classroom_assignments(&pool, slot_id).await?;
    Ok(Json(json!({ "success": true, "data": rows })).into_response())
}

pub async fn batch_upsert_slot_classroom_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
    Json(body): Json<BatchUpsertSlotClassroomAssignmentsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let n = activity_service::batch_upsert_slot_classroom_assignments(&pool, slot_id, body).await?;
    Ok(
        Json(json!({ "success": true, "data": { "count": n }, "message": "บันทึกสำเร็จ" }))
            .into_response(),
    )
}

pub async fn delete_all_slot_classroom_assignments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(slot_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let n = activity_service::delete_all_slot_classroom_assignments(&pool, slot_id).await?;
    Ok(Json(json!({ "success": true, "data": { "deleted_count": n }, "message": "ลบครูประจำห้องทั้งหมดแล้ว" })).into_response())
}

pub async fn delete_slot_classroom_assignment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((slot_id, assignment_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACTIVITY_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    activity_service::delete_slot_classroom_assignment(&pool, slot_id, assignment_id).await?;
    Ok(Json(json!({ "success": true, "data": {}, "message": "ลบสำเร็จ" })).into_response())
}
