use school_academic_lifecycle::models::*;
use school_academic_lifecycle::services;
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
use school_http::HttpError as AppError;
use school_http::{ApiErrorResponse, ApiResponse};
pub mod promotion_runs;
pub mod year_reopening;
use crate::state::AcademicHttpState;
use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
pub use promotion_runs::{
    approve_promotion_run, calculate_promotion_run, create_promotion_run, execute_promotion_run,
    get_promotion_run_workspace, list_promotion_runs, review_promotion_run_item,
};
use school_academic_lifecycle::services::{term_transitions, year_transitions};
use uuid::Uuid;

#[utoipa::path(get,path="/api/academic/lifecycle/opening-policy",operation_id="getAcademicOpeningPolicy",tag="academic",
    responses((status=200,body=ApiResponse<OpeningPolicy>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse),(status=500,body=ApiErrorResponse)))]
pub async fn get_opening_policy(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Json<ApiResponse<OpeningPolicy>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::get_opening_policy(&context.tenant.pool, &context.actor).await?,
    )))
}

#[utoipa::path(put,path="/api/academic/lifecycle/opening-policy",operation_id="updateAcademicOpeningPolicy",tag="academic",
    request_body=UpdateOpeningPolicyInput,
    responses((status=200,body=ApiResponse<OpeningPolicy>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse),(status=500,body=ApiErrorResponse)))]
pub async fn update_opening_policy(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(input): Json<UpdateOpeningPolicyInput>,
) -> Result<Json<ApiResponse<OpeningPolicy>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let policy =
        services::update_opening_policy(&context.tenant.pool, &context.actor, input).await?;
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "academic_opening_policy",
        None,
        None,
        None,
    );
    Ok(Json(ApiResponse::ok(policy)))
}

#[utoipa::path(get,path="/api/academic/lifecycle/promotion-policies/options",operation_id="getPromotionPolicyOptions",tag="academic",
    responses((status=200,body=ApiResponse<PromotionPolicyOptions>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=422,body=ApiErrorResponse)))]
pub async fn get_promotion_policy_options(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Json<ApiResponse<PromotionPolicyOptions>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::get_promotion_policy_options(&context.tenant.pool, &context.actor).await?,
    )))
}

#[utoipa::path(get,path="/api/academic/lifecycle/promotion-policies",operation_id="listPromotionPolicies",tag="academic",
    responses((status=200,body=ApiResponse<Vec<PromotionPolicyVersion>>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=422,body=ApiErrorResponse)))]
pub async fn list_promotion_policies(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Json<ApiResponse<Vec<PromotionPolicyVersion>>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::list_promotion_policies(&context.tenant.pool, &context.actor).await?,
    )))
}

#[utoipa::path(post,path="/api/academic/lifecycle/promotion-policies",operation_id="createPromotionPolicy",tag="academic",
    request_body=PromotionPolicyInput,
    responses((status=200,body=ApiResponse<PromotionPolicyVersion>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=422,body=ApiErrorResponse)))]
pub async fn create_promotion_policy(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(input): Json<PromotionPolicyInput>,
) -> Result<Json<ApiResponse<PromotionPolicyVersion>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let policy =
        services::create_promotion_policy(&context.tenant.pool, &context.actor, input).await?;
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "promotion_policy",
        Some(policy.id),
        None,
        None,
    );
    Ok(Json(ApiResponse::ok(policy)))
}

#[utoipa::path(get,path="/api/academic/lifecycle/years/{year_id}",operation_id="getYearLifecycleWorkspace",tag="academic",
    params(("year_id"=Uuid,Path)),
    responses((status=200,body=ApiResponse<YearLifecycleWorkspace>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=422,body=ApiErrorResponse)))]
pub async fn get_year_workspace(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(year): Path<Uuid>,
) -> Result<Json<ApiResponse<YearLifecycleWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::get_year_workspace(&context.tenant.pool, &context.actor, year).await?,
    )))
}

#[utoipa::path(post,path="/api/academic/lifecycle/years/{year_id}/transitions",operation_id="transitionAcademicYear",tag="academic",
    params(("year_id"=Uuid,Path)),request_body=YearTransitionRequest,
    responses((status=200,body=ApiResponse<YearTransitionOutcome>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse)))]
