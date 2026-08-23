use axum::{
    extract::{Extension, Path, Query, State},
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
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::policies::learning_offering_access_policy::{
    require_learning_offering_access, require_learning_offering_list_access, OfferingAction,
};
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::AppState;

pub async fn list_assessment_plans(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<AssessmentPlanListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let access = require_learning_offering_list_access(
        &context.tenant.pool,
        &context.actor,
        OfferingAction::Read,
    )
    .await?;
    assessment_service::require_teacher_access_enabled_for_reader(
        &context.tenant.pool,
        &context.actor,
    )
    .await?;
    let plans =
        assessment_service::list_assessment_plans(&context.tenant.pool, &query, &access).await?;
    Ok(Json(ApiResponse::ok(plans)).into_response())
}

pub async fn get_assessment_settings(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    assessment_service::require_assessment_settings_read_access(&context.actor)?;
    let settings = assessment_service::get_assessment_settings(&context.tenant.pool).await?;
    Ok(Json(ApiResponse::ok(settings)).into_response())
}

pub async fn update_assessment_settings(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<UpdateAssessmentSettingsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    assessment_service::require_assessment_settings_manage_access(&context.actor)?;
    let settings =
        assessment_service::update_assessment_settings(&context.tenant.pool, payload).await?;
    Ok(Json(ApiResponse::ok(settings)).into_response())
}

pub async fn get_assessment_plan(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(offering_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_learning_offering_access(
        &context.tenant.pool,
        &context.actor,
        offering_id,
        OfferingAction::Read,
    )
    .await?;
    assessment_service::require_teacher_access_enabled_for_reader(
        &context.tenant.pool,
        &context.actor,
    )
    .await?;
    let plan = assessment_service::get_plan_detail(&context.tenant.pool, offering_id).await?;
    Ok(Json(ApiResponse::ok(plan)).into_response())
}

pub async fn save_assessment_plan(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(offering_id): Path<Uuid>,
    Json(payload): Json<SaveAssessmentPlanRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_learning_offering_access(
        &context.tenant.pool,
        &context.actor,
        offering_id,
        OfferingAction::Manage,
    )
    .await?;
    assessment_service::require_teacher_access_enabled_for_manager(
        &context.tenant.pool,
        &context.actor,
    )
    .await?;
    let plan = assessment_service::save_plan(
        &context.tenant.pool,
        offering_id,
        context.actor.user_id,
        payload,
    )
    .await?;
    Ok(Json(ApiResponse::ok(plan)).into_response())
}

pub async fn submit_assessment_plan(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(offering_id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    require_learning_offering_access(
        &context.tenant.pool,
        &context.actor,
        offering_id,
        OfferingAction::Manage,
    )
    .await?;
    assessment_service::require_teacher_access_enabled_for_manager(
        &context.tenant.pool,
        &context.actor,
    )
    .await?;
    let plan =
        assessment_service::submit_plan(&context.tenant.pool, offering_id, context.actor.user_id)
            .await?;
    Ok(Json(ApiResponse::ok(plan)).into_response())
}
