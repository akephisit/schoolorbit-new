use crate::db::school_mapping::get_school_database_url;
use crate::error::AppError;
use crate::modules::academic::services::study_plan_service;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;
use uuid::Uuid;

use super::super::models::study_plans::*;

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
// Study Plans
// ============================================

pub async fn list_study_plans(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<StudyPlanQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let plans = study_plan_service::list_plans(&pool, query).await?;
    Ok(Json(json!({ "success": true, "data": plans })))
}

pub async fn get_study_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(plan_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let plan = study_plan_service::get_plan(&pool, plan_id).await?;
    Ok(Json(json!({ "success": true, "data": plan })))
}

pub async fn create_study_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateStudyPlanRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let plan = study_plan_service::create_plan(&pool, req).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": plan })),
    ))
}

pub async fn update_study_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(plan_id): Path<Uuid>,
    Json(req): Json<UpdateStudyPlanRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let plan = study_plan_service::update_plan(&pool, plan_id, req).await?;
    Ok(Json(json!({ "success": true, "data": plan })))
}

pub async fn delete_study_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(plan_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    study_plan_service::delete_plan(&pool, plan_id).await?;
    Ok((StatusCode::OK, Json(json!({ "success": true, "data": {} }))))
}

// ============================================
// Study Plan Versions
// ============================================

pub async fn list_study_plan_versions(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<StudyPlanVersionQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let versions = study_plan_service::list_versions(&pool, query).await?;
    Ok(Json(json!({ "success": true, "data": versions })))
}

pub async fn get_study_plan_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let version = study_plan_service::get_version(&pool, version_id).await?;
    Ok(Json(json!({ "success": true, "data": version })))
}

pub async fn create_study_plan_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateStudyPlanVersionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let version = study_plan_service::create_version(&pool, req).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": version })),
    ))
}

pub async fn update_study_plan_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
    Json(req): Json<UpdateStudyPlanVersionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let version = study_plan_service::update_version(&pool, version_id, req).await?;
    Ok(Json(json!({ "success": true, "data": version })))
}

pub async fn delete_study_plan_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    study_plan_service::delete_version(&pool, version_id).await?;
    Ok((StatusCode::OK, Json(json!({ "success": true, "data": {} }))))
}

// ============================================
// Study Plan Subjects
// ============================================

pub async fn list_study_plan_subjects(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<StudyPlanSubjectQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let subjects = study_plan_service::list_plan_subjects(&pool, query).await?;
    Ok(Json(json!({ "success": true, "data": subjects })))
}

pub async fn add_subjects_to_version(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
    Json(req): Json<AddSubjectsToVersionRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let count = study_plan_service::add_subjects_to_version(&pool, version_id, req).await?;
    Ok(Json(
        json!({ "success": true, "data": { "count": count }, "message": "Subjects added successfully" }),
    ))
}

pub async fn delete_study_plan_subject(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(sps_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    study_plan_service::delete_plan_subject(&pool, sps_id).await?;
    Ok((StatusCode::OK, Json(json!({ "success": true, "data": {} }))))
}

// ============================================
// Generate Courses from Plan
// ============================================

pub async fn generate_courses_from_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<GenerateCoursesFromPlanRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
        .await
        .ok();

    let result = study_plan_service::generate_courses_from_plan(&pool, req, user_id).await?;

    Ok(Json(
        json!({ "success": true, "data": { "items": GenerateCoursesResponse {
            added_count: result.courses_created,
            skipped_count: result.courses_skipped,
            message: format!(
                "Added {} courses, skipped {} existing courses; Added {} activities, skipped {}",
                result.courses_created, result.courses_skipped, result.activities_created, result.activities_skipped
            ),
        }, "courses_created": result.courses_created, "courses_skipped": result.courses_skipped, "activities_created": result.activities_created, "activities_skipped": result.activities_skipped } }),
    ))
}

// ============================================
// Study Plan Version Activities
// ============================================

pub async fn list_plan_activities(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let rows = study_plan_service::list_plan_activities(&pool, version_id).await?;
    Ok(Json(json!({ "success": true, "data": rows })))
}

pub async fn add_plan_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(version_id): Path<Uuid>,
    Json(req): Json<CreatePlanActivityRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let row = study_plan_service::add_plan_activity(&pool, version_id, req).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": row })),
    ))
}

pub async fn update_plan_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdatePlanActivityRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let row = study_plan_service::update_plan_activity(&pool, id, req).await?;
    Ok(Json(json!({ "success": true, "data": row })))
}

pub async fn delete_plan_activity(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    study_plan_service::delete_plan_activity(&pool, id).await?;
    Ok(Json(json!({ "success": true, "data": {} })))
}

pub async fn generate_activities_from_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<GenerateActivitiesFromPlanRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
        .await
        .ok();
    let (created, skipped, total) =
        study_plan_service::generate_activities_from_plan(&pool, req, user_id).await?;
    Ok(Json(
        json!({ "success": true, "data": { "created": created, "skipped": skipped, "total_templates": total } }),
    ))
}

// ============================================
// Activity Catalog
// ============================================

pub async fn list_activity_catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<study_plan_service::ActivityCatalogFilter>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let latest_only = filter.latest_only.unwrap_or(true);
    let rows = study_plan_service::list_activity_catalog(&pool, latest_only).await?;
    Ok(Json(json!({ "success": true, "data": rows })))
}

pub async fn create_activity_catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(req): Json<CreateCatalogRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let row = study_plan_service::create_activity_catalog(&pool, req).await?;
    Ok((
        StatusCode::CREATED,
        Json(json!({ "success": true, "data": row })),
    ))
}

pub async fn update_activity_catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpdateCatalogRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let row = study_plan_service::update_activity_catalog(&pool, id, req).await?;
    Ok(Json(json!({ "success": true, "data": row })))
}

pub async fn delete_activity_catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    study_plan_service::delete_activity_catalog(&pool, id).await?;
    Ok(Json(json!({ "success": true, "data": {} })))
}

// ============================================
// Activity Catalog Default Instructors
// ============================================

pub async fn list_catalog_default_instructors(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(catalog_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let rows = study_plan_service::list_catalog_default_instructors(&pool, catalog_id).await?;
    Ok(Json(json!({ "success": true, "data": rows })))
}

pub async fn add_catalog_default_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(catalog_id): Path<Uuid>,
    Json(body): Json<AddCatalogDefaultInstructorRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let role = body.role.unwrap_or_else(|| "secondary".to_string());
    study_plan_service::add_catalog_default_instructor(
        &pool,
        catalog_id,
        body.instructor_id,
        &role,
    )
    .await?;
    Ok(Json(json!({ "success": true, "data": {} })))
}

pub async fn remove_catalog_default_instructor(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((catalog_id, instructor_id)): Path<(Uuid, Uuid)>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    study_plan_service::remove_catalog_default_instructor(&pool, catalog_id, instructor_id).await?;
    Ok(Json(json!({ "success": true, "data": {} })))
}

pub async fn update_catalog_default_instructor_role(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path((catalog_id, instructor_id)): Path<(Uuid, Uuid)>,
    Json(body): Json<UpdateCatalogDefaultInstructorRoleRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    study_plan_service::update_catalog_default_instructor_role(
        &pool,
        catalog_id,
        instructor_id,
        &body.role,
    )
    .await?;
    Ok(Json(json!({ "success": true, "data": {} })))
}
