use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::academic::models::course_planning::{
    AddCourseInstructorRequest, AssignCoursesRequest, BatchListCourseInstructorsQuery,
    BatchListCourseInstructorsRequest, ClassroomActivityQuery, PlanQuery,
    UpdateCourseInstructorRoleRequest, UpdateCourseRequest,
};
use crate::modules::academic::services::course_planning_service;
use crate::modules::academic::websockets::TimetableEvent;
use crate::permissions::registry::codes;
use crate::utils::request_context::{actor_tenant_context, ActorTenantContext};
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;

#[derive(Debug, serde::Serialize, utoipa::ToSchema)]
pub struct CourseAssignedCountData {
    pub assigned: i64,
}

fn parse_course_ids(value: &str) -> Result<Vec<Uuid>, AppError> {
    let mut seen = std::collections::HashSet::new();
    value
        .split(',')
        .filter(|part| !part.trim().is_empty())
        .map(|part| {
            part.trim().parse::<Uuid>().map_err(|_| {
                AppError::BadRequest("course_ids must contain valid UUIDs".to_string())
            })
        })
        .filter_map(|result| match result {
            Ok(id) if seen.insert(id) => Some(Ok(id)),
            Ok(_) => None,
            Err(error) => Some(Err(error)),
        })
        .collect()
}

async fn broadcast_course_refresh(
    state: &AppState,
    headers: &HeaderMap,
    context: &ActorTenantContext,
    course_id: Uuid,
) {
    if let Some(sem_id) =
        course_planning_service::get_course_semester_id(&context.tenant.pool, course_id).await
    {
        let subdomain =
            extract_subdomain_from_request(headers).unwrap_or_else(|_| "default".to_string());
        state.websocket_manager.broadcast_mutation(
            subdomain,
            sem_id,
            TimetableEvent::CourseTeamChanged {
                user_id: context.actor.user_id,
                course_id,
            },
        );
    }
}

