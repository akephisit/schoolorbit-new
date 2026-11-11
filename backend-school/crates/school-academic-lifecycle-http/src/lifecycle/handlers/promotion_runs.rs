use super::*;
use school_auth_http::context::actor_tenant_context_from_session;

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/academic/lifecycle/promotion-runs/{run_id}/impacts",operation_id="getPromotionRunImpacts",tag="academic",
 params(("run_id"=Uuid,Path),PromotionImpactQuery),
 responses((status=200, description = "Promotion run impacts returned",body=ApiResponse<PromotionImpactWorkspace>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=422, description = "Validation failed",body=ApiErrorResponse),(status=500, description = "Internal server error",body=ApiErrorResponse))))]
pub async fn get_promotion_run_impacts(
    State(state): State<AcademicLifecycleHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(run_id): Path<Uuid>,
    Query(query): Query<PromotionImpactQuery>,
) -> Result<Json<ApiResponse<PromotionImpactWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    Ok(Json(ApiResponse::ok(
        services::get_promotion_impacts(&context.tenant.pool, &context.actor, run_id, query)
            .await?,
    )))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/academic/lifecycle/promotion-runs",operation_id="listPromotionRuns",tag="academic",
 params(PromotionRunListQuery),
 responses((status=200, description = "Promotion runs returned",body=ApiResponse<PromotionRunList>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse),(status=422, description = "Validation failed",body=ApiErrorResponse),(status=500, description = "Internal server error",body=ApiErrorResponse))))]
pub async fn list_promotion_runs(
    State(state): State<AcademicLifecycleHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<PromotionRunListQuery>,
) -> Result<Json<ApiResponse<PromotionRunList>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let outcome = services::list_runs(&context.tenant.pool, &context.actor, query).await?;

    Ok(Json(ApiResponse::ok(outcome)))
}

#[cfg_attr(feature = "api-docs", utoipa::path(get,path="/api/academic/lifecycle/promotion-runs/{run_id}",operation_id="getPromotionRunWorkspace",tag="academic",
 params(("run_id"=Uuid,Path)),
 responses((status=200, description = "Promotion run workspace returned",body=ApiResponse<PromotionRunWorkspace>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse),(status=422, description = "Validation failed",body=ApiErrorResponse),(status=500, description = "Internal server error",body=ApiErrorResponse))))]
pub async fn get_promotion_run_workspace(
    State(state): State<AcademicLifecycleHttpState>,
    Extension(session): Extension<AuthenticatedSession>,

    Path(run_id): Path<Uuid>,
) -> Result<Json<ApiResponse<PromotionRunWorkspace>>, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let outcome = services::get_run_workspace(&context.tenant.pool, &context.actor, run_id).await?;

    Ok(Json(ApiResponse::ok(outcome)))
}
