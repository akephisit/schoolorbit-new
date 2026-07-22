use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use super::models::{
    CreateStudentRequest, ListStudentsQuery, StudentProfile, UpdateOwnProfileRequest,
    UpdateStudentRequest,
};
use super::services as student_service;
use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::permissions::registry::codes;
use crate::policies::student_access_policy;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

/// GET /api/student/profile - นักเรียนดูข้อมูลตนเอง
#[utoipa::path(
    get,
    path = "/api/student/profile",
    operation_id = "getStudentProfile",
    tag = "student",
    responses(
        (status = 200, description = "Current student's scoped profile", body = ApiResponse<StudentProfile>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student profile access denied", body = ApiErrorResponse),
        (status = 404, description = "Student profile not found", body = ApiErrorResponse)
    )
)]
pub async fn get_own_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    student_access_policy::can_read_student_profile(&pool, &actor, actor.user_id).await?;
    let include_pii =
        student_access_policy::can_read_student_pii(&pool, &actor, actor.user_id).await?;
    let student = student_service::get_own_profile(&pool, actor.user_id, include_pii).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(student))))
}

/// PUT /api/student/profile - นักเรียนแก้ไขข้อมูลตนเอง (จำกัดฟิลด์)
pub async fn update_own_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateOwnProfileRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    student_service::update_own_profile(&pool, actor.user_id, payload).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("อัพเดตข้อมูลสำเร็จ")),
    ))
}

/// GET /api/students - รายชื่อนักเรียนทั้งหมด
pub async fn list_students(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<ListStudentsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let access = student_access_policy::resolve_student_list_access(&actor)?;

    let students = student_service::list_students(&pool, filter, access).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(students))))
}

/// POST /api/students - เพิ่มนักเรียนใหม่
pub async fn create_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateStudentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::STUDENT_CREATE_ALL)?;

    let student = student_service::create_student(&pool, payload).await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::with_message(student, "เพิ่มนักเรียนสำเร็จ")),
    ))
}

/// GET /api/students/:id - ดูข้อมูลนักเรียน
pub async fn get_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    student_access_policy::can_read_student_profile(&pool, &actor, student_id).await?;
    let include_pii =
        student_access_policy::can_read_student_pii(&pool, &actor, student_id).await?;

    let student = student_service::get_student(&pool, student_id, include_pii).await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(student))))
}

/// PUT /api/students/:id - แก้ไขข้อมูลนักเรียน
pub async fn update_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
    Json(payload): Json<UpdateStudentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::STUDENT_UPDATE_ALL)?;

    student_service::update_student(&pool, student_id, payload).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("อัพเดตข้อมูลนักเรียนสำเร็จ")),
    ))
}

/// DELETE /api/students/:id - ลบนักเรียน (soft delete)
pub async fn delete_student(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(student_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let tenant = context.tenant.subdomain.clone();
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::STUDENT_DELETE_ALL)?;

    student_service::delete_student(&pool, student_id).await?;
    state.permission_cache.invalidate_user(&tenant, student_id);
    state.notify_permission_changed(&tenant, student_id);

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("ลบนักเรียนสำเร็จ")),
    ))
}
