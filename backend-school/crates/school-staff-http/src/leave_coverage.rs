//! Redacted coverage transport under the existing authenticated Staff route boundary.
use axum::{extract::{Extension,Path,Query,State},Json,Router};
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
use school_http::{ApiResponse,HttpError};
#[cfg(feature="api-docs")] use school_http::ApiErrorResponse;
use school_staff_leave::{coverage,coverage_models::*,models::{LeaveMutationRequest,LeaveRequestDetail}};
use super::leave::LeaveHttpState;
use uuid::Uuid;

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/staff-leave/coverage/settings",operation_id="getStaffLeaveCoverageSettings",tag="staff-leave",responses((status=200,description="Teaching coverage result",body=ApiResponse<LeaveCoverageSettings>),(status=400,description="Invalid coverage",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission denied",body=ApiErrorResponse),(status=409,description="Stale coverage or conflicting duty",body=ApiErrorResponse))))]
pub async fn settings(State(state):State<LeaveHttpState>,Extension(session):Extension<AuthenticatedSession>)->Result<Json<ApiResponse<LeaveCoverageSettings>>,HttpError>{
 let context=actor_tenant_context_from_session(&state.auth_runtime,&session).await?;
 let result=coverage::settings(&context.tenant.pool,&context.actor).await?;
 Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(put,path="/api/staff-leave/coverage/settings",operation_id="saveStaffLeaveCoverageSettings",tag="staff-leave",request_body=SaveLeaveCoverageSettingsRequest,responses((status=200,description="Teaching coverage result",body=ApiResponse<LeaveCoverageSettings>),(status=400,description="Invalid coverage",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission denied",body=ApiErrorResponse),(status=409,description="Stale coverage or conflicting duty",body=ApiErrorResponse))))]
pub async fn save_settings(State(state):State<LeaveHttpState>,Extension(session):Extension<AuthenticatedSession>,Json(input):Json<SaveLeaveCoverageSettingsRequest>)->Result<Json<ApiResponse<LeaveCoverageSettings>>,HttpError>{
 let context=actor_tenant_context_from_session(&state.auth_runtime,&session).await?;
 let result=coverage::save_settings(&context.tenant.pool,&context.actor,input).await?;
 Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/staff-leave/coverage/preview",operation_id="previewStaffLeaveCoverage",tag="staff-leave",request_body=PreviewLeaveCoverageRequest,responses((status=200,description="Teaching coverage result",body=ApiResponse<LeaveCoveragePlan>),(status=400,description="Invalid coverage",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission denied",body=ApiErrorResponse),(status=409,description="Stale coverage or conflicting duty",body=ApiErrorResponse))))]
pub async fn preview(State(state):State<LeaveHttpState>,Extension(session):Extension<AuthenticatedSession>,Json(input):Json<PreviewLeaveCoverageRequest>)->Result<Json<ApiResponse<LeaveCoveragePlan>>,HttpError>{
 let context=actor_tenant_context_from_session(&state.auth_runtime,&session).await?;
 let result=coverage::preview(&context.tenant.pool,&context.actor,input).await?;
 Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/staff-leave/requests/{id}/coverage/candidates",operation_id="getStaffLeaveCoverageCandidates",tag="staff-leave",params(("id"=Uuid,Path,description="Leave request identity")),request_body=LeaveCoverageCandidatesRequest,responses((status=200,description="Teaching coverage result",body=ApiResponse<LeaveCoverageCandidates>),(status=400,description="Invalid coverage",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission denied",body=ApiErrorResponse),(status=409,description="Stale coverage or conflicting duty",body=ApiErrorResponse))))]
pub async fn candidates(State(state):State<LeaveHttpState>,Extension(session):Extension<AuthenticatedSession>,Path(id):Path<Uuid>,Json(input):Json<LeaveCoverageCandidatesRequest>)->Result<Json<ApiResponse<LeaveCoverageCandidates>>,HttpError>{
 let context=actor_tenant_context_from_session(&state.auth_runtime,&session).await?;
 let result=coverage::candidates(&context.tenant.pool,&context.actor,id,input).await?;
 Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/staff-leave/requests/{id}/coverage/return-lessons",operation_id="getStaffLeaveCoverageReturnLessons",tag="staff-leave",params(("id"=Uuid,Path,description="Leave request identity")),request_body=LeaveCoverageReturnLessonsRequest,responses((status=200,description="Teaching coverage result",body=ApiResponse<LeaveCoverageReturnLessons>),(status=400,description="Invalid coverage",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission denied",body=ApiErrorResponse),(status=409,description="Stale coverage or conflicting duty",body=ApiErrorResponse))))]
