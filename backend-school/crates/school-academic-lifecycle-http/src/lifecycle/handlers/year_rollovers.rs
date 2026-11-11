use super::*;

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/academic/lifecycle/year-rollovers/options",operation_id="getYearRolloverOptions",tag="academic",
    responses((status=200,description="Planning target years returned",body=ApiResponse<YearRolloverOptions>),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission denied",body=ApiErrorResponse),(status=500,description="Internal server error",body=ApiErrorResponse))))]
pub async fn options(
    State(state): State<AcademicLifecycleHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Json<ApiResponse<YearRolloverOptions>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::get_year_rollover_options(&context.tenant.pool, &context.actor).await?,
    )))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/academic/lifecycle/year-rollovers/preview",operation_id="previewYearRollover",tag="academic",request_body=YearRolloverPreviewInput,
    responses((status=200,description="Manual roster preview returned without writes",body=ApiResponse<YearRolloverPreview>),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission denied",body=ApiErrorResponse),(status=404,description="Academic context not found",body=ApiErrorResponse),(status=409,description="Roster conflicts with the current state",body=ApiErrorResponse),(status=422,description="Invalid roster declaration",body=ApiErrorResponse),(status=500,description="Internal server error",body=ApiErrorResponse))))]
pub async fn preview(
    State(state): State<AcademicLifecycleHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(input): Json<YearRolloverPreviewInput>,
) -> Result<Json<ApiResponse<YearRolloverPreview>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::preview_year_rollover(&context.tenant.pool, &context.actor, input).await?,
    )))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/academic/lifecycle/year-rollovers/confirm",operation_id="confirmYearRollover",tag="academic",request_body=ConfirmYearRolloverInput,
    responses((status=200,description="Roster prepared; target year remains inactive",body=ApiResponse<YearRolloverOutcome>),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission denied",body=ApiErrorResponse),(status=404,description="Academic context not found",body=ApiErrorResponse),(status=409,description="Preview or replay identity changed",body=ApiErrorResponse),(status=422,description="Invalid roster declaration",body=ApiErrorResponse),(status=500,description="Internal server error",body=ApiErrorResponse))))]
pub async fn confirm(
    State(state): State<AcademicLifecycleHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(input): Json<ConfirmYearRolloverInput>,
) -> Result<Json<ApiResponse<YearRolloverOutcome>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = services::confirm_year_rollover(
        state.lifecycle_providers,
        &context.tenant.pool,
        &context.actor,
        input,
    )
    .await?;
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "promotion_run",
        Some(result.run_id),
        Some(result.source_year_id),
        None,
    );
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/academic/lifecycle/years/{year_id}/retention",operation_id="getAcademicRetentionWorkspace",tag="academic",params(("year_id"=Uuid,Path)),
    responses((status=200,description="Retention policy and term eligibility returned",body=ApiResponse<AcademicRetentionWorkspace>),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission denied",body=ApiErrorResponse),(status=404,description="Academic year not found",body=ApiErrorResponse),(status=500,description="Internal server error",body=ApiErrorResponse))))]
pub async fn retention_workspace(
    State(state): State<AcademicLifecycleHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(year): Path<Uuid>,
) -> Result<Json<ApiResponse<AcademicRetentionWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::retention::get_workspace(
            state.retention,
            &context.tenant.pool,
            &context.actor,
            year,
        )
        .await?,
    )))
}

#[cfg_attr(feature="api-docs",utoipa::path(put,path="/api/academic/lifecycle/retention-policy",operation_id="updateAcademicRetentionPolicy",tag="academic",request_body=UpdateAcademicRetentionPolicy,
    responses((status=200,description="Future retention policy saved without deleting details",body=ApiResponse<AcademicRetentionPolicy>),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission denied",body=ApiErrorResponse),(status=409,description="Policy revision changed",body=ApiErrorResponse),(status=422,description="Invalid retention policy",body=ApiErrorResponse),(status=500,description="Internal server error",body=ApiErrorResponse))))]
pub async fn update_retention_policy(
    State(state): State<AcademicLifecycleHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(input): Json<UpdateAcademicRetentionPolicy>,
) -> Result<Json<ApiResponse<AcademicRetentionPolicy>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::retention::update_policy(&context.tenant.pool, &context.actor, input).await?,
    )))
}
