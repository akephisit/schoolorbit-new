use crate::state::AcademicHttpState;
use axum::{
    extract::{Extension, Path, Query, State},
    Json,
};
use school_academic_results::{models::*, services};
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
#[cfg(feature = "api-docs")]
use school_http::ApiErrorResponse;
use school_http::ApiResponse;
use school_http::HttpError as AppError;
use uuid::Uuid;

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/academic/results/annual-students",operation_id="listAnnualResultStudents",tag="academic",params(AnnualResultContext),responses((status=200, description = "Annual result student list returned",body=ApiResponse<Vec<AnnualResultStudent>>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse),(status=422, description = "Validation failed",body=ApiErrorResponse))))]
pub async fn list_annual_students(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<AnnualResultContext>,
) -> Result<Json<ApiResponse<Vec<AnnualResultStudent>>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::list_annual_students(
            &context.tenant.pool,
            &context.actor,
            query.academic_year_id,
        )
        .await?,
    )))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/academic/results/students/{student_year_id}/annual-preview",operation_id="previewAnnualResult",tag="academic",params(AnnualResultContext,("student_year_id"=Uuid,Path)),responses((status=200, description = "Annual result preview returned",body=ApiResponse<AnnualResultPreview>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse),(status=422, description = "Validation failed",body=ApiErrorResponse))))]
pub async fn preview_annual(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(student_year_id): Path<Uuid>,
    Query(query): Query<AnnualResultContext>,
) -> Result<Json<ApiResponse<AnnualResultPreview>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::preview_annual(
            &context.tenant.pool,
            &context.actor,
            query.academic_year_id,
            student_year_id,
        )
        .await?,
    )))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/academic/results/students/{student_year_id}/annual-revisions",operation_id="listAnnualResultRevisions",tag="academic",params(AnnualResultContext,("student_year_id"=Uuid,Path)),responses((status=200, description = "Annual result revisions returned",body=ApiResponse<Vec<AnnualResultRevision>>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse),(status=422, description = "Validation failed",body=ApiErrorResponse))))]
pub async fn list_annual_revisions(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(student_year_id): Path<Uuid>,
    Query(query): Query<AnnualResultContext>,
) -> Result<Json<ApiResponse<Vec<AnnualResultRevision>>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::list_annual_revisions(
            &context.tenant.pool,
            &context.actor,
            query.academic_year_id,
            student_year_id,
        )
        .await?,
    )))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/academic/results/students/{student_year_id}/annual-revisions",operation_id="lockAnnualResult",tag="academic",params(AnnualResultContext,("student_year_id"=Uuid,Path)),request_body=AnnualLockInput,responses((status=200, description = "Annual result revision lock result returned",body=ApiResponse<AnnualResultRevision>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse),(status=422, description = "Validation failed",body=ApiErrorResponse))))]
pub async fn lock_annual(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(student_year_id): Path<Uuid>,
    Query(query): Query<AnnualResultContext>,
    Json(input): Json<AnnualLockInput>,
) -> Result<Json<ApiResponse<AnnualResultRevision>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::lock_annual(
        &context.tenant.pool,
        &context.actor,
        query.academic_year_id,
        student_year_id,
        input,
    )
    .await?;
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "academic_result",
        Some(result.id),
        Some(query.academic_year_id),
        None,
    );
    Ok(Json(ApiResponse::ok(result)))
}

fn signal(
    state: &AcademicHttpState,
    session: &AuthenticatedSession,
    context: &ResultContext,
    id: Uuid,
) {
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "academic_result",
        Some(id),
        Some(context.academic_year_id),
        Some(context.academic_term_id),
    );
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/academic/results/aggregate-students",operation_id="listAggregateStudents",tag="academic",params(ResultContext),responses((status=200, description = "Aggregate result student list returned",body=ApiResponse<Vec<AggregateStudent>>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse),(status=422, description = "Validation failed",body=ApiErrorResponse))))]
pub async fn list_aggregate_students(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<ResultContext>,
) -> Result<Json<ApiResponse<Vec<AggregateStudent>>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::list_aggregate_students(&context.tenant.pool, &context.actor, &query).await?,
    )))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/academic/results/aggregate-policies",operation_id="listAggregatePolicies",tag="academic",responses((status=200, description = "Result aggregation policy versions returned",body=ApiResponse<Vec<AggregatePolicyVersion>>),(status=403, description = "Permission denied",body=ApiErrorResponse))))]
