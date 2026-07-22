use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::academic::services::academic_structure_service;
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

pub mod activity;
pub mod assessment;
pub mod course_planning;
pub mod exam_schedule;
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
use utoipa::ToSchema;
use uuid::Uuid;

#[utoipa::path(
    get,
    path = "/api/academic/structure",
    operation_id = "getAcademicStructure",
    tag = "academic",
    responses(
        (status = 200, description = "Academic structure", body = ApiResponse<crate::modules::academic::services::academic_structure_service::AcademicStructure>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic structure read permission denied", body = ApiErrorResponse),
        (status = 500, description = "Academic structure could not be loaded", body = ApiErrorResponse)
    )
)]
pub async fn list_academic_structure(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_STRUCTURE_READ_ALL)?;
    let structure = academic_structure_service::list_academic_structure(&pool).await?;

    Ok(Json(ApiResponse::ok(structure)))
}

#[utoipa::path(
    post,
    path = "/api/academic/years",
    operation_id = "createAcademicYear",
    tag = "academic",
    request_body = CreateAcademicYearRequest,
    responses(
        (status = 201, description = "Academic year created", body = ApiResponse<AcademicYear>),
        (status = 400, description = "Invalid academic year", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic structure management permission denied", body = ApiErrorResponse),
        (status = 409, description = "Academic year already exists", body = ApiErrorResponse),
        (status = 500, description = "Academic year could not be created", body = ApiErrorResponse)
    )
)]
pub async fn create_academic_year(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateAcademicYearRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_STRUCTURE_MANAGE_ALL)?;
    let year = academic_structure_service::create_academic_year(&pool, payload).await?;

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(year))))
}

#[utoipa::path(
    put,
    path = "/api/academic/years/{id}",
    operation_id = "updateAcademicYear",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Academic year ID")),
    request_body = UpdateAcademicYearRequest,
    responses(
        (status = 200, description = "Academic year updated", body = ApiResponse<AcademicYear>),
        (status = 400, description = "Invalid academic year", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic structure management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic year not found", body = ApiErrorResponse),
        (status = 409, description = "Academic year conflicts with an existing year", body = ApiErrorResponse),
        (status = 500, description = "Academic year could not be updated", body = ApiErrorResponse)
    )
)]
pub async fn update_academic_year(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateAcademicYearRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_STRUCTURE_MANAGE_ALL)?;
    let year = academic_structure_service::update_academic_year(&pool, id, payload).await?;

    Ok(Json(ApiResponse::ok(year)).into_response())
}

#[utoipa::path(
    put,
    path = "/api/academic/years/{id}/active",
    operation_id = "setActiveAcademicYear",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Academic year ID")),
    responses(
        (status = 200, description = "Active academic year updated", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic structure management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic year not found", body = ApiErrorResponse),
        (status = 500, description = "Active academic year could not be updated", body = ApiErrorResponse)
    )
)]
pub async fn toggle_active_year(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_STRUCTURE_MANAGE_ALL)?;
    academic_structure_service::toggle_active_year(&pool, id).await?;

    Ok(Json(ApiResponse::empty_with_message("Updated active year")))
}

#[utoipa::path(
    post,
    path = "/api/academic/semesters",
    operation_id = "createSemester",
    tag = "academic",
    request_body = CreateSemesterRequest,
    responses(
        (status = 201, description = "Semester created", body = ApiResponse<Semester>),
        (status = 400, description = "Invalid semester", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic structure management permission denied", body = ApiErrorResponse),
        (status = 409, description = "Semester already exists", body = ApiErrorResponse),
        (status = 500, description = "Semester could not be created", body = ApiErrorResponse)
    )
)]
pub async fn create_semester(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateSemesterRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_STRUCTURE_MANAGE_ALL)?;
    let semester = academic_structure_service::create_semester(&pool, payload).await?;

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(semester))))
}

#[utoipa::path(
    put,
    path = "/api/academic/semesters/{id}",
    operation_id = "updateSemester",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Semester ID")),
    request_body = UpdateSemesterRequest,
    responses(
        (status = 200, description = "Semester updated", body = ApiResponse<Semester>),
        (status = 400, description = "Invalid semester", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic structure management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Semester not found", body = ApiErrorResponse),
        (status = 409, description = "Semester conflicts with an existing semester", body = ApiErrorResponse),
        (status = 500, description = "Semester could not be updated", body = ApiErrorResponse)
    )
)]
pub async fn update_semester(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateSemesterRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_STRUCTURE_MANAGE_ALL)?;
    let semester = academic_structure_service::update_semester(&pool, id, payload).await?;

    Ok(Json(ApiResponse::ok(semester)))
}

#[utoipa::path(
    delete,
    path = "/api/academic/semesters/{id}",
    operation_id = "deleteSemester",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Semester ID")),
    responses(
        (status = 200, description = "Semester deleted", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 400, description = "Semester is in use", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic structure management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Semester not found", body = ApiErrorResponse),
        (status = 500, description = "Semester could not be deleted", body = ApiErrorResponse)
    )
)]
pub async fn delete_semester(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_STRUCTURE_MANAGE_ALL)?;
    academic_structure_service::delete_semester(&pool, id).await?;

    Ok(Json(ApiResponse::empty_with_message("Semester deleted")))
}

