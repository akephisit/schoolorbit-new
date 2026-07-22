use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::academic::services::study_plan_service::{
    self, GenerateActivitiesFromPlanOutcome,
};
use crate::permissions::registry::codes;
use crate::policies::curriculum_access_policy;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use super::super::models::study_plans::*;

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct CountData<T> {
    count: T,
}

#[derive(Debug, Serialize, ToSchema)]
pub(crate) struct GenerateCoursesData {
    items: GenerateCoursesResponse,
    courses_created: i32,
    courses_skipped: i32,
    activities_created: i32,
    activities_skipped: i32,
}

// ============================================
// Study Plans
// ============================================

#[utoipa::path(
    get,
    path = "/api/academic/study-plans",
    operation_id = "listStudyPlans",
    tag = "academic",
    params(("active_only" = Option<bool>, Query, description = "Return active plans only")),
    responses(
        (status = 200, description = "Study plans", body = ApiResponse<Vec<StudyPlan>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum read permission denied", body = ApiErrorResponse),
        (status = 500, description = "Study plans could not be loaded", body = ApiErrorResponse)
    )
)]
pub async fn list_study_plans(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<StudyPlanQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_read(&actor)?;
    let plans = study_plan_service::list_plans(&pool, query).await?;
    Ok(Json(ApiResponse::ok(plans)))
}

#[utoipa::path(
    get,
    path = "/api/academic/study-plans/{id}",
    operation_id = "getStudyPlan",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Study plan ID")),
    responses(
        (status = 200, description = "Study plan", body = ApiResponse<StudyPlan>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study plan not found", body = ApiErrorResponse),
        (status = 500, description = "Study plan could not be loaded", body = ApiErrorResponse)
    )
)]
pub async fn get_study_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(plan_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_read(&actor)?;
    let plan = study_plan_service::get_plan(&pool, plan_id).await?;
    Ok(Json(ApiResponse::ok(plan)))
}

#[utoipa::path(
    post,
    path = "/api/academic/study-plans",
    operation_id = "createStudyPlan",
    tag = "academic",
    request_body = CreateStudyPlanRequest,
    responses(
        (status = 201, description = "Study plan created", body = ApiResponse<StudyPlan>),
        (status = 400, description = "Invalid study plan", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum create permission denied", body = ApiErrorResponse),
        (status = 409, description = "Study plan code already exists", body = ApiErrorResponse),
        (status = 500, description = "Study plan could not be created", body = ApiErrorResponse)
    )
)]
pub async fn create_study_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateStudyPlanRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_create(&actor)?;
    let plan = study_plan_service::create_plan(&pool, req).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(plan))))
}

#[utoipa::path(
    put,
    path = "/api/academic/study-plans/{id}",
    operation_id = "updateStudyPlan",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Study plan ID")),
    request_body = UpdateStudyPlanRequest,
    responses(
        (status = 200, description = "Study plan updated", body = ApiResponse<StudyPlan>),
        (status = 400, description = "Invalid study plan update", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study plan not found", body = ApiErrorResponse),
        (status = 409, description = "Study plan conflicts with an existing plan", body = ApiErrorResponse),
        (status = 500, description = "Study plan could not be updated", body = ApiErrorResponse)
    )
)]
pub async fn update_study_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(plan_id): Path<Uuid>,
    Json(req): Json<UpdateStudyPlanRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_update(&actor)?;
    let plan = study_plan_service::update_plan(&pool, plan_id, req).await?;
    Ok(Json(ApiResponse::ok(plan)))
}

#[utoipa::path(
    delete,
    path = "/api/academic/study-plans/{id}",
    operation_id = "deleteStudyPlan",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Study plan ID")),
    responses(
        (status = 200, description = "Study plan deleted", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum delete permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study plan not found", body = ApiErrorResponse),
        (status = 500, description = "Study plan could not be deleted", body = ApiErrorResponse)
    )
)]
pub async fn delete_study_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(plan_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_delete(&actor)?;
    study_plan_service::delete_plan(&pool, plan_id).await?;
    Ok((StatusCode::OK, Json(ApiResponse::empty())))
}

// ============================================
// Study Plan Versions
// ============================================

