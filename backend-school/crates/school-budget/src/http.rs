//! Thin authenticated transport. Session, tenant and CSRF remain in existing middleware.
use crate::{models::*, ports::BudgetCatalogPort};
use axum::{
    extract::{Path, Query, State},
    Extension, Json, Router,
};
use school_auth::runtime::AuthRuntime;
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
#[cfg(feature = "api-docs")]
use school_http::ApiErrorResponse;
use school_http::ApiResponse;
use school_http::HttpError;
use school_notifications::events::WorkChangeEvent;
use uuid::Uuid;
#[derive(Clone)]
pub struct BudgetHttpState {
    pub auth_runtime: AuthRuntime,
    pub catalog: &'static dyn BudgetCatalogPort,
    pub work_event_channel: tokio::sync::broadcast::Sender<WorkChangeEvent>,
}
impl BudgetHttpState {
    fn after_commit(&self, tenant: &str) {
        school_notifications::runtime::wake_after_commit(tenant);
        if let Err(error) = self
            .work_event_channel
            .send(WorkChangeEvent::work_items_changed(tenant))
        {
            tracing::debug!(%error,"budget_work_signal_without_subscribers");
        }
    }
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/budget/catalogs",operation_id="getBudgetCatalogs",tag="budget",responses((status=200,body=ApiResponse<BudgetCatalogs>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn get_catalogs(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Json<ApiResponse<BudgetCatalogs>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::settings::catalogs(&c.tenant.pool, &c.actor).await?;
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/budget/settings",operation_id="getBudgetSettings",tag="budget",responses((status=200,body=ApiResponse<BudgetSettings>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn get_settings(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Json<ApiResponse<BudgetSettings>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::settings::get(&c.tenant.pool, &c.actor).await?;
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(put,path="/api/budget/settings",operation_id="saveBudgetSettings",tag="budget",request_body=SaveBudgetSettingsRequest,responses((status=200,body=ApiResponse<BudgetSettings>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn save_settings(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(input): Json<SaveBudgetSettingsRequest>,
) -> Result<Json<ApiResponse<BudgetSettings>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::settings::save(&c.tenant.pool, &c.actor, input).await?;
    s.after_commit(&c.tenant.subdomain);
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/budget/projects",operation_id="listBudgetProjects",tag="budget",params(BudgetListQuery),responses((status=200,body=ApiResponse<BudgetProjectPage>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn list_projects(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(q): Query<BudgetListQuery>,
) -> Result<Json<ApiResponse<BudgetProjectPage>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::queries::projects(&c.tenant.pool, &c.actor, q).await?;
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/budget/projects",operation_id="createBudgetProject",tag="budget",request_body=CreateBudgetProjectRequest,responses((status=200,body=ApiResponse<BudgetProjectWorkspace>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn create_project(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(input): Json<CreateBudgetProjectRequest>,
) -> Result<Json<ApiResponse<BudgetProjectWorkspace>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::projects::create(&c.tenant.pool, &c.actor, input).await?;
    s.after_commit(&c.tenant.subdomain);
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/budget/projects/{id}",operation_id="getBudgetProject",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),responses((status=200,body=ApiResponse<BudgetProjectWorkspace>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn get_project(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<BudgetProjectWorkspace>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::queries::workspace(&c.tenant.pool, &c.actor, id).await?;
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(put,path="/api/budget/projects/{id}",operation_id="saveBudgetProject",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),request_body=SaveBudgetProjectRequest,responses((status=200,body=ApiResponse<BudgetProjectWorkspace>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn save_project(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<SaveBudgetProjectRequest>,
) -> Result<Json<ApiResponse<BudgetProjectWorkspace>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::projects::save(&c.tenant.pool, &c.actor, id, input, s.catalog).await?;
    s.after_commit(&c.tenant.subdomain);
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/budget/projects/{id}/submit",operation_id="submitBudgetProject",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),request_body=BudgetMutationRequest,responses((status=200,body=ApiResponse<BudgetProjectWorkspace>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn submit_project(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<BudgetMutationRequest>,
) -> Result<Json<ApiResponse<BudgetProjectWorkspace>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::projects::submit(&c.tenant.pool, &c.actor, id, input).await?;
    s.after_commit(&c.tenant.subdomain);
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/budget/projects/{id}/actions",operation_id="actBudgetProject",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),request_body=BudgetActionRequest,responses((status=200,body=ApiResponse<BudgetProjectWorkspace>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn act_project(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<BudgetActionRequest>,
) -> Result<Json<ApiResponse<BudgetProjectWorkspace>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::projects::act(&c.tenant.pool, &c.actor, id, input).await?;
    s.after_commit(&c.tenant.subdomain);
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/budget/projects/{id}/amendments",operation_id="startBudgetAmendment",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),request_body=StartBudgetAmendmentRequest,responses((status=200,body=ApiResponse<BudgetProjectWorkspace>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn amend_project(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<StartBudgetAmendmentRequest>,
) -> Result<Json<ApiResponse<BudgetProjectWorkspace>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::projects::amend(&c.tenant.pool, &c.actor, id, input).await?;
    s.after_commit(&c.tenant.subdomain);
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/budget/requests",operation_id="listBudgetRequests",tag="budget",params(BudgetListQuery),responses((status=200,body=ApiResponse<BudgetRequestPage>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn list_requests(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(q): Query<BudgetListQuery>,
) -> Result<Json<ApiResponse<BudgetRequestPage>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::queries::requests(&c.tenant.pool, &c.actor, q).await?;
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/budget/projects/{id}/requests",operation_id="createBudgetSpendingRequest",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),request_body=SaveBudgetSpendingRequestRequest,responses((status=200,body=ApiResponse<BudgetSpendingRequestDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn create_request(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<SaveBudgetSpendingRequestRequest>,
) -> Result<Json<ApiResponse<BudgetSpendingRequestDetail>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::requests::create(&c.tenant.pool, &c.actor, id, input).await?;
    s.after_commit(&c.tenant.subdomain);
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/budget/requests/{id}",operation_id="getBudgetSpendingRequest",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),responses((status=200,body=ApiResponse<BudgetSpendingRequestDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn get_request(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<BudgetSpendingRequestDetail>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::queries::request_detail(&c.tenant.pool, &c.actor, id).await?;
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(put,path="/api/budget/requests/{id}",operation_id="saveBudgetSpendingRequest",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),request_body=SaveBudgetSpendingRequestRequest,responses((status=200,body=ApiResponse<BudgetSpendingRequestDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn save_request(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<SaveBudgetSpendingRequestRequest>,
) -> Result<Json<ApiResponse<BudgetSpendingRequestDetail>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::requests::save(&c.tenant.pool, &c.actor, id, input).await?;
    s.after_commit(&c.tenant.subdomain);
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/budget/requests/{id}/submit",operation_id="submitBudgetSpendingRequest",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),request_body=BudgetMutationRequest,responses((status=200,body=ApiResponse<BudgetSpendingRequestDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn submit_request(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<BudgetMutationRequest>,
) -> Result<Json<ApiResponse<BudgetSpendingRequestDetail>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::requests::submit(&c.tenant.pool, &c.actor, id, input).await?;
    s.after_commit(&c.tenant.subdomain);
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/budget/requests/{id}/actions",operation_id="actBudgetSpendingRequest",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),request_body=BudgetActionRequest,responses((status=200,body=ApiResponse<BudgetSpendingRequestDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn act_request(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<BudgetActionRequest>,
) -> Result<Json<ApiResponse<BudgetSpendingRequestDetail>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::requests::act(&c.tenant.pool, &c.actor, id, input).await?;
    s.after_commit(&c.tenant.subdomain);
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/budget/requests/{id}/close",operation_id="closeBudgetSpendingRequest",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),request_body=CloseBudgetRequestRequest,responses((status=200,body=ApiResponse<BudgetSpendingRequestDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn close_request(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<CloseBudgetRequestRequest>,
) -> Result<Json<ApiResponse<BudgetSpendingRequestDetail>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::requests::close(&c.tenant.pool, &c.actor, id, input).await?;
    s.after_commit(&c.tenant.subdomain);
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/budget/requests/{id}/payments",operation_id="listBudgetPayments",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID"),BudgetListQuery),responses((status=200,body=ApiResponse<BudgetPaymentPage>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn list_payments(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Query(q): Query<BudgetListQuery>,
) -> Result<Json<ApiResponse<BudgetPaymentPage>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::queries::payments(&c.tenant.pool, &c.actor, id, q).await?;
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/budget/requests/{id}/payments",operation_id="createBudgetPayment",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),request_body=CreateBudgetPaymentRequest,responses((status=200,body=ApiResponse<BudgetSpendingRequestDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn create_payment(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<CreateBudgetPaymentRequest>,
) -> Result<Json<ApiResponse<BudgetSpendingRequestDetail>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::requests::pay(&c.tenant.pool, &c.actor, id, input).await?;
    s.after_commit(&c.tenant.subdomain);
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/budget/adjustments",operation_id="listBudgetAdjustments",tag="budget",params(BudgetListQuery),responses((status=200,body=ApiResponse<BudgetAdjustmentPage>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn list_adjustments(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(q): Query<BudgetListQuery>,
) -> Result<Json<ApiResponse<BudgetAdjustmentPage>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::adjustments::list(&c.tenant.pool, &c.actor, q).await?;
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/budget/adjustments",operation_id="createBudgetAdjustment",tag="budget",request_body=SaveBudgetAdjustmentRequest,responses((status=200,body=ApiResponse<BudgetAdjustmentDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn create_adjustment(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(input): Json<SaveBudgetAdjustmentRequest>,
) -> Result<Json<ApiResponse<BudgetAdjustmentDetail>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::adjustments::create(&c.tenant.pool, &c.actor, input).await?;
    s.after_commit(&c.tenant.subdomain);
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/budget/adjustments/{id}",operation_id="getBudgetAdjustment",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),responses((status=200,body=ApiResponse<BudgetAdjustmentDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn get_adjustment(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<BudgetAdjustmentDetail>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::adjustments::detail(&c.tenant.pool, &c.actor, id).await?;
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(put,path="/api/budget/adjustments/{id}",operation_id="saveBudgetAdjustment",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),request_body=UpdateBudgetAdjustmentRequest,responses((status=200,body=ApiResponse<BudgetAdjustmentDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn save_adjustment(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateBudgetAdjustmentRequest>,
) -> Result<Json<ApiResponse<BudgetAdjustmentDetail>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::adjustments::save(&c.tenant.pool, &c.actor, id, input).await?;
    s.after_commit(&c.tenant.subdomain);
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/budget/adjustments/{id}/submit",operation_id="submitBudgetAdjustment",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),request_body=BudgetMutationRequest,responses((status=200,body=ApiResponse<BudgetAdjustmentDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn submit_adjustment(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<BudgetMutationRequest>,
) -> Result<Json<ApiResponse<BudgetAdjustmentDetail>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::adjustments::submit(&c.tenant.pool, &c.actor, id, input).await?;
    s.after_commit(&c.tenant.subdomain);
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/budget/adjustments/{id}/actions",operation_id="actBudgetAdjustment",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),request_body=BudgetActionRequest,responses((status=200,body=ApiResponse<BudgetAdjustmentDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn act_adjustment(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<BudgetActionRequest>,
) -> Result<Json<ApiResponse<BudgetAdjustmentDetail>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::adjustments::act(&c.tenant.pool, &c.actor, id, input).await?;
    s.after_commit(&c.tenant.subdomain);
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/budget/dashboard",operation_id="getBudgetDashboard",tag="budget",params(BudgetReportQuery),responses((status=200,body=ApiResponse<BudgetDashboard>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn get_dashboard(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(q): Query<BudgetReportQuery>,
) -> Result<Json<ApiResponse<BudgetDashboard>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::queries::dashboard(&c.tenant.pool, &c.actor, q, false).await?;
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/budget/projects/{id}/process",operation_id="getBudgetProjectProcess",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),responses((status=200,body=ApiResponse<BudgetProcessDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn project_process(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<BudgetProcessDetail>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data =
        crate::queries::resource_process(&c.tenant.pool, &c.actor, BudgetResourceKind::Project, id)
            .await?;
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/budget/projects/{id}/history",operation_id="getBudgetProjectHistory",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID"),BudgetListQuery),responses((status=200,body=ApiResponse<BudgetHistoryPage>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn project_history(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Query(q): Query<BudgetListQuery>,
) -> Result<Json<ApiResponse<BudgetHistoryPage>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data =
        crate::queries::history(&c.tenant.pool, &c.actor, BudgetResourceKind::Project, id, q)
            .await?;
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/budget/requests/{id}/process",operation_id="getBudgetRequestProcess",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),responses((status=200,body=ApiResponse<BudgetProcessDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn request_process(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<BudgetProcessDetail>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data =
        crate::queries::resource_process(&c.tenant.pool, &c.actor, BudgetResourceKind::Request, id)
            .await?;
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/budget/requests/{id}/history",operation_id="getBudgetRequestHistory",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID"),BudgetListQuery),responses((status=200,body=ApiResponse<BudgetHistoryPage>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn request_history(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Query(q): Query<BudgetListQuery>,
) -> Result<Json<ApiResponse<BudgetHistoryPage>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data =
        crate::queries::history(&c.tenant.pool, &c.actor, BudgetResourceKind::Request, id, q)
            .await?;
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/budget/adjustments/{id}/process",operation_id="getBudgetAdjustmentProcess",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID")),responses((status=200,body=ApiResponse<BudgetProcessDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn adjustment_process(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<BudgetProcessDetail>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::queries::resource_process(
        &c.tenant.pool,
        &c.actor,
        BudgetResourceKind::Adjustment,
        id,
    )
    .await?;
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/budget/adjustments/{id}/history",operation_id="getBudgetAdjustmentHistory",tag="budget",params(("id"=Uuid,Path,description="Budget resource ID"),BudgetListQuery),responses((status=200,body=ApiResponse<BudgetHistoryPage>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn adjustment_history(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Query(q): Query<BudgetListQuery>,
) -> Result<Json<ApiResponse<BudgetHistoryPage>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::queries::history(
        &c.tenant.pool,
        &c.actor,
        BudgetResourceKind::Adjustment,
        id,
        q,
    )
    .await?;
    Ok(Json(ApiResponse::ok(data)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/budget/requests/{id}/payments/{paymentId}/refunds",operation_id="refundBudgetPayment",tag="budget",params(("id"=Uuid,Path,description="Request ID"),("paymentId"=Uuid,Path,description="Payment ID")),request_body=RefundBudgetPaymentRequest,responses((status=200,body=ApiResponse<BudgetSpendingRequestDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse))))]
pub async fn refund_payment(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path((id, payment_id)): Path<(Uuid, Uuid)>,
    Json(input): Json<RefundBudgetPaymentRequest>,
) -> Result<Json<ApiResponse<BudgetSpendingRequestDetail>>, HttpError> {
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::requests::refund(&c.tenant.pool, &c.actor, id, payment_id, input).await?;
    s.after_commit(&c.tenant.subdomain);
    Ok(Json(ApiResponse::ok(data)))
}
#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/budget/reports/export",operation_id="exportBudgetReport",tag="budget",params(BudgetReportQuery),responses((status=200,description="Budget pivot CSV",content_type="text/csv",body=String),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse))))]
pub async fn export_report(
    State(s): State<BudgetHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(q): Query<BudgetReportQuery>,
) -> Result<axum::response::Response, HttpError> {
    use axum::response::IntoResponse;
    let c = actor_tenant_context_from_session(&s.auth_runtime, &session).await?;
    let data = crate::queries::export_csv(&c.tenant.pool, &c.actor, q).await?;
    Ok((
        [
            (axum::http::header::CONTENT_TYPE, "text/csv; charset=utf-8"),
            (
                axum::http::header::CONTENT_DISPOSITION,
                "attachment; filename=budget-report.csv",
            ),
        ],
        data,
    )
        .into_response())
}
pub fn routes() -> Router<BudgetHttpState> {
    Router::new()
        .route("/budget/catalogs", axum::routing::get(get_catalogs))
        .route(
            "/budget/settings",
            axum::routing::get(get_settings).put(save_settings),
        )
        .route(
            "/budget/projects",
            axum::routing::get(list_projects).post(create_project),
        )
        .route(
            "/budget/projects/{id}",
            axum::routing::get(get_project).put(save_project),
        )
        .route(
            "/budget/projects/{id}/submit",
            axum::routing::post(submit_project),
        )
        .route(
            "/budget/projects/{id}/actions",
            axum::routing::post(act_project),
        )
        .route(
            "/budget/projects/{id}/amendments",
            axum::routing::post(amend_project),
        )
        .route("/budget/requests", axum::routing::get(list_requests))
        .route(
            "/budget/projects/{id}/requests",
            axum::routing::post(create_request),
        )
        .route(
            "/budget/requests/{id}",
            axum::routing::get(get_request).put(save_request),
        )
        .route(
            "/budget/requests/{id}/submit",
            axum::routing::post(submit_request),
        )
        .route(
            "/budget/requests/{id}/actions",
            axum::routing::post(act_request),
        )
        .route(
            "/budget/requests/{id}/close",
            axum::routing::post(close_request),
        )
        .route(
            "/budget/requests/{id}/payments",
            axum::routing::get(list_payments).post(create_payment),
        )
        .route(
            "/budget/adjustments",
            axum::routing::get(list_adjustments).post(create_adjustment),
        )
        .route(
            "/budget/adjustments/{id}",
            axum::routing::get(get_adjustment).put(save_adjustment),
        )
        .route(
            "/budget/adjustments/{id}/submit",
            axum::routing::post(submit_adjustment),
        )
        .route(
            "/budget/adjustments/{id}/actions",
            axum::routing::post(act_adjustment),
        )
        .route("/budget/dashboard", axum::routing::get(get_dashboard))
        .route(
            "/budget/projects/{id}/process",
            axum::routing::get(project_process),
        )
        .route(
            "/budget/projects/{id}/history",
            axum::routing::get(project_history),
        )
        .route(
            "/budget/requests/{id}/process",
            axum::routing::get(request_process),
        )
        .route(
            "/budget/requests/{id}/history",
            axum::routing::get(request_history),
        )
        .route(
            "/budget/adjustments/{id}/process",
            axum::routing::get(adjustment_process),
        )
        .route(
            "/budget/adjustments/{id}/history",
            axum::routing::get(adjustment_history),
        )
        .route(
            "/budget/requests/{id}/payments/{paymentId}/refunds",
            axum::routing::post(refund_payment),
        )
        .route("/budget/reports/export", axum::routing::get(export_report))
}

#[cfg(feature = "api-docs")]
#[derive(utoipa::OpenApi)]
#[openapi(paths(get_catalogs,get_settings,save_settings,list_projects,create_project,get_project,save_project,submit_project,act_project,amend_project,list_requests,create_request,get_request,save_request,submit_request,act_request,close_request,list_payments,create_payment,list_adjustments,create_adjustment,get_adjustment,save_adjustment,submit_adjustment,act_adjustment,get_dashboard,project_process,project_history,request_process,request_history,adjustment_process,adjustment_history,refund_payment,export_report),components(schemas(BudgetStatus,BudgetAdjustmentKind,BudgetResourceKind,BudgetCatalogItem,BudgetFiscalYear,BudgetCatalogs,BudgetSettings,SaveBudgetSettingsRequest,BudgetProjectInput,CreateBudgetProjectRequest,SaveBudgetProjectRequest,BudgetActivityInput,BudgetPlanLineInput,BudgetActivity,BudgetPlanLine,BudgetPlanSnapshot,BudgetProjectSummary,BudgetCapabilities,BudgetAllocationRef,BudgetBalance,BudgetProjectWorkspace,BudgetProjectPage,BudgetMutationRequest,BudgetActionRequest,StartBudgetAmendmentRequest,BudgetRequestLineInput,BudgetSpendingRequestInput,SaveBudgetSpendingRequestRequest,BudgetRequestSummary,BudgetSpendingRequestDetail,BudgetRequestPage,BudgetPaymentAllocationInput,CreateBudgetPaymentRequest,RefundBudgetPaymentRequest,BudgetFileSnapshot,BudgetRefund,BudgetPayment,BudgetPaymentPage,CloseBudgetRequestRequest,BudgetAdjustmentInput,SaveBudgetAdjustmentRequest,BudgetAdjustmentDetail,BudgetAdjustmentPage,BudgetHistoryEntry,BudgetHistoryPage,BudgetProcessDetail,BudgetDashboardRow,BudgetDashboardTotals,BudgetDashboard,BudgetListQuery,BudgetReportQuery,BudgetRequestLine,UpdateBudgetAdjustmentRequest,BudgetCatalogItemInput,BudgetFiscalYearInput,BudgetReportMetric,ApiResponse<BudgetAdjustmentDetail>,ApiResponse<BudgetAdjustmentPage>,ApiResponse<BudgetCatalogs>,ApiResponse<BudgetDashboard>,ApiResponse<BudgetHistoryPage>,ApiResponse<BudgetPaymentPage>,ApiResponse<BudgetProcessDetail>,ApiResponse<BudgetProjectPage>,ApiResponse<BudgetProjectWorkspace>,ApiResponse<BudgetRequestPage>,ApiResponse<BudgetSettings>,ApiResponse<BudgetSpendingRequestDetail>)))]
pub struct BudgetApi;

#[cfg(all(test, feature = "api-docs"))]
mod contract_tests {
    use super::BudgetApi;
    use utoipa::OpenApi;

    #[test]
    fn budget_query_contract_matches_registered_extractors() {
        let document = serde_json::to_value(BudgetApi::openapi()).unwrap();
        let paths = document["paths"].as_object().unwrap();
        let list_filters = [
            "fiscalYearId",
            "projectId",
            "activityId",
            "organizationUnitId",
            "status",
            "search",
            "limit",
            "offset",
        ];
        let report_filters = [
            "exportScope",
            "metric",
            "fiscalYearId",
            "projectId",
            "organizationUnitId",
        ];
        for path in [
            "/api/budget/projects",
            "/api/budget/requests",
            "/api/budget/adjustments",
            "/api/budget/projects/{id}/history",
            "/api/budget/requests/{id}/history",
            "/api/budget/adjustments/{id}/history",
            "/api/budget/requests/{id}/payments",
            "/api/budget/dashboard",
            "/api/budget/reports/export",
        ] {
            let parameters = paths[path]["get"]["parameters"].as_array().unwrap();
            let expected = if path.ends_with("dashboard") || path.ends_with("export") {
                report_filters.as_slice()
            } else {
                list_filters.as_slice()
            };
            for name in expected {
                let parameter = parameters
                    .iter()
                    .find(|p| p["name"] == *name)
                    .unwrap_or_else(|| panic!("missing {name} parameter on GET {path}"));
                assert_eq!(parameter["in"], "query", "GET {path}: {name}");
                assert_eq!(parameter["required"], false, "GET {path}: optional {name}");
            }
        }
        // Actual path parameters must occur in the URI. Optional query filters may
        // never become synthetic required path arguments in the generated client.
        for (path, item) in paths {
            let Some(parameters) = item["get"]["parameters"].as_array() else {
                continue;
            };
            for parameter in parameters {
                let name = parameter["name"].as_str().unwrap();
                match parameter["in"].as_str().unwrap() {
                    "path" => {
                        assert!(
                            path.contains(&format!("{{{name}}}")),
                            "GET {path}: phantom {name}"
                        );
                        assert_eq!(parameter["required"], true);
                    }
                    "query" => assert_eq!(parameter["required"], false, "GET {path}: {name}"),
                    other => panic!("unexpected parameter location {other} on GET {path}"),
                }
            }
        }
    }
}