#[utoipa::path(
    get,
    path = "/api/academic/classrooms",
    operation_id = "listClassrooms",
    tag = "academic",
    params(("year_id" = Option<Uuid>, Query, description = "Filter by academic year")),
    responses(
        (status = 200, description = "Classrooms", body = ApiResponse<Vec<Classroom>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Classroom read permission denied", body = ApiErrorResponse),
        (status = 500, description = "Classrooms could not be loaded", body = ApiErrorResponse)
    )
)]
pub async fn list_classrooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<ClassroomQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_CLASSROOM_READ_ALL)?;
    let classrooms = academic_structure_service::list_classrooms(&pool, filter).await?;

    Ok(Json(ApiResponse::ok(classrooms)))
}

#[utoipa::path(
    post,
    path = "/api/academic/classrooms",
    operation_id = "createClassroom",
    tag = "academic",
    request_body = CreateClassroomRequest,
    responses(
        (status = 201, description = "Classroom created", body = ApiResponse<Classroom>),
        (status = 400, description = "Invalid classroom", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Classroom creation permission denied", body = ApiErrorResponse),
        (status = 409, description = "Classroom already exists", body = ApiErrorResponse),
        (status = 500, description = "Classroom could not be created", body = ApiErrorResponse)
    )
)]
pub async fn create_classroom(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateClassroomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_CLASSROOM_CREATE_ALL)?;
    let classroom = academic_structure_service::create_classroom(&pool, payload).await?;

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(classroom))))
}

#[utoipa::path(
    put,
    path = "/api/academic/classrooms/{id}",
    operation_id = "updateClassroom",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Classroom ID")),
    request_body = UpdateClassroomRequest,
    responses(
        (status = 200, description = "Classroom updated", body = ApiResponse<Classroom>),
        (status = 400, description = "Invalid classroom", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Classroom update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Classroom not found", body = ApiErrorResponse),
        (status = 409, description = "Classroom conflicts with an existing classroom", body = ApiErrorResponse),
        (status = 500, description = "Classroom could not be updated", body = ApiErrorResponse)
    )
)]
pub async fn update_classroom(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateClassroomRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_CLASSROOM_UPDATE_ALL)?;
    let classroom = academic_structure_service::update_classroom(&pool, id, payload).await?;

    Ok(Json(ApiResponse::ok(classroom)))
}

#[utoipa::path(
    post,
    path = "/api/academic/levels",
    operation_id = "createGradeLevel",
    tag = "academic",
    request_body = CreateGradeLevelRequest,
    responses(
        (status = 201, description = "Grade level created", body = ApiResponse<GradeLevelResponse>),
        (status = 400, description = "Invalid or duplicate grade level", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic structure management permission denied", body = ApiErrorResponse),
        (status = 500, description = "Grade level could not be created", body = ApiErrorResponse)
    )
)]
pub async fn create_grade_level(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateGradeLevelRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_STRUCTURE_MANAGE_ALL)?;
    let level = academic_structure_service::create_grade_level(&pool, payload).await?;

    Ok((StatusCode::CREATED, Json(ApiResponse::ok(level))))
}

#[utoipa::path(
    delete,
    path = "/api/academic/levels/{id}",
    operation_id = "deleteGradeLevel",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Grade level ID")),
    responses(
        (status = 200, description = "Grade level deleted", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 400, description = "Grade level is in use", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic structure management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Grade level not found", body = ApiErrorResponse),
        (status = 500, description = "Grade level could not be deleted", body = ApiErrorResponse)
    )
)]
pub async fn delete_grade_level(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_STRUCTURE_MANAGE_ALL)?;
    academic_structure_service::delete_grade_level(&pool, id).await?;

    Ok(Json(ApiResponse::empty_with_message("Grade level deleted")))
}

#[utoipa::path(
    post,
    path = "/api/academic/enrollments",
    operation_id = "enrollStudents",
    tag = "academic",
    request_body = EnrollStudentRequest,
    responses(
        (status = 200, description = "Students enrolled", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 400, description = "Invalid enrollment", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Enrollment update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Classroom not found", body = ApiErrorResponse),
        (status = 409, description = "Enrollment conflicts with existing data", body = ApiErrorResponse),
        (status = 500, description = "Students could not be enrolled", body = ApiErrorResponse)
    )
)]
pub async fn enroll_students(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<EnrollStudentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_ENROLLMENT_UPDATE_ALL)?;
    let enrolled_count = academic_structure_service::enroll_students(&pool, payload).await?;

    Ok(Json(ApiResponse::empty_with_message(format!(
        "Enrolled {} students successfully",
        enrolled_count
    ))))
}

