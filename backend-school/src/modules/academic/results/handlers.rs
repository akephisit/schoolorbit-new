use super::{models::*, services};
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::{
    api_response::{ApiErrorResponse, ApiResponse},
    error::AppError,
    AppState,
};
use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use uuid::Uuid;

fn signal(state: &AppState, session: &AuthenticatedSession, context: &ResultContext, id: Uuid) {
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "academic_result",
        Some(id),
        Some(context.academic_year_id),
        Some(context.academic_term_id),
    );
}

#[utoipa::path(get,path="/api/academic/results/policies",operation_id="listAcademicGradingPolicies",tag="academic",params(ResultContext),responses((status=200,body=ApiResponse<Vec<GradingPolicyVersion>>),(status=403,body=ApiErrorResponse)))]
pub async fn list_policies(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<ResultContext>,
) -> Result<Json<ApiResponse<Vec<GradingPolicyVersion>>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::list_policies(&context.tenant.pool, &context.actor, &query).await?,
    )))
}

#[utoipa::path(post,path="/api/academic/results/policies",operation_id="createAcademicGradingPolicy",tag="academic",params(ResultContext),request_body=GradingPolicyInput,responses((status=200,body=ApiResponse<GradingPolicyVersion>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn create_policy(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<ResultContext>,
    Json(input): Json<GradingPolicyInput>,
) -> Result<Json<ApiResponse<GradingPolicyVersion>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result =
        services::create_policy(&context.tenant.pool, &context.actor, &query, input).await?;
    signal(&state, &session, &query, result.id);
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(post,path="/api/academic/results/policies/{policy_id}/activate",operation_id="activateAcademicGradingPolicy",tag="academic",params(ResultContext,("policy_id"=Uuid,Path)),request_body=PolicyActivationInput,responses((status=200,body=ApiResponse<GradingPolicyVersion>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn activate_policy(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(policy_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
    Json(input): Json<PolicyActivationInput>,
) -> Result<Json<ApiResponse<GradingPolicyVersion>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::activate_policy(
        &context.tenant.pool,
        &context.actor,
        &query,
        policy_id,
        input.row_version,
    )
    .await?;
    signal(&state, &session, &query, policy_id);
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(get,path="/api/academic/results/groups/{group_id}/course",operation_id="getCourseResultPreparation",tag="academic",params(ResultContext,("group_id"=Uuid,Path)),responses((status=200,body=ApiResponse<CoursePreparationWorkspace>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn get_course_workspace(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(group_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
) -> Result<Json<ApiResponse<CoursePreparationWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::get_course_workspace(&context.tenant.pool, &context.actor, &query, group_id)
            .await?,
    )))
}

#[utoipa::path(put,path="/api/academic/results/groups/{group_id}/course/selection",operation_id="saveCourseResultSelection",tag="academic",params(ResultContext,("group_id"=Uuid,Path)),request_body=SelectionInput,responses((status=200,body=ApiResponse<CoursePreparationWorkspace>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn save_selection(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(group_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
    Json(input): Json<SelectionInput>,
) -> Result<Json<ApiResponse<CoursePreparationWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::save_selection(
        &context.tenant.pool,
        &context.actor,
        &query,
        group_id,
        input,
    )
    .await?;
    signal(&state, &session, &query, group_id);
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(post,path="/api/academic/results/groups/{group_id}/course/confirm",operation_id="confirmCourseGroupResults",tag="academic",params(ResultContext,("group_id"=Uuid,Path)),request_body=ResultConfirmationInput,responses((status=200,body=ApiResponse<CoursePreparationWorkspace>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn confirm_group_results(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(group_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
    Json(input): Json<ResultConfirmationInput>,
) -> Result<Json<ApiResponse<CoursePreparationWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::confirm_group_results(
        &context.tenant.pool,
        &context.actor,
        &query,
        group_id,
        input,
    )
    .await?;
    signal(&state, &session, &query, group_id);
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(get,path="/api/academic/results/groups/{group_id}/activity",operation_id="getActivityResultPreparation",tag="academic",params(ResultContext,("group_id"=Uuid,Path)),responses((status=200,body=ApiResponse<ActivityPreparationWorkspace>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn get_activity_workspace(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(group_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
) -> Result<Json<ApiResponse<ActivityPreparationWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::get_activity_workspace(&context.tenant.pool, &context.actor, &query, group_id)
            .await?,
    )))
}

#[utoipa::path(put,path="/api/academic/results/groups/{group_id}/activity/outcomes",operation_id="saveActivityResultOutcomes",tag="academic",params(ResultContext,("group_id"=Uuid,Path)),request_body=ActivityBatchInput,responses((status=200,body=ApiResponse<ActivityPreparationWorkspace>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn save_activity_outcomes(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(group_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
    Json(input): Json<ActivityBatchInput>,
) -> Result<Json<ApiResponse<ActivityPreparationWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::save_activity_outcomes(
        &context.tenant.pool,
        &context.actor,
        &query,
        group_id,
        input,
    )
    .await?;
    signal(&state, &session, &query, group_id);
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(post,path="/api/academic/results/groups/{group_id}/activity/confirm",operation_id="confirmActivityGroupResults",tag="academic",params(ResultContext,("group_id"=Uuid,Path)),request_body=ResultConfirmationInput,responses((status=200,body=ApiResponse<ActivityPreparationWorkspace>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn confirm_activity(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(group_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
    Json(input): Json<ResultConfirmationInput>,
) -> Result<Json<ApiResponse<ActivityPreparationWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::confirm_activity(
        &context.tenant.pool,
        &context.actor,
        &query,
        group_id,
        input,
    )
    .await?;
    signal(&state, &session, &query, group_id);
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(get,path="/api/academic/results/readiness",operation_id="getAcademicResultReadiness",tag="academic",params(ResultContext),responses((status=200,body=ApiResponse<ResultReadiness>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn readiness(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<ResultContext>,
) -> Result<Json<ApiResponse<ResultReadiness>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::readiness(&context.tenant.pool, &context.actor, &query).await?,
    )))
}