pub async fn transition_year(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(year): Path<Uuid>,
    Json(request): Json<YearTransitionRequest>,
) -> Result<Json<ApiResponse<YearTransitionOutcome>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let outcome =
        year_transitions::transition_year(&context.tenant.pool, &context.actor, year, request)
            .await?;
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "academic_year",
        Some(year),
        Some(year),
        None,
    );
    Ok(Json(ApiResponse::ok(outcome)))
}

#[utoipa::path(get,path="/api/academic/lifecycle/terms/{term_id}",operation_id="getTermLifecycleWorkspace",tag="academic",
    params(TermLifecycleQuery,("term_id"=Uuid,Path)),
    responses((status=200,body=ApiResponse<TermLifecycleWorkspace>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=422,body=ApiErrorResponse)))]
pub async fn get_workspace(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(term): Path<Uuid>,
    Query(query): Query<TermLifecycleQuery>,
) -> Result<Json<ApiResponse<TermLifecycleWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::get_workspace(
            state.lifecycle_providers,
            &context.tenant.pool,
            &context.actor,
            query.academic_year_id,
            term,
        )
        .await?,
    )))
}

#[utoipa::path(get,path="/api/academic/lifecycle/terms/{term_id}/activation",operation_id="getTermActivationWorkspace",tag="academic",
    params(TermLifecycleQuery,("term_id"=Uuid,Path)),
    responses((status=200,body=ApiResponse<TermActivationWorkspace>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse),(status=500,body=ApiErrorResponse)))]
pub async fn get_activation_workspace(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(term): Path<Uuid>,
    Query(query): Query<TermLifecycleQuery>,
) -> Result<Json<ApiResponse<TermActivationWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::get_activation_workspace(
            &context.tenant.pool,
            &context.actor,
            query.academic_year_id,
            term,
        )
        .await?,
    )))
}

#[utoipa::path(post,path="/api/academic/lifecycle/terms/{term_id}/transitions",operation_id="transitionAcademicTerm",tag="academic",
    params(("term_id"=Uuid,Path)),request_body=TermTransitionRequest,
    responses((status=200,body=ApiResponse<TermTransitionOutcome>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse)))]
pub async fn transition_term(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(term): Path<Uuid>,
    Json(request): Json<TermTransitionRequest>,
) -> Result<Json<ApiResponse<TermTransitionOutcome>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let outcome = term_transitions::transition_term(
        state.lifecycle_providers,
        &context.tenant.pool,
        &context.actor,
        term,
        request,
    )
    .await?;
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "academic_term",
        Some(term),
        Some(outcome.context.academic_year_id),
        Some(term),
    );
    Ok(Json(ApiResponse::ok(outcome)))
}

#[utoipa::path(post,path="/api/academic/lifecycle/term-preparations/preview",operation_id="previewAcademicTermPreparation",tag="academic",
    request_body=PreviewTermPreparationInput,
    responses((status=200,body=ApiResponse<TermPreparationWorkspace>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse),(status=500,body=ApiErrorResponse)))]
pub async fn preview_term_preparation(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(input): Json<PreviewTermPreparationInput>,
) -> Result<Json<ApiResponse<TermPreparationWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::preview_term_preparation(&context.tenant.pool, &context.actor, input).await?,
    )))
}

#[utoipa::path(post,path="/api/academic/lifecycle/term-preparations/apply",operation_id="applyAcademicTermPreparation",tag="academic",
    request_body=ApplyTermPreparationInput,
    responses((status=200,body=ApiResponse<TermPreparationOutcome>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse),(status=500,body=ApiErrorResponse)))]
pub async fn apply_term_preparation(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(input): Json<ApplyTermPreparationInput>,
) -> Result<Json<ApiResponse<TermPreparationOutcome>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let target_term_id = input.target_term_id;
    let outcome = services::apply_term_preparation(
        state.lifecycle_providers,
        &context.tenant.pool,
        &context.actor,
        input,
    )
    .await?;
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "academic_term_preparation",
        Some(outcome.run_id),
        None,
        Some(target_term_id),
    );
    Ok(Json(ApiResponse::ok(outcome)))
}