#[utoipa::path(
    get,
    path = "/api/academic/enrollments/class/{id}",
    operation_id = "listClassEnrollments",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Classroom ID")),
    responses(
        (status = 200, description = "Class enrollments", body = ApiResponse<Vec<StudentEnrollment>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Enrollment read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Classroom not found", body = ApiErrorResponse),
        (status = 500, description = "Class enrollments could not be loaded", body = ApiErrorResponse)
    )
)]
pub async fn get_class_enrollments(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(class_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_ENROLLMENT_READ_ALL)?;
    let enrollments = academic_structure_service::get_class_enrollments(&pool, class_id).await?;

    Ok(Json(ApiResponse::ok(enrollments)))
}

#[utoipa::path(
    delete,
    path = "/api/academic/enrollments/{id}",
    operation_id = "removeEnrollment",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Enrollment ID")),
    responses(
        (status = 200, description = "Enrollment removed", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Enrollment update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Enrollment not found", body = ApiErrorResponse),
        (status = 500, description = "Enrollment could not be removed", body = ApiErrorResponse)
    )
)]
pub async fn remove_enrollment(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_ENROLLMENT_UPDATE_ALL)?;
    academic_structure_service::remove_enrollment(&pool, id).await?;

    Ok(Json(ApiResponse::empty_with_message("Enrollment removed")))
}

#[derive(serde::Deserialize, ToSchema)]
pub struct UpdateEnrollmentNumberRequest {
    pub class_number: Option<i32>,
}

#[utoipa::path(
    put,
    path = "/api/academic/enrollments/{id}/number",
    operation_id = "updateEnrollmentNumber",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Enrollment ID")),
    request_body = UpdateEnrollmentNumberRequest,
    responses(
        (status = 200, description = "Enrollment number updated", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 400, description = "Invalid enrollment number", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Enrollment update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Enrollment not found", body = ApiErrorResponse),
        (status = 500, description = "Enrollment number could not be updated", body = ApiErrorResponse)
    )
)]
pub async fn update_enrollment_number(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateEnrollmentNumberRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_ENROLLMENT_UPDATE_ALL)?;
    academic_structure_service::update_enrollment_number(&pool, id, payload.class_number).await?;

    Ok(Json(ApiResponse::empty_with_message(
        "Class number updated",
    )))
}

#[derive(serde::Deserialize, ToSchema)]
pub struct AutoAssignClassNumbersRequest {
    pub sort_by: String,
}

#[utoipa::path(
    post,
    path = "/api/academic/enrollments/class/{id}/auto-number",
    operation_id = "autoAssignClassNumbers",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Classroom ID")),
    request_body = AutoAssignClassNumbersRequest,
    responses(
        (status = 200, description = "Class numbers assigned", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 400, description = "Invalid numbering method", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Enrollment update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Classroom not found", body = ApiErrorResponse),
        (status = 500, description = "Class numbers could not be assigned", body = ApiErrorResponse)
    )
)]
pub async fn auto_assign_class_numbers(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(class_id): Path<Uuid>,
    Json(payload): Json<AutoAssignClassNumbersRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_ENROLLMENT_UPDATE_ALL)?;
    let updated_count =
        academic_structure_service::auto_assign_class_numbers(&pool, class_id, &payload.sort_by)
            .await?;

    Ok(Json(ApiResponse::empty_with_message(format!(
        "เรียงเลขที่สำหรับ {} คนเรียบร้อยแล้ว",
        updated_count
    ))))
}

#[utoipa::path(
    get,
    path = "/api/academic/years/{id}/levels",
    operation_id = "getAcademicYearLevels",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Academic year ID")),
    responses(
        (status = 200, description = "Grade level IDs enabled for the academic year", body = ApiResponse<Vec<String>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic structure read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic year not found", body = ApiErrorResponse),
        (status = 500, description = "Academic year levels could not be loaded", body = ApiErrorResponse)
    )
)]
pub async fn get_year_levels(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(year_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_STRUCTURE_READ_ALL)?;
    let level_ids = academic_structure_service::get_year_levels(&pool, year_id).await?;
    let level_ids = level_ids
        .into_iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>();

    Ok(Json(ApiResponse::ok(level_ids)))
}

#[utoipa::path(
    put,
    path = "/api/academic/years/{id}/levels",
    operation_id = "updateAcademicYearLevels",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Academic year ID")),
    request_body = UpdateYearLevelsRequest,
    responses(
        (status = 200, description = "Academic year levels updated", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 400, description = "Invalid grade level selection", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic structure management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic year not found", body = ApiErrorResponse),
        (status = 500, description = "Academic year levels could not be updated", body = ApiErrorResponse)
    )
)]
pub async fn update_year_levels(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(year_id): Path<Uuid>,
    Json(payload): Json<UpdateYearLevelsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_STRUCTURE_MANAGE_ALL)?;
    academic_structure_service::update_year_levels(&pool, year_id, payload.grade_level_ids).await?;

    Ok(Json(ApiResponse::empty_with_message("Year levels updated")))
}
