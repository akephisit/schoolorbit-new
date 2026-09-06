use super::{models::*, services};
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::{
    api_response::{ApiErrorResponse, ApiResponse, EmptyData},
    error::AppError,
    AppState,
};
use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use uuid::Uuid;

fn signal(state: &AppState, session: &AuthenticatedSession, ctx: &EvaluationContext, id: Uuid) {
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "learner_evaluation",
        Some(id),
        Some(ctx.academic_year_id),
        Some(ctx.academic_term_id),
    );
}

#[utoipa::path(get,path="/api/academic/learner-evaluations/subjects",operation_id="listLearnerEvaluationSubjects",tag="academic",params(EvaluationContext),responses((status=200,body=ApiResponse<Vec<EvaluationSubject>>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn list_subjects(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<EvaluationContext>,
) -> Result<Json<ApiResponse<Vec<EvaluationSubject>>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::list_subjects(&context.tenant.pool, &context.actor, &query).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(get,path="/api/academic/learner-evaluations/catalog",operation_id="listLearnerEvaluationCatalog",tag="academic",params(EvaluationContext),responses((status=200,body=ApiResponse<Vec<CatalogCriterion>>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn list_catalog(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<EvaluationContext>,
) -> Result<Json<ApiResponse<Vec<CatalogCriterion>>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::list_catalog(&context.tenant.pool, &context.actor, &query).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(post,path="/api/academic/learner-evaluations/catalog",operation_id="createLearnerEvaluationCatalog",tag="academic",params(EvaluationContext),request_body=CatalogInput,responses((status=200,body=ApiResponse<CatalogCriterion>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn create_catalog(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<EvaluationContext>,
    Json(input): Json<CatalogInput>,
) -> Result<Json<ApiResponse<CatalogCriterion>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result =
        services::save_catalog(&context.tenant.pool, &context.actor, &query, None, input).await?;
    signal(&state, &session, &query, result.id);
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(put,path="/api/academic/learner-evaluations/catalog/{criterion_id}",operation_id="updateLearnerEvaluationCatalog",tag="academic",params(EvaluationContext,("criterion_id"=Uuid,Path)),request_body=CatalogInput,responses((status=200,body=ApiResponse<CatalogCriterion>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn update_catalog(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(criterion_id): Path<Uuid>,
    Query(query): Query<EvaluationContext>,
    Json(input): Json<CatalogInput>,
) -> Result<Json<ApiResponse<CatalogCriterion>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::save_catalog(
        &context.tenant.pool,
        &context.actor,
        &query,
        Some(criterion_id),
        input,
    )
    .await?;
    signal(&state, &session, &query, criterion_id);
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(delete,path="/api/academic/learner-evaluations/catalog/{criterion_id}",operation_id="removeLearnerEvaluationCatalog",tag="academic",params(EvaluationContext,("criterion_id"=Uuid,Path)),request_body=VersionInput,responses((status=200,body=ApiResponse<EmptyData>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn remove_catalog(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(criterion_id): Path<Uuid>,
    Query(query): Query<EvaluationContext>,
    Json(input): Json<VersionInput>,
) -> Result<Json<ApiResponse<EmptyData>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    services::remove_catalog(
        &context.tenant.pool,
        &context.actor,
        &query,
        criterion_id,
        input.row_version,
    )
    .await?;
    signal(&state, &session, &query, criterion_id);
    Ok(Json(ApiResponse::empty()))
}

#[utoipa::path(get,path="/api/academic/learner-evaluations/controls",operation_id="listLearnerEvaluationControls",tag="academic",params(EvaluationContext),responses((status=200,body=ApiResponse<Vec<EvaluationControl>>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn list_controls(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<EvaluationContext>,
) -> Result<Json<ApiResponse<Vec<EvaluationControl>>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::list_controls(&context.tenant.pool, &context.actor, &query).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(put,path="/api/academic/learner-evaluations/controls/{domain}",operation_id="updateLearnerEvaluationControl",tag="academic",params(EvaluationContext,("domain"=LearnerEvaluationDomain,Path)),request_body=ControlInput,responses((status=200,body=ApiResponse<EvaluationControl>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn update_control(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(domain): Path<LearnerEvaluationDomain>,
    Query(query): Query<EvaluationContext>,
    Json(input): Json<ControlInput>,
) -> Result<Json<ApiResponse<EvaluationControl>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result =
        services::update_control(&context.tenant.pool, &context.actor, domain, &query, input)
            .await?;
    signal(&state, &session, &query, result.id);
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(get,path="/api/academic/learner-evaluations/subjects/{subject_id}/domains/{domain}/configuration",operation_id="getLearnerEvaluationConfiguration",tag="academic",params(EvaluationContext,("subject_id"=Uuid,Path),("domain"=LearnerEvaluationDomain,Path)),responses((status=200,body=ApiResponse<EvaluationConfiguration>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn get_configuration(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((subject_id, domain)): Path<(Uuid, LearnerEvaluationDomain)>,
    Query(query): Query<EvaluationContext>,
) -> Result<Json<ApiResponse<EvaluationConfiguration>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::get_configuration(
        &context.tenant.pool,
        &context.actor,
        subject_id,
        domain,
        &query,
    )
    .await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(post,path="/api/academic/learner-evaluations/subjects/{subject_id}/domains/{domain}/criteria",operation_id="createSubjectEvaluationCriterion",tag="academic",params(EvaluationContext,("subject_id"=Uuid,Path),("domain"=LearnerEvaluationDomain,Path)),request_body=CriterionInput,responses((status=200,body=ApiResponse<EvaluationCriterion>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn create_criterion(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((subject_id, domain)): Path<(Uuid, LearnerEvaluationDomain)>,
    Query(query): Query<EvaluationContext>,
    Json(input): Json<CriterionInput>,
) -> Result<Json<ApiResponse<EvaluationCriterion>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::save_criterion(
        &context.tenant.pool,
        &context.actor,
        subject_id,
        domain,
        &query,
        None,
        input,
    )
    .await?;
    signal(&state, &session, &query, subject_id);
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(put,path="/api/academic/learner-evaluations/subjects/{subject_id}/domains/{domain}/criteria/{criterion_id}",operation_id="updateSubjectEvaluationCriterion",tag="academic",params(EvaluationContext,("subject_id"=Uuid,Path),("domain"=LearnerEvaluationDomain,Path),("criterion_id"=Uuid,Path)),request_body=CriterionInput,responses((status=200,body=ApiResponse<EvaluationCriterion>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn update_criterion(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((subject_id, domain, criterion_id)): Path<(Uuid, LearnerEvaluationDomain, Uuid)>,
    Query(query): Query<EvaluationContext>,
    Json(input): Json<CriterionInput>,
) -> Result<Json<ApiResponse<EvaluationCriterion>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::save_criterion(
        &context.tenant.pool,
        &context.actor,
        subject_id,
        domain,
        &query,
        Some(criterion_id),
        input,
    )
    .await?;
    signal(&state, &session, &query, subject_id);
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(delete,path="/api/academic/learner-evaluations/subjects/{subject_id}/domains/{domain}/criteria/{criterion_id}",operation_id="removeSubjectEvaluationCriterion",tag="academic",params(EvaluationContext,("subject_id"=Uuid,Path),("domain"=LearnerEvaluationDomain,Path),("criterion_id"=Uuid,Path)),request_body=VersionInput,responses((status=200,body=ApiResponse<CriterionRemoval>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn remove_criterion(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((subject_id, domain, criterion_id)): Path<(Uuid, LearnerEvaluationDomain, Uuid)>,
    Query(query): Query<EvaluationContext>,
    Json(input): Json<VersionInput>,
) -> Result<Json<ApiResponse<CriterionRemoval>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::remove_criterion(
        &context.tenant.pool,
        &context.actor,
        subject_id,
        domain,
        &query,
        criterion_id,
        input.row_version,
    )
    .await?;
    signal(&state, &session, &query, subject_id);
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(get,path="/api/academic/learner-evaluations/groups/{group_id}/domains/{domain}",operation_id="getLearnerEvaluationWorkspace",tag="academic",params(EvaluationContext,("group_id"=Uuid,Path),("domain"=LearnerEvaluationDomain,Path)),responses((status=200,body=ApiResponse<EvaluationWorkspace>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn get_workspace(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((group_id, domain)): Path<(Uuid, LearnerEvaluationDomain)>,
    Query(query): Query<EvaluationContext>,
) -> Result<Json<ApiResponse<EvaluationWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::get_workspace(
        &context.tenant.pool,
        &context.actor,
        group_id,
        domain,
        &query,
    )
    .await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(put,path="/api/academic/learner-evaluations/groups/{group_id}/domains/{domain}/responses",operation_id="saveLearnerEvaluationResponses",tag="academic",params(EvaluationContext,("group_id"=Uuid,Path),("domain"=LearnerEvaluationDomain,Path)),request_body=ResponseBatchInput,responses((status=200,body=ApiResponse<EvaluationWorkspace>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn save_responses(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((group_id, domain)): Path<(Uuid, LearnerEvaluationDomain)>,
    Query(query): Query<EvaluationContext>,
    Json(input): Json<ResponseBatchInput>,
) -> Result<Json<ApiResponse<EvaluationWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::save_responses(
        &context.tenant.pool,
        &context.actor,
        group_id,
        domain,
        &query,
        input.cells,
    )
    .await?;
    signal(&state, &session, &query, group_id);
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(post,path="/api/academic/learner-evaluations/groups/{group_id}/domains/{domain}/confirm",operation_id="confirmLearnerEvaluationGroup",tag="academic",params(EvaluationContext,("group_id"=Uuid,Path),("domain"=LearnerEvaluationDomain,Path)),request_body=ConfirmationInput,responses((status=200,body=ApiResponse<ConfirmationOutcome>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn confirm_group(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((group_id, domain)): Path<(Uuid, LearnerEvaluationDomain)>,
    Query(query): Query<EvaluationContext>,
    Json(input): Json<ConfirmationInput>,
) -> Result<Json<ApiResponse<ConfirmationOutcome>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::confirm_group(
        &context.tenant.pool,
        &context.actor,
        group_id,
        domain,
        &query,
        input,
    )
    .await?;
    signal(&state, &session, &query, group_id);
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(post,path="/api/academic/learner-evaluations/subjects/{subject_id}/domains/{domain}/lock",operation_id="lockLearnerEvaluationSubject",tag="academic",params(EvaluationContext,("subject_id"=Uuid,Path),("domain"=LearnerEvaluationDomain,Path)),responses((status=200,body=ApiResponse<LockOutcome>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn lock_subject(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((subject_id, domain)): Path<(Uuid, LearnerEvaluationDomain)>,
    Query(query): Query<EvaluationContext>,
) -> Result<Json<ApiResponse<LockOutcome>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::lock_subject(
        &context.tenant.pool,
        &context.actor,
        subject_id,
        domain,
        &query,
    )
    .await?;
    signal(&state, &session, &query, subject_id);
    Ok(Json(ApiResponse::ok(result)))
}

#[utoipa::path(get,path="/api/academic/learner-evaluations/students/{student_academic_year_id}/summary",operation_id="getStudentLearnerEvaluationSummary",tag="academic",params(EvaluationContext,("student_academic_year_id"=Uuid,Path)),responses((status=200,body=ApiResponse<StudentEvaluationSummary>),(status=403,body=ApiErrorResponse),(status=409,body=ApiErrorResponse)))]
pub async fn student_summary(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(student_academic_year_id): Path<Uuid>,
    Query(query): Query<EvaluationContext>,
) -> Result<Json<ApiResponse<StudentEvaluationSummary>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let result = services::summary_for_actor(
        &context.tenant.pool,
        &context.actor,
        &query,
        student_academic_year_id,
    )
    .await?;
    Ok(Json(ApiResponse::ok(result)))
}