#[utoipa::path(
    get,
    path = "/api/academic/study-plan-versions",
    operation_id = "listStudyPlanVersions",
    tag = "academic",
    params(
        ("study_plan_id" = Option<Uuid>, Query, description = "Filter by study plan"),
        ("active_only" = Option<bool>, Query, description = "Return active versions only")
    ),
    responses(
        (status = 200, description = "Study-plan versions", body = ApiResponse<Vec<StudyPlanVersion>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum read permission denied", body = ApiErrorResponse),
        (status = 500, description = "Study-plan versions could not be loaded", body = ApiErrorResponse)
    )
)]
pub async fn list_study_plan_versions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<StudyPlanVersionQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_read(&actor)?;
    let versions = study_plan_service::list_versions(&pool, query).await?;
    Ok(Json(ApiResponse::ok(versions)))
}

#[utoipa::path(
    get,
    path = "/api/academic/study-plan-versions/{id}",
    operation_id = "getStudyPlanVersion",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Study-plan version ID")),
    responses(
        (status = 200, description = "Study-plan version", body = ApiResponse<StudyPlanVersion>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study-plan version not found", body = ApiErrorResponse),
        (status = 500, description = "Study-plan version could not be loaded", body = ApiErrorResponse)
    )
)]
pub async fn get_study_plan_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_read(&actor)?;
    let version = study_plan_service::get_version(&pool, version_id).await?;
    Ok(Json(ApiResponse::ok(version)))
}

#[utoipa::path(
    post,
    path = "/api/academic/study-plan-versions",
    operation_id = "createStudyPlanVersion",
    tag = "academic",
    request_body = CreateStudyPlanVersionRequest,
    responses(
        (status = 201, description = "Study-plan version created", body = ApiResponse<StudyPlanVersion>),
        (status = 400, description = "Invalid study-plan version", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum create permission denied", body = ApiErrorResponse),
        (status = 404, description = "Referenced plan or academic year not found", body = ApiErrorResponse),
        (status = 409, description = "Study-plan version already exists", body = ApiErrorResponse),
        (status = 500, description = "Study-plan version could not be created", body = ApiErrorResponse)
    )
)]
pub async fn create_study_plan_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateStudyPlanVersionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_create(&actor)?;
    let version = study_plan_service::create_version(&pool, req).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(version))))
}

#[utoipa::path(
    put,
    path = "/api/academic/study-plan-versions/{id}",
    operation_id = "updateStudyPlanVersion",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Study-plan version ID")),
    request_body = UpdateStudyPlanVersionRequest,
    responses(
        (status = 200, description = "Study-plan version updated", body = ApiResponse<StudyPlanVersion>),
        (status = 400, description = "Invalid study-plan version update", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study-plan version not found", body = ApiErrorResponse),
        (status = 409, description = "Study-plan version conflicts with an existing version", body = ApiErrorResponse),
        (status = 500, description = "Study-plan version could not be updated", body = ApiErrorResponse)
    )
)]
pub async fn update_study_plan_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
    Json(req): Json<UpdateStudyPlanVersionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_update(&actor)?;
    let version = study_plan_service::update_version(&pool, version_id, req).await?;
    Ok(Json(ApiResponse::ok(version)))
}

#[utoipa::path(
    delete,
    path = "/api/academic/study-plan-versions/{id}",
    operation_id = "deleteStudyPlanVersion",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Study-plan version ID")),
    responses(
        (status = 200, description = "Study-plan version deleted", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum delete permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study-plan version not found", body = ApiErrorResponse),
        (status = 500, description = "Study-plan version could not be deleted", body = ApiErrorResponse)
    )
)]
pub async fn delete_study_plan_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_delete(&actor)?;
    study_plan_service::delete_version(&pool, version_id).await?;
    Ok((StatusCode::OK, Json(ApiResponse::empty())))
}

// ============================================
// Study Plan Subjects
// ============================================

#[utoipa::path(
    get,
    path = "/api/academic/study-plan-versions/{id}/subjects",
    operation_id = "listStudyPlanSubjects",
    tag = "academic",
    params(
        ("id" = Uuid, Path, description = "Study-plan version ID"),
        ("grade_level_id" = Option<Uuid>, Query, description = "Filter by grade level"),
        ("term" = Option<String>, Query, description = "Filter by term")
    ),
    responses(
        (status = 200, description = "Subjects in the study-plan version", body = ApiResponse<Vec<StudyPlanSubject>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study-plan version not found", body = ApiErrorResponse),
        (status = 500, description = "Study-plan subjects could not be loaded", body = ApiErrorResponse)
    )
)]
pub async fn list_study_plan_subjects(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
    Query(mut query): Query<StudyPlanSubjectQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_read(&actor)?;
    query.study_plan_version_id = Some(version_id);
    let subjects = study_plan_service::list_plan_subjects(&pool, query).await?;
    Ok(Json(ApiResponse::ok(subjects)))
}

