use super::*;

#[utoipa::path(get,path="/api/academic/lifecycle/promotion-runs/{run_id}/impacts",operation_id="getPromotionRunImpacts",tag="academic",
 params(("run_id"=Uuid,Path),PromotionImpactQuery),
 responses((status=200,body=ApiResponse<PromotionImpactWorkspace>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=422,body=ApiErrorResponse),(status=500,body=ApiErrorResponse)))]
pub async fn get_promotion_run_impacts(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(run_id): Path<Uuid>,
    Query(query): Query<PromotionImpactQuery>,
) -> Result<Json<ApiResponse<PromotionImpactWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::get_promotion_impacts(&context.tenant.pool, &context.actor, run_id, query)
            .await?,
    )))
}

#[utoipa::path(post,path="/api/academic/lifecycle/promotion-runs/{run_id}/impacts/{impact_id}/resolve",operation_id="resolvePromotionRunImpact",tag="academic",
 params(("run_id"=Uuid,Path),("impact_id"=Uuid,Path)),request_body=ResolvePromotionImpactInput,
 responses((status=200,body=ApiResponse<PromotionImpactResolution>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse),(status=500,body=ApiErrorResponse)))]
pub async fn resolve_promotion_run_impact(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((run_id, impact_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<ResolvePromotionImpactInput>,
) -> Result<Json<ApiResponse<PromotionImpactResolution>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let outcome = services::resolve_promotion_impact(
        &context.tenant.pool,
        &context.actor,
        run_id,
        impact_id,
        input,
    )
    .await?;
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "promotion_impact",
        Some(impact_id),
        None,
        None,
    );
    Ok(Json(ApiResponse::ok(outcome)))
}

#[utoipa::path(get,path="/api/academic/lifecycle/promotion-runs",operation_id="listPromotionRuns",tag="academic",
 params(PromotionRunListQuery),
 responses((status=200,body=ApiResponse<PromotionRunList>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse),(status=500,body=ApiErrorResponse)))]
pub async fn list_promotion_runs(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<PromotionRunListQuery>,
) -> Result<Json<ApiResponse<PromotionRunList>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let outcome = services::list_runs(&context.tenant.pool, &context.actor, query).await?;

    Ok(Json(ApiResponse::ok(outcome)))
}

#[utoipa::path(get,path="/api/academic/lifecycle/promotion-runs/{run_id}",operation_id="getPromotionRunWorkspace",tag="academic",
 params(("run_id"=Uuid,Path)),
 responses((status=200,body=ApiResponse<PromotionRunWorkspace>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse),(status=500,body=ApiErrorResponse)))]
pub async fn get_promotion_run_workspace(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,

    Path(run_id): Path<Uuid>,
) -> Result<Json<ApiResponse<PromotionRunWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let outcome = services::get_run_workspace(&context.tenant.pool, &context.actor, run_id).await?;

    Ok(Json(ApiResponse::ok(outcome)))
}

#[utoipa::path(post,path="/api/academic/lifecycle/promotion-runs",operation_id="createPromotionRun",tag="academic",
 request_body=CreatePromotionRunInput,
 responses((status=200,body=ApiResponse<PromotionRun>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse),(status=500,body=ApiErrorResponse)))]
pub async fn create_promotion_run(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,

    Json(input): Json<CreatePromotionRunInput>,
) -> Result<Json<ApiResponse<PromotionRun>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let outcome = services::create_run(&context.tenant.pool, &context.actor, input).await?;
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "promotion_run",
        Some(outcome.id),
        Some(outcome.source_year_id),
        None,
    );
    Ok(Json(ApiResponse::ok(outcome)))
}

#[utoipa::path(post,path="/api/academic/lifecycle/promotion-runs/{run_id}/calculate",operation_id="calculatePromotionRun",tag="academic",
 params(("run_id"=Uuid,Path)),request_body=CalculatePromotionRunInput,
 responses((status=200,body=ApiResponse<PromotionRunCalculation>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse),(status=500,body=ApiErrorResponse)))]
pub async fn calculate_promotion_run(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,

    Path(run_id): Path<Uuid>,
    Json(input): Json<CalculatePromotionRunInput>,
) -> Result<Json<ApiResponse<PromotionRunCalculation>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let outcome =
        services::calculate_run(&context.tenant.pool, &context.actor, run_id, input).await?;
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "promotion_run",
        Some(run_id),
        Some(outcome.run.source_year_id),
        None,
    );
    Ok(Json(ApiResponse::ok(outcome)))
}

#[utoipa::path(put,path="/api/academic/lifecycle/promotion-runs/{run_id}/items/{item_id}",operation_id="reviewPromotionRunItem",tag="academic",
 params(("run_id"=Uuid,Path),("item_id"=Uuid,Path)),request_body=ReviewPromotionItemInput,
 responses((status=200,body=ApiResponse<PromotionItemReview>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse),(status=500,body=ApiErrorResponse)))]
pub async fn review_promotion_run_item(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,

    Path((run_id, item_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<ReviewPromotionItemInput>,
) -> Result<Json<ApiResponse<PromotionItemReview>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let outcome =
        services::review_item(&context.tenant.pool, &context.actor, run_id, item_id, input).await?;
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "promotion_run",
        Some(run_id),
        Some(outcome.run.source_year_id),
        None,
    );
    Ok(Json(ApiResponse::ok(outcome)))
}

#[utoipa::path(post,path="/api/academic/lifecycle/promotion-runs/{run_id}/approve",operation_id="approvePromotionRun",tag="academic",
 params(("run_id"=Uuid,Path)),request_body=ApprovePromotionRunInput,
 responses((status=200,body=ApiResponse<PromotionRun>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse),(status=500,body=ApiErrorResponse)))]
pub async fn approve_promotion_run(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,

    Path(run_id): Path<Uuid>,
    Json(input): Json<ApprovePromotionRunInput>,
) -> Result<Json<ApiResponse<PromotionRun>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let outcome =
        services::approve_run(&context.tenant.pool, &context.actor, run_id, input).await?;
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "promotion_run",
        Some(run_id),
        Some(outcome.source_year_id),
        None,
    );
    Ok(Json(ApiResponse::ok(outcome)))
}

#[utoipa::path(post,path="/api/academic/lifecycle/promotion-runs/{run_id}/execute",operation_id="executePromotionRun",tag="academic",
 params(("run_id"=Uuid,Path)),request_body=ExecutePromotionRunInput,
 responses((status=200,body=ApiResponse<PromotionExecutionResult>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse),(status=500,body=ApiErrorResponse)))]
pub async fn execute_promotion_run(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,

    Path(run_id): Path<Uuid>,
    Json(input): Json<ExecutePromotionRunInput>,
) -> Result<Json<ApiResponse<PromotionExecutionResult>>, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let outcome =
        services::execute_run(&context.tenant.pool, &context.actor, run_id, input).await?;
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        session.user_id,
        "promotion_run",
        Some(run_id),
        Some(outcome.run.source_year_id),
        None,
    );
    Ok(Json(ApiResponse::ok(outcome)))
}