pub async fn list_aggregate_policies(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Json<ApiResponse<Vec<AggregatePolicyVersion>>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::list_aggregate_policies(&context.tenant.pool, &context.actor).await?,
    )))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/academic/results/aggregate-policies",operation_id="createAggregatePolicy",tag="academic",request_body=AggregatePolicyInput,responses((status=200, description = "Result aggregation policy version created",body=ApiResponse<AggregatePolicyVersion>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse))))]
pub async fn create_aggregate_policy(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(input): Json<AggregatePolicyInput>,
) -> Result<Json<ApiResponse<AggregatePolicyVersion>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result =
        services::create_aggregate_policy(&context.tenant.pool, &context.actor, input).await?;
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "academic_result",
        Some(result.id),
        None,
        None,
    );
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/academic/results/students/{student_year_id}/aggregate-preview",operation_id="previewTermAggregate",tag="academic",params(AggregatePreviewQuery,("student_year_id"=Uuid,Path)),responses((status=200, description = "Term aggregate result preview returned",body=ApiResponse<TermAggregatePreview>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse))))]
pub async fn preview_aggregate(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(student_year_id): Path<Uuid>,
    Query(query): Query<AggregatePreviewQuery>,
) -> Result<Json<ApiResponse<TermAggregatePreview>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::preview_aggregate(
            &context.tenant.pool,
            &context.actor,
            student_year_id,
            &query,
        )
        .await?,
    )))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/academic/results/students/{student_year_id}/aggregate-revisions",operation_id="listTermAggregateRevisions",tag="academic",params(ResultContext,("student_year_id"=Uuid,Path)),responses((status=200, description = "Term aggregate result revisions returned",body=ApiResponse<Vec<TermAggregateRevision>>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse))))]
pub async fn list_term_aggregate_revisions(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(student_year_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
) -> Result<Json<ApiResponse<Vec<TermAggregateRevision>>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::list_term_aggregate_revisions(
            &context.tenant.pool,
            &context.actor,
            &query,
            student_year_id,
        )
        .await?,
    )))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/academic/results/students/{student_year_id}/aggregate-revisions",operation_id="lockTermAggregate",tag="academic",params(ResultContext,("student_year_id"=Uuid,Path)),request_body=AggregateLockInput,responses((status=200, description = "Term aggregate result revision lock result returned",body=ApiResponse<TermAggregateRevision>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn lock_term_aggregate(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(student_year_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
    Json(input): Json<AggregateLockInput>,
) -> Result<Json<ApiResponse<TermAggregateRevision>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::lock_term_aggregate(
        &context.tenant.pool,
        &context.actor,
        &query,
        student_year_id,
        input,
    )
    .await?;
    signal(&state, &session, &query, result.id);
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/academic/results/students/{student_year_id}/term-preview",operation_id="previewStudentTermResults",tag="academic",params(TermResultPreviewQuery,("student_year_id"=Uuid,Path)),responses((status=200, description = "Student term result preview returned",body=ApiResponse<TermResultPreview>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse))))]
pub async fn preview_student_term(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(student_year_id): Path<Uuid>,
    Query(query): Query<TermResultPreviewQuery>,
) -> Result<Json<ApiResponse<TermResultPreview>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::preview_student_term(
            &context.tenant.pool,
            &context.actor,
            student_year_id,
            &query,
        )
        .await?,
    )))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/academic/results/policies",operation_id="listAcademicGradingPolicies",tag="academic",params(ResultContext),responses((status=200, description = "Grading policy versions returned",body=ApiResponse<Vec<GradingPolicyVersion>>),(status=403, description = "Permission denied",body=ApiErrorResponse))))]
