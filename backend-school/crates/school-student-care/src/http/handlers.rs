use super::state::StudentCareHttpState;
use crate::{models::*, policy::CareAccess, services};
use axum::{
    extract::{Extension, Path, Query, State},
    response::IntoResponse,
    Json,
};
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
#[cfg(feature = "api-docs")]
use school_http::ApiErrorResponse;
use school_http::{ApiResponse, HttpError};
use uuid::Uuid;

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/student-care/overview",operation_id="student_care_overview",tag="student-care",responses((status=200,body=ApiResponse<CareOverview>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn overview(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::overview(&context.tenant.pool, a).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/student-care/instruments",operation_id="student_care_list_instruments",tag="student-care",params(CareListQuery),responses((status=200,body=ApiResponse<Vec<CareInstrumentVersion>>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn list_instruments(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(p): Query<CareListQuery>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::instruments(&context.tenant.pool, a, p).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/student-care/instruments/{id}",operation_id="student_care_get_instrument",tag="student-care",params(("id"=Uuid,Path)),responses((status=200,body=ApiResponse<CareInstrumentVersion>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn get_instrument(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::instrument(&context.tenant.pool, a, id).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/student-care/instruments",operation_id="student_care_create_instrument",tag="student-care",request_body=CreateCareInstrumentVersion,responses((status=200,body=ApiResponse<CareInstrumentVersion>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn create_instrument(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(p): Json<CreateCareInstrumentVersion>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::create_instrument(&context.tenant.pool, a, p).await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/student-care/rounds/{id}",operation_id="student_care_get_round",tag="student-care",params(("id"=Uuid,Path)),responses((status=200,body=ApiResponse<CareRound>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn get_round(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::get_round(&context.tenant.pool, a, id).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/student-care/rounds",operation_id="student_care_list_rounds",tag="student-care",params(CareListQuery),responses((status=200,body=ApiResponse<Vec<CareRound>>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn list_rounds(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(p): Query<CareListQuery>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::rounds(&context.tenant.pool, a, p).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/student-care/rounds",operation_id="student_care_create_round",tag="student-care",request_body=CreateCareRound,responses((status=200,body=ApiResponse<CareRound>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn create_round(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(p): Json<CreateCareRound>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::create_round(&context.tenant.pool, a, p).await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(put,path="/api/student-care/rounds/{id}",operation_id="student_care_update_round",tag="student-care",params(("id"=Uuid,Path)),request_body=UpdateCareRound,responses((status=200,body=ApiResponse<CareRound>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn update_round(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(p): Json<UpdateCareRound>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::update_round(&context.tenant.pool, a, id, p).await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/student-care/students",operation_id="student_care_list_students",tag="student-care",params(CareStudentQuery),responses((status=200,body=ApiResponse<Vec<CareStudentOption>>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn list_students(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(p): Query<CareStudentQuery>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::students(&context.tenant.pool, a, p).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/student-care/staff",operation_id="student_care_list_staff",tag="student-care",responses((status=200,body=ApiResponse<Vec<CareStaffOption>>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn list_staff(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::staff(&context.tenant.pool, a).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/student-care/assignments",operation_id="student_care_list_assignments",tag="student-care",params(CareListQuery),responses((status=200,body=ApiResponse<Vec<CareAssignment>>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn list_assignments(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(p): Query<CareListQuery>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::assignments(&context.tenant.pool, a, p).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/student-care/assignments",operation_id="student_care_create_assignments",tag="student-care",request_body=CreateCareAssignments,responses((status=200,body=ApiResponse<Vec<CareAssignment>>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn create_assignments(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(p): Json<CreateCareAssignments>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::create_assignments(&context.tenant.pool, a, p).await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/student-care/assignments/{id}",operation_id="student_care_get_assignment",tag="student-care",params(("id"=Uuid,Path)),responses((status=200,body=ApiResponse<CareAssignmentDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn get_assignment(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::get_assignment(&context.tenant.pool, a, id).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(put,path="/api/student-care/assignments/{id}/draft",operation_id="student_care_save_draft",tag="student-care",params(("id"=Uuid,Path)),request_body=SaveCareResponse,responses((status=200,body=ApiResponse<CareAssignmentDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn save_draft(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(p): Json<SaveCareResponse>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::save_response(&context.tenant.pool, a, id, p, false).await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/student-care/assignments/{id}/submit",operation_id="student_care_submit_response",tag="student-care",params(("id"=Uuid,Path)),request_body=SaveCareResponse,responses((status=200,body=ApiResponse<CareAssignmentDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn submit_response(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(p): Json<SaveCareResponse>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::save_response(&context.tenant.pool, a, id, p, true).await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/student-care/assignments/{id}/cancel",operation_id="student_care_cancel_assignment",tag="student-care",params(("id"=Uuid,Path)),request_body=CareRevisionCommand,responses((status=200,body=ApiResponse<CareAssignment>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn cancel_assignment(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(p): Json<CareRevisionCommand>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::assignment_command(&context.tenant.pool, a, id, p, false).await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/student-care/assignments/{id}/acknowledge",operation_id="student_care_acknowledge_assignment",tag="student-care",params(("id"=Uuid,Path)),request_body=CareRevisionCommand,responses((status=200,body=ApiResponse<CareAssignment>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn acknowledge_assignment(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(p): Json<CareRevisionCommand>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::assignment_command(&context.tenant.pool, a, id, p, true).await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/student-care/visits",operation_id="student_care_list_visits",tag="student-care",params(CareListQuery),responses((status=200,body=ApiResponse<Vec<CareVisit>>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn list_visits(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(p): Query<CareListQuery>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::visits(&context.tenant.pool, a, p).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/student-care/visits",operation_id="student_care_create_visit",tag="student-care",request_body=SaveCareVisit,responses((status=200,body=ApiResponse<CareVisitRecord>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn create_visit(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(p): Json<SaveCareVisit>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::save_visit(&context.tenant.pool, a, None, p).await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/student-care/visits/{id}",operation_id="student_care_get_visit",tag="student-care",params(("id"=Uuid,Path)),responses((status=200,body=ApiResponse<CareVisitRecord>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn get_visit(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::get_visit(&context.tenant.pool, a, id).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(put,path="/api/student-care/visits/{id}",operation_id="student_care_update_visit",tag="student-care",params(("id"=Uuid,Path)),request_body=SaveCareVisit,responses((status=200,body=ApiResponse<CareVisitRecord>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn update_visit(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(p): Json<SaveCareVisit>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::save_visit(&context.tenant.pool, a, Some(id), p).await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/student-care/cases/{id}/events",operation_id="student_care_list_case_events",tag="student-care",params(("id"=Uuid,Path),CareListQuery),responses((status=200,body=ApiResponse<Vec<CareCaseEvent>>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn list_case_events(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Query(p): Query<CareListQuery>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::list_case_events(&context.tenant.pool, a, id, p).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/student-care/cases",operation_id="student_care_list_cases",tag="student-care",params(CareListQuery),responses((status=200,body=ApiResponse<Vec<CareCase>>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn list_cases(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(p): Query<CareListQuery>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::cases(&context.tenant.pool, a, p).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/student-care/cases",operation_id="student_care_create_case",tag="student-care",request_body=SaveCareCase,responses((status=200,body=ApiResponse<CareCaseDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn create_case(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(p): Json<SaveCareCase>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::save_case(&context.tenant.pool, a, None, p).await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(get,path="/api/student-care/cases/{id}",operation_id="student_care_get_case",tag="student-care",params(("id"=Uuid,Path)),responses((status=200,body=ApiResponse<CareCaseDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn get_case(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::get_case(&context.tenant.pool, a, id).await?;
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(put,path="/api/student-care/cases/{id}",operation_id="student_care_update_case",tag="student-care",params(("id"=Uuid,Path)),request_body=SaveCareCase,responses((status=200,body=ApiResponse<CareCaseDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn update_case(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(p): Json<SaveCareCase>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::save_case(&context.tenant.pool, a, Some(id), p).await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/student-care/cases/{id}/events",operation_id="student_care_add_case_event",tag="student-care",params(("id"=Uuid,Path)),request_body=AddCareCaseEvent,responses((status=200,body=ApiResponse<CareCaseDetail>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn add_case_event(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(p): Json<AddCareCaseEvent>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::add_case_event(&context.tenant.pool, a, id, p).await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)))
}

#[cfg_attr(feature="api-docs",utoipa::path(post,path="/api/student-care/assignments/{id}/reviewers",operation_id="student_care_update_assignment_reviewers",tag="student-care",params(("id"=Uuid,Path)),request_body=UpdateCareAssignmentReviewers,responses((status=200,body=ApiResponse<CareAssignment>),(status=400,body=ApiErrorResponse),(status=401,body=ApiErrorResponse),(status=403,body=ApiErrorResponse),(status=404,body=ApiErrorResponse),(status=409,body=ApiErrorResponse),(status=422,body=ApiErrorResponse))))]
pub async fn update_assignment_reviewers(
    State(state): State<StudentCareHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(p): Json<UpdateCareAssignmentReviewers>,
) -> Result<impl IntoResponse, HttpError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let a = CareAccess::new(&context.actor);
    let result = services::update_assignment_reviewers(&context.tenant.pool, a, id, p).await?;
    school_notifications::runtime::wake_after_commit(&context.tenant.subdomain);
    Ok(Json(ApiResponse::ok(result)))
}