#[utoipa::path(
    post,
    path = "/api/academic/study-plan-versions/{id}/subjects",
    operation_id = "addSubjectsToStudyPlanVersion",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Study-plan version ID")),
    request_body = AddSubjectsToVersionRequest,
    responses(
        (status = 200, description = "Subjects added to the study-plan version", body = ApiResponse<CountData<usize>>),
        (status = 400, description = "Invalid subject assignment", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study-plan version or referenced item not found", body = ApiErrorResponse),
        (status = 500, description = "Subjects could not be added", body = ApiErrorResponse)
    )
)]
pub async fn add_subjects_to_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
    Json(req): Json<AddSubjectsToVersionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_update(&actor)?;
    let count = study_plan_service::add_subjects_to_version(&pool, version_id, req).await?;
    Ok(Json(ApiResponse::with_message(
        CountData { count },
        "Subjects added successfully",
    )))
}

#[utoipa::path(
    delete,
    path = "/api/academic/study-plan-subjects/{id}",
    operation_id = "deleteStudyPlanSubject",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Study-plan subject row ID")),
    responses(
        (status = 200, description = "Study-plan subject deleted", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum delete permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study-plan subject not found", body = ApiErrorResponse),
        (status = 500, description = "Study-plan subject could not be deleted", body = ApiErrorResponse)
    )
)]
pub async fn delete_study_plan_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sps_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_delete(&actor)?;
    study_plan_service::delete_plan_subject(&pool, sps_id).await?;
    Ok((StatusCode::OK, Json(ApiResponse::empty())))
}

// ============================================
// Generate Courses from Plan
// ============================================

#[utoipa::path(
    post,
    path = "/api/academic/planning/generate-from-plan",
    operation_id = "generateCoursesFromStudyPlan",
    tag = "academic",
    request_body = GenerateCoursesFromPlanRequest,
    responses(
        (status = 200, description = "Classroom courses generated from the assigned study plan", body = ApiResponse<GenerateCoursesData>),
        (status = 400, description = "Classroom has no study plan or request is invalid", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Course-plan management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Classroom or semester not found", body = ApiErrorResponse),
        (status = 500, description = "Courses could not be generated", body = ApiErrorResponse)
    )
)]
pub async fn generate_courses_from_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<GenerateCoursesFromPlanRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;

    let result =
        study_plan_service::generate_courses_from_plan(&pool, req, Some(actor.user_id)).await?;

    Ok(Json(ApiResponse::ok(GenerateCoursesData {
        items: GenerateCoursesResponse {
            added_count: result.courses_created,
            skipped_count: result.courses_skipped,
            message: format!(
                "Added {} courses, skipped {} existing courses; Added {} activities, skipped {}",
                result.courses_created,
                result.courses_skipped,
                result.activities_created,
                result.activities_skipped
            ),
        },
        courses_created: result.courses_created,
        courses_skipped: result.courses_skipped,
        activities_created: result.activities_created,
        activities_skipped: result.activities_skipped,
    })))
}

// ============================================
// Study Plan Version Activities
// ============================================

#[utoipa::path(
    get,
    path = "/api/academic/study-plan-versions/{id}/activities",
    operation_id = "listStudyPlanActivities",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Study-plan version ID")),
    responses(
        (status = 200, description = "Activities in the study-plan version", body = ApiResponse<Vec<StudyPlanVersionActivity>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study-plan version not found", body = ApiErrorResponse),
        (status = 500, description = "Plan activities could not be loaded", body = ApiErrorResponse)
    )
)]
pub async fn list_plan_activities(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_read(&actor)?;
    let rows = study_plan_service::list_plan_activities(&pool, version_id).await?;
    Ok(Json(ApiResponse::ok(rows)))
}

#[utoipa::path(
    post,
    path = "/api/academic/study-plan-versions/{id}/activities",
    operation_id = "addStudyPlanActivity",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Study-plan version ID")),
    request_body = CreatePlanActivityRequest,
    responses(
        (status = 201, description = "Activity added to the study-plan version", body = ApiResponse<StudyPlanVersionActivity>),
        (status = 400, description = "Activity already exists in this plan scope or request is invalid", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study-plan version, catalog, or grade level not found", body = ApiErrorResponse),
        (status = 500, description = "Plan activity could not be added", body = ApiErrorResponse)
    )
)]
pub async fn add_plan_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
    Json(req): Json<CreatePlanActivityRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_update(&actor)?;
    let row = study_plan_service::add_plan_activity(&pool, version_id, req).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(row))))
}