pub async fn list_policies(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<ResultContext>,
) -> Result<Json<ApiResponse<Vec<GradingPolicyVersion>>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::list_policies(&context.tenant.pool, &context.actor, &query).await?,
    )))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/academic/results/policies",operation_id="createAcademicGradingPolicy",tag="academic",params(ResultContext),request_body=GradingPolicyInput,responses((status=200, description = "Grading policy version created",body=ApiResponse<GradingPolicyVersion>),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn create_policy(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<ResultContext>,
    Json(input): Json<GradingPolicyInput>,
) -> Result<Json<ApiResponse<GradingPolicyVersion>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result =
        services::create_policy(&context.tenant.pool, &context.actor, &query, input).await?;
    signal(&state, &session, &query, result.id);
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/academic/results/policies/{policy_id}/activate",operation_id="activateAcademicGradingPolicy",tag="academic",params(ResultContext,("policy_id"=Uuid,Path)),request_body=PolicyActivationInput,responses((status=200, description = "Grading policy activated",body=ApiResponse<GradingPolicyVersion>),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn activate_policy(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(policy_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
    Json(input): Json<PolicyActivationInput>,
) -> Result<Json<ApiResponse<GradingPolicyVersion>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/academic/results/groups/{group_id}/course",operation_id="getCourseResultPreparation",tag="academic",params(ResultContext,("group_id"=Uuid,Path)),responses((status=200, description = "Course result preparation workspace returned",body=ApiResponse<CoursePreparationWorkspace>),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn get_course_workspace(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(group_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
) -> Result<Json<ApiResponse<CoursePreparationWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::get_course_workspace(&context.tenant.pool, &context.actor, &query, group_id)
            .await?,
    )))
}

#[cfg_attr(feature = "api-docs", utoipa::path(put,path="/api/academic/results/groups/{group_id}/course/selection",operation_id="saveCourseResultSelection",tag="academic",params(ResultContext,("group_id"=Uuid,Path)),request_body=SelectionInput,responses((status=200, description = "Course result selection saved",body=ApiResponse<CoursePreparationWorkspace>),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn save_selection(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(group_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
    Json(input): Json<SelectionInput>,
) -> Result<Json<ApiResponse<CoursePreparationWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/academic/results/groups/{group_id}/course/confirm",operation_id="confirmCourseGroupResults",tag="academic",params(ResultContext,("group_id"=Uuid,Path)),request_body=ResultConfirmationInput,responses((status=200, description = "Course result confirmation result returned",body=ApiResponse<CoursePreparationWorkspace>),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn confirm_group_results(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(group_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
    Json(input): Json<ResultConfirmationInput>,
) -> Result<Json<ApiResponse<CoursePreparationWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/academic/results/groups/{group_id}/activity",operation_id="getActivityResultPreparation",tag="academic",params(ResultContext,("group_id"=Uuid,Path)),responses((status=200, description = "Activity result preparation workspace returned",body=ApiResponse<ActivityPreparationWorkspace>),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn get_activity_workspace(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(group_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
) -> Result<Json<ApiResponse<ActivityPreparationWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::get_activity_workspace(&context.tenant.pool, &context.actor, &query, group_id)
            .await?,
    )))
}