#[utoipa::path(
    get,
    path = "/api/academic/planning/courses",
    operation_id = "listClassroomCourses",
    tag = "academic",
    params(PlanQuery),
    responses(
        (status = 200, description = "Classroom courses", body = ApiResponse<Vec<crate::modules::academic::models::course_planning::ClassroomCourse>>),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan read permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Classroom courses could not be loaded", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn list_classroom_courses(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<PlanQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let courses = course_planning_service::list_classroom_courses(&pool, &query).await?;
    Ok(Json(ApiResponse::ok(courses)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/planning/courses",
    operation_id = "assignCourses",
    tag = "academic",
    request_body = AssignCoursesRequest,
    responses(
        (status = 200, description = "Courses assigned", body = ApiResponse<CourseAssignedCountData>),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan management permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 404, description = "Classroom, semester, or subject not found", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Courses could not be assigned", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn assign_courses(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<AssignCoursesRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    let added = course_planning_service::assign_courses(&pool, payload).await?;
    Ok(Json(ApiResponse::with_message(
        CourseAssignedCountData { assigned: added },
        format!("Assigned {added} courses"),
    ))
    .into_response())
}

#[utoipa::path(
    delete,
    path = "/api/academic/planning/courses/{id}",
    operation_id = "removeClassroomCourse",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Classroom course ID")),
    responses(
        (status = 200, description = "Classroom course removed", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan management permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 404, description = "Classroom course not found", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Classroom course could not be removed", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn remove_course(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    course_planning_service::remove_course(&pool, id).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

#[utoipa::path(
    put,
    path = "/api/academic/planning/courses/{id}",
    operation_id = "updateClassroomCourse",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Classroom course ID")),
    request_body = UpdateCourseRequest,
    responses(
        (status = 200, description = "Classroom course updated", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan management permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 404, description = "Classroom course or instructor not found", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Classroom course could not be updated", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn update_course(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateCourseRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    course_planning_service::update_course(&pool, id, payload).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/planning/courses/instructors/batch",
    operation_id = "batchListCourseInstructors",
    tag = "academic",
    request_body = BatchListCourseInstructorsRequest,
    responses(
        (status = 200, description = "Course instructors grouped by course", body = ApiResponse<HashMap<String, Vec<crate::modules::academic::models::course_planning::CourseInstructor>>>),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan read permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Course instructors could not be loaded", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn batch_list_course_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<BatchListCourseInstructorsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let grouped =
        course_planning_service::batch_list_course_instructors(&pool, &payload.course_ids).await?;
    Ok(Json(ApiResponse::ok(grouped)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/planning/courses/instructors",
    operation_id = "batchListCourseInstructorsFromQuery",
    tag = "academic",
    params(BatchListCourseInstructorsQuery),
    responses(
        (status = 200, description = "Course instructors grouped by course", body = ApiResponse<HashMap<String, Vec<crate::modules::academic::models::course_planning::CourseInstructor>>>),
        (status = 400, description = "course_ids contains a malformed UUID", body = crate::api_response::ApiErrorResponse),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan read permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Course instructors could not be loaded", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn batch_list_course_instructors_from_query(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<BatchListCourseInstructorsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let ids = parse_course_ids(&query.course_ids)?;
    let grouped = course_planning_service::batch_list_course_instructors(&pool, &ids).await?;
    Ok(Json(ApiResponse::ok(grouped)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/planning/courses/{id}/instructors",
    operation_id = "listCourseInstructors",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Classroom course ID")),
    responses(
        (status = 200, description = "Course instructors", body = ApiResponse<Vec<crate::modules::academic::models::course_planning::CourseInstructor>>),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan read permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 404, description = "Classroom course not found", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Course instructors could not be loaded", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn list_course_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(course_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let rows = course_planning_service::list_course_instructors(&pool, course_id).await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/planning/courses/{id}/instructors",
    operation_id = "addCourseInstructor",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Classroom course ID")),
    request_body = AddCourseInstructorRequest,
    responses(
        (status = 200, description = "Course instructor added", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 400, description = "Instructor role is invalid", body = crate::api_response::ApiErrorResponse),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan management permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 404, description = "Classroom course or instructor not found", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Course instructor could not be added", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn add_course_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(course_id): Path<Uuid>,
    Json(body): Json<AddCourseInstructorRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    let role = body.role.unwrap_or_else(|| "secondary".to_string());
    course_planning_service::add_course_instructor(
        &context.tenant.pool,
        course_id,
        body.instructor_id,
        &role,
    )
    .await?;
    broadcast_course_refresh(&state, &headers, &context, course_id).await;
    Ok(Json(ApiResponse::empty()).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/academic/planning/courses/{id}/instructors/{uid}",
    operation_id = "removeCourseInstructor",
    tag = "academic",
    params(
        ("id" = Uuid, Path, description = "Classroom course ID"),
        ("uid" = Uuid, Path, description = "Instructor user ID")
    ),
    responses(
        (status = 200, description = "Course instructor removed", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan management permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 404, description = "Classroom course or instructor assignment not found", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Course instructor could not be removed", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn remove_course_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((course_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    course_planning_service::remove_course_instructor(
        &context.tenant.pool,
        course_id,
        instructor_id,
    )
    .await?;
    broadcast_course_refresh(&state, &headers, &context, course_id).await;
    Ok(Json(ApiResponse::empty()).into_response())
}

#[utoipa::path(
    put,
    path = "/api/academic/planning/courses/{id}/instructors/{uid}",
    operation_id = "updateCourseInstructorRole",
    tag = "academic",
    params(
        ("id" = Uuid, Path, description = "Classroom course ID"),
        ("uid" = Uuid, Path, description = "Instructor user ID")
    ),
    request_body = UpdateCourseInstructorRoleRequest,
    responses(
        (status = 200, description = "Course instructor role updated", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 400, description = "Instructor role is invalid", body = crate::api_response::ApiErrorResponse),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan management permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 404, description = "Classroom course or instructor assignment not found", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Course instructor role could not be updated", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn update_course_instructor_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((course_id, instructor_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateCourseInstructorRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    context
        .actor
        .require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    course_planning_service::update_course_instructor_role(
        &context.tenant.pool,
        course_id,
        instructor_id,
        &body.role,
    )
    .await?;
    broadcast_course_refresh(&state, &headers, &context, course_id).await;
    Ok(Json(ApiResponse::empty()).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/planning/classrooms/{classroom_id}/activities",
    operation_id = "listClassroomActivities",
    tag = "academic",
    params(
        ("classroom_id" = Uuid, Path, description = "Classroom ID"),
        ClassroomActivityQuery
    ),
    responses(
        (status = 200, description = "Classroom activities", body = ApiResponse<Vec<course_planning_service::ClassroomActivity>>),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan read permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 404, description = "Classroom or semester not found", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Classroom activities could not be loaded", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn list_classroom_activities(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(classroom_id): Path<Uuid>,
    Query(query): Query<ClassroomActivityQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let rows =
        course_planning_service::list_classroom_activities(&pool, classroom_id, query.semester_id)
            .await?;
    Ok(Json(ApiResponse::ok(rows)).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/academic/planning/classrooms/{classroom_id}/activities/{slot_id}",
    operation_id = "removeClassroomFromActivitySlot",
    tag = "academic",
    params(
        ("classroom_id" = Uuid, Path, description = "Classroom ID"),
        ("slot_id" = Uuid, Path, description = "Activity slot ID")
    ),
    responses(
        (status = 200, description = "Classroom removed from activity slot", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Course-plan management permission denied", body = crate::api_response::ApiErrorResponse),
        (status = 404, description = "Classroom activity assignment not found", body = crate::api_response::ApiErrorResponse),
        (status = 500, description = "Classroom could not be removed from activity slot", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn remove_classroom_from_slot(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((classroom_id, slot_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    course_planning_service::remove_classroom_from_slot(&pool, classroom_id, slot_id).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_course_ids_accepts_empty_and_valid_values() {
        assert_eq!(
            parse_course_ids("").expect("empty input should be valid"),
            Vec::<Uuid>::new()
        );

        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        assert_eq!(
            parse_course_ids(&format!("{first}, {second}, {first}"))
                .expect("valid UUIDs should parse"),
            vec![first, second]
        );
    }

    #[test]
    fn parse_course_ids_rejects_any_malformed_value() {
        assert!(matches!(
            parse_course_ids(&format!("{},not-a-uuid", Uuid::new_v4())),
            Err(AppError::BadRequest(message))
                if message == "course_ids must contain valid UUIDs"
        ));
    }
}