#[utoipa::path(
    put,
    path = "/api/academic/study-plan-activities/{id}",
    operation_id = "updateStudyPlanActivity",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Study-plan activity ID")),
    request_body = UpdatePlanActivityRequest,
    responses(
        (status = 200, description = "Study-plan activity updated", body = ApiResponse<StudyPlanVersionActivity>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study-plan activity not found", body = ApiErrorResponse),
        (status = 500, description = "Plan activity could not be updated", body = ApiErrorResponse)
    )
)]
pub async fn update_plan_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePlanActivityRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_update(&actor)?;
    let row = study_plan_service::update_plan_activity(&pool, id, req).await?;
    Ok(Json(ApiResponse::ok(row)))
}

#[utoipa::path(
    delete,
    path = "/api/academic/study-plan-activities/{id}",
    operation_id = "deleteStudyPlanActivity",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Study-plan activity ID")),
    responses(
        (status = 200, description = "Study-plan activity deleted", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum delete permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study-plan activity not found", body = ApiErrorResponse),
        (status = 500, description = "Plan activity could not be deleted", body = ApiErrorResponse)
    )
)]
pub async fn delete_plan_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_delete(&actor)?;
    study_plan_service::delete_plan_activity(&pool, id).await?;
    Ok(Json(ApiResponse::empty()))
}

#[utoipa::path(
    post,
    path = "/api/academic/activities/generate-from-plan",
    operation_id = "generateActivitiesFromStudyPlan",
    tag = "academic",
    request_body = GenerateActivitiesFromPlanRequest,
    responses(
        (status = 200, description = "Semester activity workspace generated from the study plan", body = ApiResponse<GenerateActivitiesFromPlanOutcome>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum read or school activity management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study-plan version or semester not found", body = ApiErrorResponse),
        (status = 500, description = "Activities could not be generated", body = ApiErrorResponse)
    )
)]
pub async fn generate_activities_from_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<GenerateActivitiesFromPlanRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_read(&actor)?;
    actor.require_permission(codes::ACTIVITY_MANAGE_ALL)?;
    let result =
        study_plan_service::generate_activities_from_plan(&pool, req, Some(actor.user_id)).await?;
    Ok(Json(ApiResponse::ok(result)))
}

// ============================================
// Activity Catalog
// ============================================

#[utoipa::path(
    get,
    path = "/api/academic/activity-catalog",
    operation_id = "listActivityCatalog",
    tag = "academic",
    params(("latest_only" = Option<bool>, Query, description = "Return only the latest active version per name")),
    responses(
        (status = 200, description = "Activity catalog", body = ApiResponse<Vec<ActivityCatalog>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum read permission denied", body = ApiErrorResponse),
        (status = 500, description = "Activity catalog could not be loaded", body = ApiErrorResponse)
    )
)]
pub async fn list_activity_catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<study_plan_service::ActivityCatalogFilter>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_read(&actor)?;
    let latest_only = filter.latest_only.unwrap_or(true);
    let rows = study_plan_service::list_activity_catalog(&pool, latest_only).await?;
    Ok(Json(ApiResponse::ok(rows)))
}

#[utoipa::path(
    post,
    path = "/api/academic/activity-catalog",
    operation_id = "createActivityCatalog",
    tag = "academic",
    request_body = CreateCatalogRequest,
    responses(
        (status = 201, description = "Activity catalog version created", body = ApiResponse<ActivityCatalog>),
        (status = 400, description = "Catalog request is invalid", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum create permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic year, grade level, or instructor not found", body = ApiErrorResponse),
        (status = 500, description = "Activity catalog could not be created", body = ApiErrorResponse)
    )
)]
pub async fn create_activity_catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateCatalogRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_create(&actor)?;
    let row = study_plan_service::create_activity_catalog(&pool, req).await?;
    Ok((StatusCode::CREATED, Json(ApiResponse::ok(row))))
}

#[utoipa::path(
    put,
    path = "/api/academic/activity-catalog/{id}",
    operation_id = "updateActivityCatalog",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity catalog ID")),
    request_body = UpdateCatalogRequest,
    responses(
        (status = 200, description = "Activity catalog updated", body = ApiResponse<ActivityCatalog>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity catalog or grade level not found", body = ApiErrorResponse),
        (status = 500, description = "Activity catalog could not be updated", body = ApiErrorResponse)
    )
)]
pub async fn update_activity_catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateCatalogRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_update(&actor)?;
    let row = study_plan_service::update_activity_catalog(&pool, id, req).await?;
    Ok(Json(ApiResponse::ok(row)))
}