#[cfg_attr(feature = "api-docs", utoipa::path(put,path="/api/academic/results/groups/{group_id}/activity/outcomes",operation_id="saveActivityResultOutcomes",tag="academic",params(ResultContext,("group_id"=Uuid,Path)),request_body=ActivityBatchInput,responses((status=200, description = "Activity result outcomes saved",body=ApiResponse<ActivityPreparationWorkspace>),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn save_activity_outcomes(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(group_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
    Json(input): Json<ActivityBatchInput>,
) -> Result<Json<ApiResponse<ActivityPreparationWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/academic/results/groups/{group_id}/activity/confirm",operation_id="confirmActivityGroupResults",tag="academic",params(ResultContext,("group_id"=Uuid,Path)),request_body=ResultConfirmationInput,responses((status=200, description = "Activity result confirmation result returned",body=ApiResponse<ActivityPreparationWorkspace>),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn confirm_activity(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(group_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
    Json(input): Json<ResultConfirmationInput>,
) -> Result<Json<ApiResponse<ActivityPreparationWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/academic/results/readiness",operation_id="getAcademicResultReadiness",tag="academic",params(ResultContext),responses((status=200, description = "Academic result readiness returned",body=ApiResponse<ResultReadiness>),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn readiness(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<ResultContext>,
) -> Result<Json<ApiResponse<ResultReadiness>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::readiness(&context.tenant.pool, &context.actor, &query).await?,
    )))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/academic/results/subjects/{subject_id}/lock",operation_id="lockCourseSubjectResults",tag="academic",params(ResultContext,("subject_id"=Uuid,Path)),responses((status=200, description = "Course subject result lock result returned",body=ApiResponse<CourseResultLockOutcome>),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn lock_course_subject(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(subject_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
) -> Result<Json<ApiResponse<CourseResultLockOutcome>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result =
        services::lock_course_subject(&context.tenant.pool, &context.actor, &query, subject_id)
            .await?;
    if result.lock.is_some() {
        signal(&state, &session, &query, subject_id);
    }
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/academic/results/groups/{group_id}/activity/lock",operation_id="lockActivityGroupResults",tag="academic",params(ResultContext,("group_id"=Uuid,Path)),responses((status=200, description = "Activity group result lock result returned",body=ApiResponse<ActivityResultLockOutcome>),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn lock_activity_group(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(group_id): Path<Uuid>,
    Query(query): Query<ResultContext>,
) -> Result<Json<ApiResponse<ActivityResultLockOutcome>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result =
        services::lock_activity_group(&context.tenant.pool, &context.actor, &query, group_id)
            .await?;
    if result.lock.is_some() {
        signal(&state, &session, &query, group_id);
    }
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/academic/results/activities/lock-ready",operation_id="lockAllReadyActivityResults",tag="academic",params(ResultContext),responses((status=200, description = "Ready activity result lock outcomes returned",body=ApiResponse<BulkActivityResultLockOutcome>),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn lock_all_ready_activities(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<ResultContext>,
) -> Result<Json<ApiResponse<BulkActivityResultLockOutcome>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result =
        services::lock_all_ready_activities(&context.tenant.pool, &context.actor, &query).await?;
    if !result.locked.is_empty() {
        signal(&state, &session, &query, query.academic_term_id);
    }
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/academic/results/effective",operation_id="searchEffectiveAcademicResults",tag="academic",params(EffectiveResultSearch),responses((status=200, description = "Effective academic result search results returned",body=ApiResponse<Vec<EffectiveResultSearchItem>>),(status=403, description = "Permission denied",body=ApiErrorResponse))))]
pub async fn search_effective_results(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<EffectiveResultSearch>,
) -> Result<Json<ApiResponse<Vec<EffectiveResultSearchItem>>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::search_effective_results(&context.tenant.pool, &context.actor, &query).await?,
    )))
}

#[cfg_attr(feature = "api-docs", utoipa::path(post,path="/api/academic/results/corrections",operation_id="correctEffectiveAcademicResult",tag="academic",params(ResultContext),request_body=ResultCorrectionInput,responses((status=200, description = "Academic result correction result returned",body=ApiResponse<EffectiveResult>),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn correct_result(
    State(state): State<AcademicHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<ResultContext>,
    Json(input): Json<ResultCorrectionInput>,
) -> Result<Json<ApiResponse<EffectiveResult>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result =
        services::correct_result(&context.tenant.pool, &context.actor, &query, input).await?;
    signal(&state, &session, &query, result.result_id);
    Ok(Json(ApiResponse::ok(result)))
}
