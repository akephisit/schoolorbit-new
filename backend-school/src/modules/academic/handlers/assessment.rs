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
    AssessmentPlanListQuery, SaveAssessmentPlanRequest,
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
    let assigned_filter = assessment_service::assigned_instructor_filter_for_list(&actor)?;

    let plans = assessment_service::list_assessment_plans(&pool, &query, assigned_filter).await?;
    Ok(Json(ApiResponse::ok(plans)).into_response())
}

pub async fn get_assessment_plan(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(course_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    assessment_service::require_course_read_access(&context.tenant.pool, &context.actor, course_id)
        .await?;
    let plan = assessment_service::get_or_create_plan_detail(
        &context.tenant.pool,
        course_id,
        context.actor.user_id,
    )
    .await?;
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
    let plan =
        assessment_service::submit_plan(&context.tenant.pool, course_id, context.actor.user_id)
            .await?;
    Ok(Json(ApiResponse::ok(plan)).into_response())
}