#[utoipa::path(
    delete,
    path = "/api/academic/activity-catalog/{id}",
    operation_id = "deleteActivityCatalog",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity catalog ID")),
    responses(
        (status = 200, description = "Activity catalog deleted", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 400, description = "Catalog is still referenced by a study plan", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum delete permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity catalog not found", body = ApiErrorResponse),
        (status = 500, description = "Activity catalog could not be deleted", body = ApiErrorResponse)
    )
)]
pub async fn delete_activity_catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_delete(&actor)?;
    study_plan_service::delete_activity_catalog(&pool, id).await?;
    Ok(Json(ApiResponse::empty()))
}

// ============================================
// Activity Catalog Default Instructors
// ============================================

#[utoipa::path(
    get,
    path = "/api/academic/activity-catalog/{id}/default-instructors",
    operation_id = "listActivityCatalogDefaultInstructors",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity catalog ID")),
    responses(
        (status = 200, description = "Default activity instructors", body = ApiResponse<Vec<CatalogDefaultInstructor>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity catalog not found", body = ApiErrorResponse),
        (status = 500, description = "Default instructors could not be loaded", body = ApiErrorResponse)
    )
)]
pub async fn list_catalog_default_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(catalog_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_read(&actor)?;
    let rows = study_plan_service::list_catalog_default_instructors(&pool, catalog_id).await?;
    Ok(Json(ApiResponse::ok(rows)))
}

#[utoipa::path(
    post,
    path = "/api/academic/activity-catalog/{id}/default-instructors",
    operation_id = "addActivityCatalogDefaultInstructor",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity catalog ID")),
    request_body = AddCatalogDefaultInstructorRequest,
    responses(
        (status = 200, description = "Default instructor added", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 400, description = "Instructor role is invalid", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity catalog or instructor not found", body = ApiErrorResponse),
        (status = 500, description = "Default instructor could not be added", body = ApiErrorResponse)
    )
)]
pub async fn add_catalog_default_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(catalog_id): Path<Uuid>,
    Json(body): Json<AddCatalogDefaultInstructorRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_update(&actor)?;
    let role = body.role.unwrap_or_else(|| "secondary".to_string());
    study_plan_service::add_catalog_default_instructor(
        &pool,
        catalog_id,
        body.instructor_id,
        &role,
    )
    .await?;
    Ok(Json(ApiResponse::empty()))
}

#[utoipa::path(
    delete,
    path = "/api/academic/activity-catalog/{id}/default-instructors/{uid}",
    operation_id = "removeActivityCatalogDefaultInstructor",
    tag = "academic",
    params(
        ("id" = Uuid, Path, description = "Activity catalog ID"),
        ("uid" = Uuid, Path, description = "Instructor user ID")
    ),
    responses(
        (status = 200, description = "Default instructor removed", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Default instructor assignment not found", body = ApiErrorResponse),
        (status = 500, description = "Default instructor could not be removed", body = ApiErrorResponse)
    )
)]
pub async fn remove_catalog_default_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((catalog_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_update(&actor)?;
    study_plan_service::remove_catalog_default_instructor(&pool, catalog_id, instructor_id).await?;
    Ok(Json(ApiResponse::empty()))
}

#[utoipa::path(
    put,
    path = "/api/academic/activity-catalog/{id}/default-instructors/{uid}",
    operation_id = "updateActivityCatalogDefaultInstructorRole",
    tag = "academic",
    params(
        ("id" = Uuid, Path, description = "Activity catalog ID"),
        ("uid" = Uuid, Path, description = "Instructor user ID")
    ),
    request_body = UpdateCatalogDefaultInstructorRoleRequest,
    responses(
        (status = 200, description = "Default instructor role updated", body = ApiResponse<crate::api_response::EmptyData>),
        (status = 400, description = "Instructor role is invalid", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Curriculum update permission denied", body = ApiErrorResponse),
        (status = 404, description = "Default instructor assignment not found", body = ApiErrorResponse),
        (status = 500, description = "Default instructor role could not be updated", body = ApiErrorResponse)
    )
)]
pub async fn update_catalog_default_instructor_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((catalog_id, instructor_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateCatalogDefaultInstructorRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    curriculum_access_policy::ensure_curriculum_update(&actor)?;
    study_plan_service::update_catalog_default_instructor_role(
        &pool,
        catalog_id,
        instructor_id,
        &body.role,
    )
    .await?;
    Ok(Json(ApiResponse::empty()))
}