pub async fn return_lessons(State(state):State<LeaveHttpState>,Extension(session):Extension<AuthenticatedSession>,Path(id):Path<Uuid>,Json(input):Json<LeaveCoverageReturnLessonsRequest>)->Result<Json<ApiResponse<LeaveCoverageReturnLessons>>,HttpError>{
 let context=actor_tenant_context_from_session(&state.auth_runtime,&session).await?;
 let result=coverage::return_lessons(&context.tenant.pool,&context.actor,id,input).await?;
 Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(put,path="/api/staff-leave/requests/{id}/coverage",operation_id="saveStaffLeaveCoverage",tag="staff-leave",params(("id"=Uuid,Path,description="Leave request identity")),request_body=SaveLeaveCoverageRequest,responses((status=200,description="Teaching coverage result",body=ApiResponse<LeaveRequestDetail>),(status=400,description="Invalid coverage",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission denied",body=ApiErrorResponse),(status=409,description="Stale coverage or conflicting duty",body=ApiErrorResponse))))]
pub async fn save(State(state):State<LeaveHttpState>,Extension(session):Extension<AuthenticatedSession>,Path(id):Path<Uuid>,Json(input):Json<SaveLeaveCoverageRequest>)->Result<Json<ApiResponse<LeaveRequestDetail>>,HttpError>{
 let context=actor_tenant_context_from_session(&state.auth_runtime,&session).await?;
 let result=coverage::save(&context.tenant.pool,&context.actor,id,input).await?;
 Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/staff-leave/requests/{id}/coverage/confirm",operation_id="confirmStaffLeaveCoverage",tag="staff-leave",params(("id"=Uuid,Path,description="Leave request identity")),request_body=ConfirmLeaveCoverageRequest,responses((status=200,description="Teaching coverage result",body=ApiResponse<LeaveCoverageTask>),(status=400,description="Invalid coverage",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission denied",body=ApiErrorResponse),(status=409,description="Stale coverage or conflicting duty",body=ApiErrorResponse))))]
pub async fn confirm(State(state):State<LeaveHttpState>,Extension(session):Extension<AuthenticatedSession>,Path(id):Path<Uuid>,Json(input):Json<ConfirmLeaveCoverageRequest>)->Result<Json<ApiResponse<LeaveCoverageTask>>,HttpError>{
 let context=actor_tenant_context_from_session(&state.auth_runtime,&session).await?;
 let result=coverage::confirm(&context.tenant.pool,&context.actor,id,input).await?;
 Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/staff-leave/requests/{id}/coverage/replan",operation_id="replanStaffLeaveCoverage",tag="staff-leave",params(("id"=Uuid,Path,description="Leave request identity")),request_body=LeaveMutationRequest,responses((status=200,description="Teaching coverage result",body=ApiResponse<LeaveRequestDetail>),(status=400,description="Invalid coverage",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission denied",body=ApiErrorResponse),(status=409,description="Stale coverage or conflicting duty",body=ApiErrorResponse))))]
pub async fn replan(State(state):State<LeaveHttpState>,Extension(session):Extension<AuthenticatedSession>,Path(id):Path<Uuid>,Json(input):Json<LeaveMutationRequest>)->Result<Json<ApiResponse<LeaveRequestDetail>>,HttpError>{
 let context=actor_tenant_context_from_session(&state.auth_runtime,&session).await?;
 let result=coverage::replan(&context.tenant.pool,&context.actor,id,input).await?;
 Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/staff-leave/coverage/assigned",operation_id="listAssignedStaffLeaveCoverage",tag="staff-leave",params(LeaveCoverageTaskQuery),responses((status=200,description="Teaching coverage result",body=ApiResponse<LeaveCoverageTaskPage>),(status=400,description="Invalid coverage",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Permission denied",body=ApiErrorResponse),(status=409,description="Stale coverage or conflicting duty",body=ApiErrorResponse))))]
pub async fn assigned(State(state):State<LeaveHttpState>,Extension(session):Extension<AuthenticatedSession>,Query(query):Query<LeaveCoverageTaskQuery>)->Result<Json<ApiResponse<LeaveCoverageTaskPage>>,HttpError>{
 let context=actor_tenant_context_from_session(&state.auth_runtime,&session).await?;
 let result=coverage::assigned(&context.tenant.pool,&context.actor,query).await?;
 Ok(Json(ApiResponse::ok(result)))
}

pub fn routes()->Router<LeaveHttpState>{Router::new()
.route("/staff-leave/coverage/settings",axum::routing::get(settings).put(save_settings))
.route("/staff-leave/coverage/preview",axum::routing::post(preview))
.route("/staff-leave/requests/{id}/coverage/candidates",axum::routing::post(candidates))
.route("/staff-leave/requests/{id}/coverage/return-lessons",axum::routing::post(return_lessons))
.route("/staff-leave/requests/{id}/coverage",axum::routing::put(save))
.route("/staff-leave/requests/{id}/coverage/confirm",axum::routing::post(confirm))
.route("/staff-leave/requests/{id}/coverage/replan",axum::routing::post(replan))
.route("/staff-leave/coverage/assigned",axum::routing::get(assigned))
}
