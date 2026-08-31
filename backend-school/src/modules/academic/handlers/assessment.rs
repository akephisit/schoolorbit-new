use axum::{
    extract::{Extension, Path, Query, State},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::academic::models::assessment::{
    AssessmentPhaseControlListQuery, AssessmentPlanListQuery, SaveAssessmentPlanRequest,
    UpdateAssessmentPhaseControlRequest,
};
use crate::modules::academic::services::assessment_service;
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::policies::learning_offering_access_policy::{
    require_learning_offering_access, require_learning_offering_list_access, OfferingAction,
};
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::AppState;

#[utoipa::path(
    get,
    path = "/api/academic/assessments/plans",
    operation_id = "listAssessmentPlans",
    tag = "academic",
    params(AssessmentPlanListQuery),
    responses(
        (status = 200, description = "Assessment plans for the selected term", body = ApiResponse<Vec<crate::modules::academic::models::assessment::AssessmentPlanSummary>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Assessment read permission denied", body = ApiErrorResponse)
    )
)]
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
    let plans =
        assessment_service::list_assessment_plans(&context.tenant.pool, &query, &access).await?;
    Ok(Json(ApiResponse::ok(plans)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/assessments/phase-controls",
    operation_id = "listAssessmentPhaseControls",
    tag = "academic",
    params(AssessmentPhaseControlListQuery),
    responses(
        (status = 200, description = "Assessment phase controls", body = ApiResponse<Vec<crate::modules::academic::models::assessment::AssessmentPhaseControl>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Assessment phase control read permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_assessment_phase_controls(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<AssessmentPhaseControlListQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    assessment_service::require_phase_controls_read_access(&context.actor)?;
    let controls =
        assessment_service::list_phase_controls(&context.tenant.pool, query.academic_term_id)
            .await?;
    Ok(Json(ApiResponse::ok(controls)).into_response())
}

#[utoipa::path(
    put,
    path = "/api/academic/assessments/phase-controls/{control_id}",
    operation_id = "updateAssessmentPhaseControl",
    tag = "academic",
    params(("control_id" = Uuid, Path, description = "Assessment phase control ID")),
    request_body = UpdateAssessmentPhaseControlRequest,
    responses(
        (status = 200, description = "Updated assessment phase control", body = ApiResponse<crate::modules::academic::models::assessment::AssessmentPhaseControl>),
        (status = 400, description = "Invalid assessment phase control", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Assessment phase control manage permission denied", body = ApiErrorResponse),
        (status = 409, description = "Stale assessment phase control version", body = ApiErrorResponse)
    )
)]
pub async fn update_assessment_phase_control(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(control_id): Path<Uuid>,
    Json(payload): Json<UpdateAssessmentPhaseControlRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    assessment_service::require_phase_controls_manage_access(&context.actor)?;
    let control = assessment_service::update_phase_control(
        &context.tenant.pool,
        control_id,
        context.actor.user_id,
        payload,
    )
    .await?;
    Ok(Json(ApiResponse::ok(control)).into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/assessments/offerings/{offering_id}",
    operation_id = "getAssessmentPlan",
    tag = "academic",
    params(("offering_id" = Uuid, Path, description = "Learning offering ID")),
    responses(
        (status = 200, description = "Assessment plan for an offering", body = ApiResponse<crate::modules::academic::models::assessment::AssessmentPlanDetail>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Assessment plan read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Offering not found", body = ApiErrorResponse)
    )
)]
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
    let plan = assessment_service::get_plan_detail(&context.tenant.pool, offering_id).await?;
    Ok(Json(ApiResponse::ok(plan)).into_response())
}

#[utoipa::path(
    put,
    path = "/api/academic/assessments/offerings/{offering_id}",
    operation_id = "saveAssessmentPlan",
    tag = "academic",
    params(("offering_id" = Uuid, Path, description = "Learning offering ID")),
    request_body = SaveAssessmentPlanRequest,
    responses(
        (status = 200, description = "Saved assessment plan", body = ApiResponse<crate::modules::academic::models::assessment::AssessmentPlanDetail>),
        (status = 400, description = "Invalid assessment plan", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Assessment plan manage permission denied", body = ApiErrorResponse),
        (status = 404, description = "Offering not found", body = ApiErrorResponse),
        (status = 409, description = "Stale assessment plan version", body = ApiErrorResponse)
    )
)]
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
    let can_manage_school = assessment_service::actor_can_manage_all_plans(&context.actor);
    let plan = assessment_service::save_plan(
        &context.tenant.pool,
        offering_id,
        context.actor.user_id,
        can_manage_school,
        payload,
    )
    .await?;
    Ok(Json(ApiResponse::ok(plan)).into_response())
}
