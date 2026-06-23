use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::academic::models::assessment::{
    AssessmentPlanListQuery, SaveAssessmentPlanRequest, UpdateAssessmentSettingsRequest,
};
use crate::modules::academic::services::assessment_service;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

pub async fn list_assessment_plans(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<AssessmentPlanListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let actor = context.actor;
    let pool = context.tenant.pool;
    let access = assessment_service::resolve_assessment_plan_list_access(&pool, &actor).await?;
    if !access.is_school() {
        assessment_service::require_teacher_access_enabled_for_assigned_reader(&pool, &actor)
            .await?;
    }

    let plans = assessment_service::list_assessment_plans(&pool, &query, &access).await?;
    Ok(Json(ApiResponse::ok(plans)).into_response())
}

pub async fn get_assessment_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    assessment_service::require_assessment_settings_read_access(&context.actor)?;
    let settings = assessment_service::get_assessment_settings(&context.tenant.pool).await?;
    Ok(Json(ApiResponse::ok(settings)).into_response())
}

pub async fn update_assessment_settings(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<UpdateAssessmentSettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    assessment_service::require_assessment_settings_manage_access(&context.actor)?;
    let settings =
        assessment_service::update_assessment_settings(&context.tenant.pool, payload).await?;
    Ok(Json(ApiResponse::ok(settings)).into_response())
}

pub async fn get_assessment_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(course_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    assessment_service::require_course_read_access(&context.tenant.pool, &context.actor, course_id)
        .await?;
    assessment_service::require_teacher_access_enabled_for_assigned_reader(
        &context.tenant.pool,
        &context.actor,
    )
    .await?;
    let plan = assessment_service::get_plan_detail(&context.tenant.pool, course_id).await?;
    Ok(Json(ApiResponse::ok(plan)).into_response())
}

pub async fn save_assessment_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(course_id): Path<Uuid>,
    Json(payload): Json<SaveAssessmentPlanRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    assessment_service::require_course_manage_access(
        &context.tenant.pool,
        &context.actor,
        course_id,
    )
    .await?;
    assessment_service::require_teacher_access_enabled_for_assigned_manager(
        &context.tenant.pool,
        &context.actor,
    )
    .await?;
    let plan = assessment_service::save_plan(
        &context.tenant.pool,
        course_id,
        context.actor.user_id,
        payload,
    )
    .await?;
    Ok(Json(ApiResponse::ok(plan)).into_response())
}

pub async fn submit_assessment_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(course_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    assessment_service::require_course_manage_access(
        &context.tenant.pool,
        &context.actor,
        course_id,
    )
    .await?;
    assessment_service::require_teacher_access_enabled_for_assigned_manager(
        &context.tenant.pool,
        &context.actor,
    )
    .await?;
    let plan =
        assessment_service::submit_plan(&context.tenant.pool, course_id, context.actor.user_id)
            .await?;
    Ok(Json(ApiResponse::ok(plan)).into_response())
}
