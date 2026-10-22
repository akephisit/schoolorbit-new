//! Thin authenticated transport for Staff-owned leave workflows.
use super::{models::*, service};
use axum::{
    extract::{Extension, Path, Query, State},
    Json, Router,
};
use school_auth::{runtime::AuthRuntime, session_service::AuthenticatedSession};
use school_auth_http::context::actor_tenant_context_from_session;
#[cfg(feature = "api-docs")]
use school_http::ApiErrorResponse;
use school_http::{ApiResponse, EmptyData, HttpError};
use uuid::Uuid;
#[derive(Clone)]
pub struct LeaveHttpState {
    pub auth_runtime: AuthRuntime,
    pub work_event_channel:
        tokio::sync::broadcast::Sender<school_notifications::events::WorkChangeEvent>,
}
impl LeaveHttpState {
    fn notify(&self, tenant: &str) {
        if let Err(error) = self
            .work_event_channel
            .send(school_notifications::events::WorkChangeEvent::work_items_changed(tenant))
        {
            tracing::debug!(%error,"staff_leave_work_signal_without_subscribers");
        }
    }
}
#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/staff-leave/types",operation_id="listStaffLeaveTypes",tag="staff-leave",params(LeaveListQuery),responses((status=200, description = "Staff leave types returned",body=ApiResponse<Vec<LeaveType>>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn list_types(
    State(state): State<LeaveHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<LeaveListQuery>,
) -> Result<Json<ApiResponse<Vec<LeaveType>>>, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = service::list_types(&context.tenant.pool, &context.actor, query).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/staff-leave/types",operation_id="createStaffLeaveType",tag="staff-leave",request_body=SaveLeaveTypeRequest,responses((status=200, description = "Staff leave type created",body=ApiResponse<LeaveType>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn create_type(
    State(state): State<LeaveHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(input): Json<SaveLeaveTypeRequest>,
) -> Result<Json<ApiResponse<LeaveType>>, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = service::save_type(&context.tenant.pool, &context.actor, None, input).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature="api-docs",utoipa::path(put,path="/api/staff-leave/types/{id}",operation_id="updateStaffLeaveType",tag="staff-leave",params(("id"=Uuid,Path,description="Staff leave resource ID")),request_body=SaveLeaveTypeRequest,responses((status=200, description = "Staff leave type updated",body=ApiResponse<LeaveType>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn update_type(
    State(state): State<LeaveHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<SaveLeaveTypeRequest>,
) -> Result<Json<ApiResponse<LeaveType>>, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = service::save_type(&context.tenant.pool, &context.actor, Some(id), input).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature="api-docs",utoipa::path(delete,path="/api/staff-leave/types/{id}",operation_id="deleteStaffLeaveType",tag="staff-leave",params(("id"=Uuid,Path,description="Staff leave resource ID")),request_body=LeaveMutationRequest,responses((status=200, description = "Staff leave type removed",body=ApiResponse<EmptyData>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn delete_type(
    State(state): State<LeaveHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<LeaveMutationRequest>,
) -> Result<Json<ApiResponse<EmptyData>>, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    service::delete_type(&context.tenant.pool, &context.actor, id, input).await?;
    Ok(Json(ApiResponse::empty()))
}
#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/staff-leave/types/{id}/policies",operation_id="listStaffLeavePolicies",tag="staff-leave",params(("id"=Uuid,Path,description="Staff leave resource ID")),responses((status=200, description = "Staff leave policy versions returned",body=ApiResponse<Vec<LeavePolicyVersion>>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn list_policies(
    State(state): State<LeaveHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<Vec<LeavePolicyVersion>>>, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = service::policies(&context.tenant.pool, &context.actor, id).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/staff-leave/types/{id}/policies",operation_id="createStaffLeavePolicy",tag="staff-leave",params(("id"=Uuid,Path,description="Staff leave resource ID")),request_body=SaveLeavePolicyRequest,responses((status=200, description = "Staff leave policy version created",body=ApiResponse<LeavePolicyVersion>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn create_policy(
    State(state): State<LeaveHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<SaveLeavePolicyRequest>,
) -> Result<Json<ApiResponse<LeavePolicyVersion>>, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = service::save_policy(&context.tenant.pool, &context.actor, id, input).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/staff-leave/types/{id}/effective-policy",operation_id="getStaffLeaveEffectivePolicy",tag="staff-leave",params(("id"=Uuid,Path,description="Staff leave resource ID"),LeaveEffectivePolicyQuery),responses((status=200, description = "Effective staff leave policy returned",body=ApiResponse<LeavePolicyVersion>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn effective_policy(
    State(state): State<LeaveHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Query(query): Query<LeaveEffectivePolicyQuery>,
) -> Result<Json<ApiResponse<LeavePolicyVersion>>, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result =
        service::effective_policy(&context.tenant.pool, &context.actor, id, query.start_date)
            .await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/staff-leave/options",operation_id="getStaffLeaveOptions",tag="staff-leave",responses((status=200, description = "Personal staff leave options returned",body=ApiResponse<LeavePersonalOptions>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn options(
    State(state): State<LeaveHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Json<ApiResponse<LeavePersonalOptions>>, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = service::options(&context.tenant.pool, &context.actor).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/staff-leave/routing-options",operation_id="getStaffLeaveRoutingOptions",tag="staff-leave",responses((status=200, description = "Staff leave routing options returned",body=ApiResponse<LeaveRoutingOptions>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn routing_options(
    State(state): State<LeaveHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Json<ApiResponse<LeaveRoutingOptions>>, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = service::routing_options(&context.tenant.pool, &context.actor).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/staff-leave/requests",operation_id="listStaffLeaveRequests",tag="staff-leave",params(LeaveListQuery),responses((status=200, description = "Staff leave requests returned",body=ApiResponse<LeaveRequestPage>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn list_requests(
    State(state): State<LeaveHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<LeaveListQuery>,
) -> Result<Json<ApiResponse<LeaveRequestPage>>, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = service::list(&context.tenant.pool, &context.actor, query).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/staff-leave/requests",operation_id="createStaffLeaveRequest",tag="staff-leave",request_body=CreateLeaveRequest,responses((status=200, description = "Staff leave request created",body=ApiResponse<LeaveRequestDetail>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn create_request(
    State(state): State<LeaveHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(input): Json<CreateLeaveRequest>,
) -> Result<Json<ApiResponse<LeaveRequestDetail>>, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = service::create(&context.tenant.pool, &context.actor, input).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/staff-leave/requests/{id}",operation_id="getStaffLeaveRequest",tag="staff-leave",params(("id"=Uuid,Path,description="Staff leave resource ID")),responses((status=200, description = "Staff leave request detail returned",body=ApiResponse<LeaveRequestDetail>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn get_request(
    State(state): State<LeaveHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Json<ApiResponse<LeaveRequestDetail>>, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = service::detail(&context.tenant.pool, &context.actor, id).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature="api-docs",utoipa::path(put,path="/api/staff-leave/requests/{id}",operation_id="updateStaffLeaveRequest",tag="staff-leave",params(("id"=Uuid,Path,description="Staff leave resource ID")),request_body=UpdateLeaveRequest,responses((status=200, description = "Staff leave request updated",body=ApiResponse<LeaveRequestDetail>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn update_request(
    State(state): State<LeaveHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateLeaveRequest>,
) -> Result<Json<ApiResponse<LeaveRequestDetail>>, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = service::update(&context.tenant.pool, &context.actor, id, input).await?;
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/staff-leave/requests/{id}/submit",operation_id="submitStaffLeaveRequest",tag="staff-leave",params(("id"=Uuid,Path,description="Staff leave resource ID")),request_body=LeaveMutationRequest,responses((status=200, description = "Staff leave request submission result returned",body=ApiResponse<LeaveRequestDetail>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn submit_request(
    State(state): State<LeaveHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<LeaveMutationRequest>,
) -> Result<Json<ApiResponse<LeaveRequestDetail>>, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = service::submit(&context.tenant.pool, &context.actor, id, input).await?;
    state.notify(&context.tenant.subdomain);
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/staff-leave/requests/{id}/actions",operation_id="actStaffLeaveRequest",tag="staff-leave",params(("id"=Uuid,Path,description="Staff leave resource ID")),request_body=LeaveActionRequest,responses((status=200, description = "Staff leave request action result returned",body=ApiResponse<LeaveRequestDetail>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn act_request(
    State(state): State<LeaveHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<LeaveActionRequest>,
) -> Result<Json<ApiResponse<LeaveRequestDetail>>, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = service::act(&context.tenant.pool, &context.actor, id, input).await?;
    state.notify(&context.tenant.subdomain);
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/staff-leave/requests/{id}/cancel",operation_id="cancelStaffLeaveRequest",tag="staff-leave",params(("id"=Uuid,Path,description="Staff leave resource ID")),request_body=CancelLeaveRequest,responses((status=200, description = "Staff leave cancellation request result returned",body=ApiResponse<LeaveRequestDetail>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn cancel_request(
    State(state): State<LeaveHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(input): Json<CancelLeaveRequest>,
) -> Result<Json<ApiResponse<LeaveRequestDetail>>, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = service::cancel(&context.tenant.pool, &context.actor, id, input).await?;
    state.notify(&context.tenant.subdomain);
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)))
}
#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/staff-leave/usage",operation_id="getStaffLeaveUsage",tag="staff-leave",params(LeaveUsageQuery),responses((status=200, description = "Staff leave usage report returned",body=ApiResponse<LeaveUsageReport>),(status=400, description = "Invalid input",body=ApiErrorResponse),(status=401, description = "Authentication required",body=ApiErrorResponse),(status=403, description = "Permission denied",body=ApiErrorResponse),(status=404, description = "Resource not found",body=ApiErrorResponse),(status=409, description = "Request conflicts with the current resource state",body=ApiErrorResponse))))]
pub async fn usage(
    State(state): State<LeaveHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<LeaveUsageQuery>,
) -> Result<Json<ApiResponse<LeaveUsageReport>>, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let result = service::usage(&context.tenant.pool, &context.actor, query).await?;
    Ok(Json(ApiResponse::ok(result)))
}
pub fn routes() -> Router<LeaveHttpState> {
    Router::new()
        .route(
            "/staff-leave/types",
            axum::routing::get(list_types).post(create_type),
        )
        .route(
            "/staff-leave/types/{id}",
            axum::routing::put(update_type).delete(delete_type),
        )
        .route(
            "/staff-leave/types/{id}/policies",
            axum::routing::get(list_policies).post(create_policy),
        )
        .route(
            "/staff-leave/types/{id}/effective-policy",
            axum::routing::get(effective_policy),
        )
        .route("/staff-leave/options", axum::routing::get(options))
        .route(
            "/staff-leave/routing-options",
            axum::routing::get(routing_options),
        )
        .route(
            "/staff-leave/requests",
            axum::routing::get(list_requests).post(create_request),
        )
        .route(
            "/staff-leave/requests/{id}",
            axum::routing::get(get_request).put(update_request),
        )
        .route(
            "/staff-leave/requests/{id}/submit",
            axum::routing::post(submit_request),
        )
        .route(
            "/staff-leave/requests/{id}/actions",
            axum::routing::post(act_request),
        )
        .route(
            "/staff-leave/requests/{id}/cancel",
            axum::routing::post(cancel_request),
        )
        .route("/staff-leave/usage", axum::routing::get(usage))
}
