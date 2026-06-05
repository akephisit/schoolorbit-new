use crate::error::AppError;
use crate::modules::academic::services::academic_structure_service;
use crate::utils::request_context::tenant_pool;
use crate::AppState;

pub mod activity;
pub mod course_planning;
pub mod scheduling;
pub mod scheduling_config;
pub mod study_plans;
pub mod subjects;
pub mod timetable;
pub mod timetable_templates;

use super::models::*;
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

pub async fn list_academic_structure(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let structure = academic_structure_service::list_academic_structure(&pool).await?;

    Ok(Json(json!({ "success": true, "data": structure })))
}

pub async fn create_academic_year(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAcademicYearRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let year = academic_structure_service::create_academic_year(&pool, payload).await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": year })),
    ))
}

pub async fn update_academic_year(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAcademicYearRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let year = academic_structure_service::update_academic_year(&pool, id, payload).await?;

    Ok(Json(json!({ "success": true, "data": year })).into_response())
}

pub async fn toggle_active_year(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    academic_structure_service::toggle_active_year(&pool, id).await?;

    Ok(Json(json!({
        "success": true,
        "data": {},
        "message": "Updated active year"
    })))
}

pub async fn create_semester(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateSemesterRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let semester = academic_structure_service::create_semester(&pool, payload).await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": semester })),
    ))
}

pub async fn update_semester(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSemesterRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let semester = academic_structure_service::update_semester(&pool, id, payload).await?;

    Ok(Json(json!({ "success": true, "data": semester })))
}

pub async fn delete_semester(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    academic_structure_service::delete_semester(&pool, id).await?;

    Ok(Json(json!({
        "success": true,
        "data": {},
        "message": "Semester deleted"
    })))
}

pub async fn list_classrooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<ClassroomQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let classrooms = academic_structure_service::list_classrooms(&pool, filter).await?;

    Ok(Json(json!({ "success": true, "data": classrooms })))
}

pub async fn create_classroom(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateClassroomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let classroom = academic_structure_service::create_classroom(&pool, payload).await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": classroom })),
    ))
}

pub async fn update_classroom(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateClassroomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let classroom = academic_structure_service::update_classroom(&pool, id, payload).await?;

    Ok(Json(json!({ "success": true, "data": classroom })))
}

pub async fn create_grade_level(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateGradeLevelRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let level = academic_structure_service::create_grade_level(&pool, payload).await?;

    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": level })),
    ))
}

pub async fn delete_grade_level(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    academic_structure_service::delete_grade_level(&pool, id).await?;

    Ok(Json(json!({
        "success": true,
        "data": {},
        "message": "Grade level deleted"
    })))
}

pub async fn enroll_students(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<EnrollStudentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let enrolled_count = academic_structure_service::enroll_students(&pool, payload).await?;

    Ok(Json(json!({
        "success": true,
        "data": {},
        "message": format!("Enrolled {} students successfully", enrolled_count)
    })))
}

pub async fn get_class_enrollments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(class_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let enrollments = academic_structure_service::get_class_enrollments(&pool, class_id).await?;

    Ok(Json(json!({ "success": true, "data": enrollments })))
}

pub async fn remove_enrollment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    academic_structure_service::remove_enrollment(&pool, id).await?;

    Ok(Json(json!({
        "success": true,
        "data": {},
        "message": "Enrollment removed"
    })))
}

#[derive(serde::Deserialize)]
pub struct UpdateEnrollmentNumberRequest {
    pub class_number: Option<i32>,
}

pub async fn update_enrollment_number(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateEnrollmentNumberRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    academic_structure_service::update_enrollment_number(&pool, id, payload.class_number).await?;

    Ok(Json(json!({
        "success": true,
        "data": {},
        "message": "Class number updated"
    })))
}

#[derive(serde::Deserialize)]
pub struct AutoAssignClassNumbersRequest {
    pub sort_by: String,
}

pub async fn auto_assign_class_numbers(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(class_id): Path<Uuid>,
    Json(payload): Json<AutoAssignClassNumbersRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let updated_count =
        academic_structure_service::auto_assign_class_numbers(&pool, class_id, &payload.sort_by)
            .await?;

    Ok(Json(json!({
        "success": true,
        "data": {},
        "message": format!("เรียงเลขที่สำหรับ {} คนเรียบร้อยแล้ว", updated_count)
    })))
}

pub async fn get_year_levels(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(year_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let level_ids = academic_structure_service::get_year_levels(&pool, year_id).await?;

    Ok(Json(json!({ "success": true, "data": level_ids })))
}

pub async fn update_year_levels(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(year_id): Path<Uuid>,
    Json(payload): Json<UpdateYearLevelsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    academic_structure_service::update_year_levels(&pool, year_id, payload.grade_level_ids).await?;

    Ok(Json(json!({
        "success": true,
        "data": {},
        "message": "Year levels updated"
    })))
}
