use axum::{
    extract::{rejection::JsonRejection, Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::academic::models::scheduling_config::{
    CcPreferredRoomView, ClassroomCourseConstraintView, ListClassroomCourseConstraintsQuery,
    SaveSchedulingConfigurationRequest, SchedulerSettingsView, SchedulingConfigurationSaveResult,
    SchedulingRoomView, SubjectConstraintView,
};
use crate::modules::academic::services::scheduling_config_service;
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

fn parse_json_payload<T>(payload_result: Result<Json<T>, JsonRejection>) -> Result<T, AppError> {
    let Json(payload) =
        payload_result.map_err(|rejection| AppError::BadRequest(rejection.body_text()))?;
    Ok(payload)
}

#[utoipa::path(
    get,
    path = "/api/academic/scheduling/instructors",
    operation_id = "listSchedulingInstructorConstraints",
    tag = "academic",
    responses(
        (status = 200, description = "Scheduling instructor constraints", body = ApiResponse<Vec<crate::modules::academic::models::scheduling_config::InstructorConstraintView>>),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan read permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 404, description = "Active academic year not found", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Instructor constraints could not be loaded", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn list_instructor_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let rows = scheduling_config_service::list_instructor_constraints(&context.tenant.pool).await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/scheduling/subjects",
    operation_id = "listSchedulingSubjectConstraints",
    tag = "academic",
    responses(
        (status = 200, description = "Scheduling subject constraints", body = ApiResponse<Vec<SubjectConstraintView>>),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan read permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Subject constraints could not be loaded", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn list_subject_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let rows = scheduling_config_service::list_subject_constraints(&context.tenant.pool).await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/scheduling/settings",
    operation_id = "getSchedulingSettings",
    tag = "academic",
    responses(
        (status = 200, description = "Scheduling settings", body = ApiResponse<SchedulerSettingsView>),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan read permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Scheduling settings could not be loaded", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn get_scheduler_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let settings = scheduling_config_service::get_scheduler_settings(&context.tenant.pool).await?;
    Ok(Json(ApiResponse::ok(settings)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/scheduling/classroom-courses",
    operation_id = "listSchedulingClassroomCourseConstraints",
    tag = "academic",
    params(ListClassroomCourseConstraintsQuery),
    responses(
        (status = 200, description = "Scheduling classroom-course constraints", body = ApiResponse<Vec<ClassroomCourseConstraintView>>),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan read permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 404, description = "Active academic year not found", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Classroom-course constraints could not be loaded", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn list_classroom_course_constraints(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListClassroomCourseConstraintsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let rows = scheduling_config_service::list_classroom_course_constraints(
        &context.tenant.pool,
        query.instructor_id,
    )
    .await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/scheduling/classroom-courses/{id}/rooms",
    operation_id = "listSchedulingClassroomCoursePreferredRooms",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Classroom-course ID")),
    responses(
        (status = 200, description = "Preferred rooms for a classroom course", body = ApiResponse<Vec<CcPreferredRoomView>>),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan read permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 404, description = "Classroom course not found", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Preferred rooms could not be loaded", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn list_cc_preferred_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(classroom_course_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let rows = scheduling_config_service::list_cc_preferred_rooms(
        &context.tenant.pool,
        classroom_course_id,
    )
    .await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/scheduling/rooms",
    operation_id = "listSchedulingRooms",
    tag = "academic",
    responses(
        (status = 200, description = "Active scheduling rooms", body = ApiResponse<Vec<SchedulingRoomView>>),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan read permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Rooms could not be loaded", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn list_all_rooms(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let rows = scheduling_config_service::list_all_rooms(&context.tenant.pool).await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

#[utoipa::path(
    put,
    path = "/api/academic/scheduling/configuration",
    operation_id = "saveSchedulingConfiguration",
    tag = "academic",
    request_body = SaveSchedulingConfigurationRequest,
    responses(
        (status = 200, description = "Scheduling configuration saved atomically", body = ApiResponse<SchedulingConfigurationSaveResult>),
        (status = 400, description = "Malformed or invalid scheduling configuration", body = crate::api_response::ApiErrorResponse),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan management permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 404, description = "Active academic year or referenced target not found", body = crate::api_response::ApiErrorResponse),
        (status = 409, description = "Scheduling configuration changed concurrently", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Scheduling configuration could not be saved", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn save_scheduling_configuration(
    State(state): State<AppState>,
    headers: HeaderMap,
    payload_result: Result<Json<SaveSchedulingConfigurationRequest>, JsonRejection>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    let payload = parse_json_payload(payload_result)?;
    let result =
        scheduling_config_service::save_scheduling_configuration(&context.tenant.pool, payload)
            .await?;
    Ok(Json(ApiResponse::ok(result)).into_response())
}
