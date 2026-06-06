use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use super::models::{
    CreateStudentRequest, ListStudentsQuery, UpdateOwnProfileRequest, UpdateStudentRequest,
};
use super::services as student_service;
use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

/// GET /api/student/profile - นักเรียนดูข้อมูลตนเอง
pub async fn get_own_profile(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let student = student_service::get_own_profile(&pool, actor.user_id).await?;

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
    actor.require_permission(codes::STUDENT_READ_ALL)?;

    let students = student_service::list_students(&pool, filter).await?;

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
    actor.require_permission(codes::STUDENT_CREATE)?;

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
    actor.require_permission(codes::STUDENT_READ_ALL)?;

    let student = student_service::get_student(&pool, student_id).await?;

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
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::STUDENT_DELETE)?;

    student_service::delete_student(&pool, student_id).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("ลบนักเรียนสำเร็จ")),
    ))
}
