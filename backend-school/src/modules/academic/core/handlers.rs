use axum::{
    extract::{Extension, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::api_response::{ApiErrorResponse, ApiResponse, EmptyData};
use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::permissions::registry::codes;
use crate::policies::{
    academic_catalog_access_policy::{self, CatalogAction, CatalogResourceRef},
    academic_curriculum_access_policy::{self, CurriculumAction},
};
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::utils::tenant::tenant_context;
use crate::AppState;

use super::models::*;
use super::services::{
    bell_schedules, catalog, context, curriculum, progressions, student_years, years_terms,
};

#[derive(Debug, Deserialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcademicYearQuery {
    pub academic_year_id: Uuid,
}

fn ok<T: serde::Serialize>(data: T) -> Response {
    Json(ApiResponse::ok(data)).into_response()
}

fn created<T: serde::Serialize>(data: T) -> Response {
    (StatusCode::CREATED, Json(ApiResponse::ok(data))).into_response()
}

fn signal_core_changed(
    state: &AppState,
    session: &AuthenticatedSession,
    actor: &ActorContext,
    entity_type: &str,
    entity_id: Option<Uuid>,
    academic_year_id: Option<Uuid>,
    academic_term_id: Option<Uuid>,
) {
    state.websocket_manager.broadcast_academic_core_changed(
        session.tenant.subdomain.clone(),
        actor.user_id,
        entity_type,
        entity_id,
        academic_year_id,
        academic_term_id,
    );
}

#[utoipa::path(
    get,
    path = "/api/academic/context/options",
    operation_id = "listAcademicContextOptions",
    tag = "academic",
    responses(
        (status = 200, description = "Academic context options", body = ApiResponse<AcademicContextOptions>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic context read permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_context_options(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_CONTEXT_READ_SCHOOL)?;
    Ok(ok(context::list_options(&pool).await?))
}

#[utoipa::path(
    get,
    path = "/api/public/academic-context/options",
    operation_id = "listPublicAcademicContextOptions",
    tag = "academic",
    responses(
        (status = 200, description = "Published academic years and terms available to the public calendar", body = ApiResponse<AcademicContextOptions>),
        (status = 404, description = "School tenant not found", body = ApiErrorResponse)
    )
)]
pub async fn list_public_context_options(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let tenant = tenant_context(&state, &headers).await?;
    Ok(ok(context::list_public_options(&tenant.pool).await?))
}

#[utoipa::path(
    get,
    path = "/api/me/academic-context/options",
    operation_id = "listMyAcademicContextOptions",
    tag = "academic",
    responses(
        (status = 200, description = "Academic years and terms available to the current student", body = ApiResponse<AcademicContextOptions>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student context access denied", body = ApiErrorResponse)
    )
)]
pub async fn list_my_context_options(
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Response, AppError> {
    if session.user_type != "student" {
        return Err(AppError::Forbidden(
            "เฉพาะนักเรียนเท่านั้นที่ดูประวัติบริบทของตนเองได้".to_string(),
        ));
    }
    Ok(ok(context::list_options_for_student(
        &session.tenant.pool,
        session.user_id,
    )
    .await?))
}

#[utoipa::path(
    get,
    path = "/api/academic/years",
    operation_id = "listAcademicYears",
    tag = "academic",
    responses(
        (status = 200, description = "Academic years", body = ApiResponse<Vec<AcademicYear>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic year read permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_years(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_any_permission(&[
        codes::ACADEMIC_YEAR_READ_SCHOOL,
        codes::ACADEMIC_YEAR_MANAGE_SCHOOL,
    ])?;
    Ok(ok(years_terms::list_years(&pool).await?))
}

#[utoipa::path(
    post,
    path = "/api/academic/years",
    operation_id = "createAcademicYear",
    tag = "academic",
    request_body = CreateAcademicYearRequest,
    responses(
        (status = 201, description = "Academic year created", body = ApiResponse<AcademicYear>),
        (status = 400, description = "Invalid academic year", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic year management permission denied", body = ApiErrorResponse),
        (status = 409, description = "Academic year conflict", body = ApiErrorResponse)
    )
)]
pub async fn create_year(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(request): Json<CreateAcademicYearRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_YEAR_MANAGE_SCHOOL)?;
    let year = years_terms::create_year(&pool, actor.user_id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "academic_year",
        Some(year.id),
        Some(year.id),
        None,
    );
    Ok(created(year))
}

#[utoipa::path(
    get,
    path = "/api/academic/years/{id}",
    operation_id = "getAcademicYear",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Academic year ID")),
    responses(
        (status = 200, description = "Academic year", body = ApiResponse<AcademicYear>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic year read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic year not found", body = ApiErrorResponse)
    )
)]
pub async fn get_year(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_any_permission(&[
        codes::ACADEMIC_YEAR_READ_SCHOOL,
        codes::ACADEMIC_YEAR_MANAGE_SCHOOL,
    ])?;
    Ok(ok(years_terms::get_year(&pool, id).await?))
}

#[utoipa::path(
    patch,
    path = "/api/academic/years/{id}",
    operation_id = "updateAcademicYear",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Academic year ID")),
    request_body = UpdateAcademicYearRequest,
    responses(
        (status = 200, description = "Academic year updated", body = ApiResponse<AcademicYear>),
        (status = 400, description = "Invalid academic year", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic year management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic year not found", body = ApiErrorResponse),
        (status = 409, description = "Academic year row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn update_year(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateAcademicYearRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_YEAR_MANAGE_SCHOOL)?;
    let year = years_terms::update_year(&pool, actor.user_id, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "academic_year",
        Some(id),
        Some(id),
        None,
    );
    Ok(ok(year))
}

#[utoipa::path(
    get,
    path = "/api/academic/terms",
    operation_id = "listAcademicTerms",
    tag = "academic",
    params(AcademicYearQuery),
    responses(
        (status = 200, description = "Academic terms in the selected year", body = ApiResponse<Vec<AcademicTerm>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic term read permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_terms(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<AcademicYearQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_any_permission(&[
        codes::ACADEMIC_TERM_READ_SCHOOL,
        codes::ACADEMIC_TERM_MANAGE_SCHOOL,
    ])?;
    Ok(ok(
        years_terms::list_terms(&pool, query.academic_year_id).await?
    ))
}

#[utoipa::path(
    post,
    path = "/api/academic/terms",
    operation_id = "createAcademicTerm",
    tag = "academic",
    request_body = CreateAcademicTermRequest,
    responses(
        (status = 201, description = "Academic term created", body = ApiResponse<AcademicTerm>),
        (status = 400, description = "Invalid academic term", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic term management permission denied", body = ApiErrorResponse),
        (status = 409, description = "Academic term conflict", body = ApiErrorResponse)
    )
)]
pub async fn create_term(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(request): Json<CreateAcademicTermRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_TERM_MANAGE_SCHOOL)?;
    let term = years_terms::create_term(&pool, actor.user_id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "academic_term",
        Some(term.id),
        Some(term.academic_year_id),
        Some(term.id),
    );
    Ok(created(term))
}

#[utoipa::path(
    get,
    path = "/api/academic/terms/{id}",
    operation_id = "getAcademicTerm",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Academic term ID")),
    responses(
        (status = 200, description = "Academic term", body = ApiResponse<AcademicTerm>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic term read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic term not found", body = ApiErrorResponse)
    )
)]
pub async fn get_term(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_any_permission(&[
        codes::ACADEMIC_TERM_READ_SCHOOL,
        codes::ACADEMIC_TERM_MANAGE_SCHOOL,
    ])?;
    Ok(ok(years_terms::get_term(&pool, id).await?))
}

#[utoipa::path(
    patch,
    path = "/api/academic/terms/{id}",
    operation_id = "updateAcademicTerm",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Academic term ID")),
    request_body = UpdateAcademicTermRequest,
    responses(
        (status = 200, description = "Academic term updated", body = ApiResponse<AcademicTerm>),
        (status = 400, description = "Invalid academic term", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic term management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic term not found", body = ApiErrorResponse),
        (status = 409, description = "Academic term row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn update_term(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateAcademicTermRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_TERM_MANAGE_SCHOOL)?;
    let term = years_terms::update_term(&pool, actor.user_id, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "academic_term",
        Some(id),
        Some(term.academic_year_id),
        Some(id),
    );
    Ok(ok(term))
}

#[utoipa::path(
    delete,
    path = "/api/academic/terms/{id}",
    operation_id = "deleteAcademicTerm",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Academic term ID")),
    responses(
        (status = 200, description = "Academic term deleted", body = ApiResponse<EmptyData>),
        (status = 400, description = "Academic term cannot be deleted", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic term management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic term not found", body = ApiErrorResponse),
        (status = 409, description = "Academic term has dependent records", body = ApiErrorResponse)
    )
)]
pub async fn delete_term(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_TERM_MANAGE_SCHOOL)?;
    let term = years_terms::get_term(&pool, id).await?;
    years_terms::delete_term(&pool, actor.user_id, id).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "academic_term",
        Some(id),
        Some(term.academic_year_id),
        None,
    );
    Ok(ok(EmptyData {}))
}

#[utoipa::path(
    get,
    path = "/api/academic/bell-schedules",
    operation_id = "listBellSchedules",
    tag = "academic",
    params(AcademicYearQuery),
    responses(
        (status = 200, description = "Bell schedules in the selected year", body = ApiResponse<Vec<BellSchedule>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic term read permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_bell_schedules(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<AcademicYearQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_any_permission(&[
        codes::ACADEMIC_TERM_READ_SCHOOL,
        codes::ACADEMIC_TERM_MANAGE_SCHOOL,
    ])?;
    Ok(ok(
        bell_schedules::list(&pool, query.academic_year_id).await?
    ))
}

#[utoipa::path(
    post,
    path = "/api/academic/bell-schedules",
    operation_id = "createBellSchedule",
    tag = "academic",
    request_body = CreateBellScheduleRequest,
    responses(
        (status = 201, description = "Bell schedule created", body = ApiResponse<BellSchedule>),
        (status = 400, description = "Invalid bell schedule", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic term management permission denied", body = ApiErrorResponse),
        (status = 409, description = "Bell schedule conflict", body = ApiErrorResponse)
    )
)]
pub async fn create_bell_schedule(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(request): Json<CreateBellScheduleRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_TERM_MANAGE_SCHOOL)?;
    let schedule = bell_schedules::create(&pool, actor.user_id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "bell_schedule",
        Some(schedule.id),
        Some(schedule.academic_year_id),
        None,
    );
    Ok(created(schedule))
}

#[utoipa::path(
    get,
    path = "/api/academic/bell-schedules/{id}",
    operation_id = "getBellSchedule",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Bell schedule ID")),
    responses(
        (status = 200, description = "Bell schedule", body = ApiResponse<BellSchedule>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic term read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Bell schedule not found", body = ApiErrorResponse)
    )
)]
pub async fn get_bell_schedule(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_any_permission(&[
        codes::ACADEMIC_TERM_READ_SCHOOL,
        codes::ACADEMIC_TERM_MANAGE_SCHOOL,
    ])?;
    Ok(ok(bell_schedules::get(&pool, id).await?))
}

#[utoipa::path(
    patch,
    path = "/api/academic/bell-schedules/{id}",
    operation_id = "updateBellSchedule",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Bell schedule ID")),
    request_body = UpdateBellScheduleRequest,
    responses(
        (status = 200, description = "Bell schedule updated", body = ApiResponse<BellSchedule>),
        (status = 400, description = "Invalid bell schedule", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic term management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Bell schedule not found", body = ApiErrorResponse),
        (status = 409, description = "Bell schedule row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn update_bell_schedule(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateBellScheduleRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_TERM_MANAGE_SCHOOL)?;
    let schedule = bell_schedules::update(&pool, actor.user_id, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "bell_schedule",
        Some(id),
        Some(schedule.academic_year_id),
        None,
    );
    Ok(ok(schedule))
}

#[utoipa::path(
    get,
    path = "/api/academic/bell-schedules/{id}/periods",
    operation_id = "listBellSchedulePeriods",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Bell schedule ID")),
    responses(
        (status = 200, description = "Bell schedule periods", body = ApiResponse<Vec<BellSchedulePeriod>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic term read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Bell schedule not found", body = ApiErrorResponse)
    )
)]
pub async fn list_bell_schedule_periods(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_any_permission(&[
        codes::ACADEMIC_TERM_READ_SCHOOL,
        codes::ACADEMIC_TERM_MANAGE_SCHOOL,
    ])?;
    Ok(ok(bell_schedules::list_periods(&pool, id).await?))
}

#[utoipa::path(
    put,
    path = "/api/academic/bell-schedules/{id}/periods",
    operation_id = "replaceBellSchedulePeriods",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Bell schedule ID")),
    request_body = ReplaceBellSchedulePeriodsRequest,
    responses(
        (status = 200, description = "Bell schedule periods replaced", body = ApiResponse<Vec<BellSchedulePeriod>>),
        (status = 400, description = "Invalid bell schedule periods", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic term management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Bell schedule not found", body = ApiErrorResponse),
        (status = 409, description = "Bell schedule row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn replace_bell_schedule_periods(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<ReplaceBellSchedulePeriodsRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_TERM_MANAGE_SCHOOL)?;
    let periods = bell_schedules::replace_periods(&pool, actor.user_id, id, request).await?;
    let schedule = bell_schedules::get(&pool, id).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "bell_schedule_periods",
        Some(id),
        Some(schedule.academic_year_id),
        None,
    );
    Ok(ok(periods))
}

#[utoipa::path(
    get,
    path = "/api/academic/grade-progressions",
    operation_id = "listGradeProgressions",
    tag = "academic",
    responses(
        (status = 200, description = "Grade progression rules", body = ApiResponse<GradeProgressionSet>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic year read permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_grade_progressions(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_any_permission(&[
        codes::ACADEMIC_YEAR_READ_SCHOOL,
        codes::ACADEMIC_YEAR_MANAGE_SCHOOL,
    ])?;
    Ok(ok(progressions::list(&pool).await?))
}

#[utoipa::path(
    put,
    path = "/api/academic/grade-progressions",
    operation_id = "replaceGradeProgressions",
    tag = "academic",
    request_body = ReplaceGradeProgressionsRequest,
    responses(
        (status = 200, description = "Grade progression rules replaced", body = ApiResponse<GradeProgressionSet>),
        (status = 400, description = "Invalid grade progression rules", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic year management permission denied", body = ApiErrorResponse),
        (status = 409, description = "Grade progression row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn replace_grade_progressions(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(request): Json<ReplaceGradeProgressionsRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_YEAR_MANAGE_SCHOOL)?;
    let values = progressions::replace(&pool, actor.user_id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "grade_progressions",
        None,
        None,
        None,
    );
    Ok(ok(values))
}

#[utoipa::path(
    get,
    path = "/api/academic/catalog/subjects",
    operation_id = "listCatalogSubjects",
    tag = "academic",
    responses(
        (status = 200, description = "Catalog subjects", body = ApiResponse<Vec<CatalogSubject>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog read permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_catalog_subjects(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let filter = academic_catalog_access_policy::require_academic_catalog_list_access(
        &pool,
        &actor,
        CatalogAction::Read,
    )
    .await?;
    Ok(ok(catalog::list_subjects(&pool, &filter).await?))
}

#[utoipa::path(
    post,
    path = "/api/academic/catalog/subjects",
    operation_id = "createCatalogSubject",
    tag = "academic",
    request_body = CreateCatalogSubjectRequest,
    responses(
        (status = 201, description = "Catalog subject created", body = ApiResponse<CatalogSubject>),
        (status = 400, description = "Invalid catalog subject", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog management permission denied", body = ApiErrorResponse),
        (status = 409, description = "Catalog subject conflict", body = ApiErrorResponse)
    )
)]
pub async fn create_catalog_subject(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(request): Json<CreateCatalogSubjectRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let filter = academic_catalog_access_policy::require_academic_catalog_list_access(
        &pool,
        &actor,
        CatalogAction::Manage,
    )
    .await?;
    if !catalog::owner_allowed(&filter, request.owning_organization_unit_id) {
        return Err(AppError::Forbidden(
            "ไม่มีสิทธิ์สร้างรายวิชาในหน่วยงานนี้".to_string(),
        ));
    }
    let subject = catalog::create_subject(&pool, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "catalog_subject",
        Some(subject.id),
        None,
        None,
    );
    Ok(created(subject))
}

#[utoipa::path(
    get,
    path = "/api/academic/catalog/subjects/{id}",
    operation_id = "getCatalogSubject",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Catalog subject ID")),
    responses(
        (status = 200, description = "Catalog subject", body = ApiResponse<CatalogSubject>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Catalog subject not found", body = ApiErrorResponse)
    )
)]
pub async fn get_catalog_subject(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Subject(id),
        CatalogAction::Read,
    )
    .await?;
    Ok(ok(catalog::get_subject(&pool, id).await?))
}

#[utoipa::path(
    patch,
    path = "/api/academic/catalog/subjects/{id}",
    operation_id = "updateCatalogSubject",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Catalog subject ID")),
    request_body = UpdateCatalogSubjectRequest,
    responses(
        (status = 200, description = "Catalog subject updated", body = ApiResponse<CatalogSubject>),
        (status = 400, description = "Invalid catalog subject", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Catalog subject not found", body = ApiErrorResponse),
        (status = 409, description = "Catalog subject row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn update_catalog_subject(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateCatalogSubjectRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Subject(id),
        CatalogAction::Manage,
    )
    .await?;
    let filter = academic_catalog_access_policy::require_academic_catalog_list_access(
        &pool,
        &actor,
        CatalogAction::Manage,
    )
    .await?;
    if !catalog::owner_allowed(&filter, request.owning_organization_unit_id) {
        return Err(AppError::Forbidden(
            "ไม่มีสิทธิ์ย้ายเจ้าของรายวิชาไปหน่วยงานนี้".to_string(),
        ));
    }
    let subject = catalog::update_subject(&pool, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "catalog_subject",
        Some(id),
        None,
        None,
    );
    Ok(ok(subject))
}

#[utoipa::path(
    get,
    path = "/api/academic/catalog/subjects/{id}/versions",
    operation_id = "listSubjectVersions",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Catalog subject ID")),
    responses(
        (status = 200, description = "Subject versions", body = ApiResponse<Vec<SubjectVersion>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Catalog subject not found", body = ApiErrorResponse)
    )
)]
pub async fn list_subject_versions(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(subject_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Subject(subject_id),
        CatalogAction::Read,
    )
    .await?;
    Ok(ok(catalog::list_subject_versions(&pool, subject_id).await?))
}

#[utoipa::path(
    post,
    path = "/api/academic/catalog/subjects/{id}/versions",
    operation_id = "createSubjectVersion",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Catalog subject ID")),
    request_body = CreateSubjectVersionRequest,
    responses(
        (status = 201, description = "Subject version created", body = ApiResponse<SubjectVersion>),
        (status = 400, description = "Invalid subject version", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Catalog subject not found", body = ApiErrorResponse),
        (status = 409, description = "Subject version conflict", body = ApiErrorResponse)
    )
)]
pub async fn create_subject_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(subject_id): Path<Uuid>,
    Json(request): Json<CreateSubjectVersionRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Subject(subject_id),
        CatalogAction::Manage,
    )
    .await?;
    let version = catalog::create_subject_version(&pool, subject_id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "subject_version",
        Some(version.id),
        None,
        None,
    );
    Ok(created(version))
}

#[utoipa::path(
    get,
    path = "/api/academic/catalog/subject-versions/{id}",
    operation_id = "getSubjectVersion",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Subject version ID")),
    responses(
        (status = 200, description = "Subject version", body = ApiResponse<SubjectVersion>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Subject version not found", body = ApiErrorResponse)
    )
)]
pub async fn get_subject_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let version = catalog::get_subject_version(&pool, id).await?;
    let subject_id = version.subject_id;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Subject(subject_id),
        CatalogAction::Read,
    )
    .await?;
    Ok(ok(version))
}

#[utoipa::path(
    patch,
    path = "/api/academic/catalog/subject-versions/{id}",
    operation_id = "updateSubjectVersion",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Subject version ID")),
    request_body = UpdateSubjectVersionRequest,
    responses(
        (status = 200, description = "Subject version updated", body = ApiResponse<SubjectVersion>),
        (status = 400, description = "Invalid or immutable subject version", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Subject version not found", body = ApiErrorResponse),
        (status = 409, description = "Subject version row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn update_subject_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateSubjectVersionRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let subject_id = catalog::get_subject_version(&pool, id).await?.subject_id;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Subject(subject_id),
        CatalogAction::Manage,
    )
    .await?;
    let version = catalog::update_subject_version(&pool, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "subject_version",
        Some(id),
        None,
        None,
    );
    Ok(ok(version))
}

#[utoipa::path(
    post,
    path = "/api/academic/catalog/subject-versions/{id}/publish",
    operation_id = "publishSubjectVersion",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Subject version ID")),
    request_body = PublishVersionRequest,
    responses(
        (status = 200, description = "Subject version published", body = ApiResponse<SubjectVersion>),
        (status = 400, description = "Subject version cannot be published", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Subject version not found", body = ApiErrorResponse),
        (status = 409, description = "Subject version row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn publish_subject_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<PublishVersionRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let subject_id = catalog::get_subject_version(&pool, id).await?.subject_id;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Subject(subject_id),
        CatalogAction::Manage,
    )
    .await?;
    let version = catalog::publish_subject_version(&pool, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "subject_version",
        Some(id),
        None,
        None,
    );
    Ok(ok(version))
}

#[utoipa::path(
    get,
    path = "/api/academic/catalog/subjects/{id}/default-teachers",
    operation_id = "listSubjectDefaultTeachers",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Catalog subject ID")),
    responses(
        (status = 200, description = "Subject default teachers", body = ApiResponse<Vec<DefaultTeacher>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Catalog subject not found", body = ApiErrorResponse)
    )
)]
pub async fn list_subject_default_teachers(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Subject(id),
        CatalogAction::Read,
    )
    .await?;
    Ok(ok(catalog::list_subject_default_teachers(&pool, id).await?))
}

#[utoipa::path(
    put,
    path = "/api/academic/catalog/subjects/{id}/default-teachers",
    operation_id = "replaceSubjectDefaultTeachers",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Catalog subject ID")),
    request_body = ReplaceDefaultTeachersRequest,
    responses(
        (status = 200, description = "Subject default teachers replaced", body = ApiResponse<Vec<DefaultTeacher>>),
        (status = 400, description = "Invalid default teachers", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Catalog subject not found", body = ApiErrorResponse),
        (status = 409, description = "Catalog subject row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn replace_subject_default_teachers(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<ReplaceDefaultTeachersRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Subject(id),
        CatalogAction::Manage,
    )
    .await?;
    let teachers = catalog::replace_subject_default_teachers(&pool, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "subject_default_teachers",
        Some(id),
        None,
        None,
    );
    Ok(ok(teachers))
}

#[utoipa::path(
    get,
    path = "/api/academic/catalog/subject-groups",
    operation_id = "listSubjectGroups",
    tag = "academic",
    responses(
        (status = 200, description = "Subject groups", body = ApiResponse<Vec<SubjectGroup>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog read permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_subject_groups(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let filter = academic_catalog_access_policy::require_academic_catalog_list_access(
        &pool,
        &actor,
        CatalogAction::Read,
    )
    .await?;
    if !filter.includes_school_owned {
        return Err(AppError::Forbidden("กลุ่มสาระเป็นข้อมูลระดับโรงเรียน".to_string()));
    }
    Ok(ok(catalog::list_subject_groups(&pool).await?))
}

#[utoipa::path(
    post,
    path = "/api/academic/catalog/subject-groups",
    operation_id = "createSubjectGroup",
    tag = "academic",
    request_body = CreateSubjectGroupRequest,
    responses(
        (status = 201, description = "Subject group created", body = ApiResponse<SubjectGroup>),
        (status = 400, description = "Invalid subject group", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog management permission denied", body = ApiErrorResponse),
        (status = 409, description = "Subject group conflict", body = ApiErrorResponse)
    )
)]
pub async fn create_subject_group(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(request): Json<CreateSubjectGroupRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_CATALOG_MANAGE_SCHOOL)?;
    let group = catalog::create_subject_group(&pool, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "subject_group",
        Some(group.id),
        None,
        None,
    );
    Ok(created(group))
}

#[utoipa::path(
    get,
    path = "/api/academic/catalog/subject-groups/{id}",
    operation_id = "getSubjectGroup",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Subject group ID")),
    responses(
        (status = 200, description = "Subject group", body = ApiResponse<SubjectGroup>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Subject group not found", body = ApiErrorResponse)
    )
)]
pub async fn get_subject_group(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_any_permission(&[
        codes::ACADEMIC_CATALOG_READ_SCHOOL,
        codes::ACADEMIC_CATALOG_MANAGE_SCHOOL,
    ])?;
    let group = catalog::list_subject_groups(&pool)
        .await?
        .into_iter()
        .find(|group| group.id == id)
        .ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มสาระ".to_string()))?;
    Ok(ok(group))
}

#[utoipa::path(
    patch,
    path = "/api/academic/catalog/subject-groups/{id}",
    operation_id = "updateSubjectGroup",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Subject group ID")),
    request_body = UpdateSubjectGroupRequest,
    responses(
        (status = 200, description = "Subject group updated", body = ApiResponse<SubjectGroup>),
        (status = 400, description = "Invalid subject group", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Subject group not found", body = ApiErrorResponse),
        (status = 409, description = "Subject group row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn update_subject_group(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateSubjectGroupRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_CATALOG_MANAGE_SCHOOL)?;
    let group = catalog::update_subject_group(&pool, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "subject_group",
        Some(id),
        None,
        None,
    );
    Ok(ok(group))
}

#[utoipa::path(
    delete,
    path = "/api/academic/catalog/subject-groups/{id}",
    operation_id = "deleteSubjectGroup",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Subject group ID")),
    responses(
        (status = 200, description = "Subject group deleted", body = ApiResponse<EmptyData>),
        (status = 400, description = "Subject group cannot be deleted", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Subject group not found", body = ApiErrorResponse),
        (status = 409, description = "Subject group has dependent records", body = ApiErrorResponse)
    )
)]
pub async fn delete_subject_group(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_CATALOG_MANAGE_SCHOOL)?;
    catalog::delete_subject_group(&pool, id).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "subject_group",
        Some(id),
        None,
        None,
    );
    Ok(ok(EmptyData {}))
}

#[utoipa::path(
    get,
    path = "/api/academic/catalog/activities",
    operation_id = "listCatalogActivities",
    tag = "academic",
    responses(
        (status = 200, description = "Catalog activities", body = ApiResponse<Vec<CatalogActivity>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog read permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_catalog_activities(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let filter = academic_catalog_access_policy::require_academic_catalog_list_access(
        &pool,
        &actor,
        CatalogAction::Read,
    )
    .await?;
    Ok(ok(catalog::list_activities(&pool, &filter).await?))
}

#[utoipa::path(
    post,
    path = "/api/academic/catalog/activities",
    operation_id = "createCatalogActivity",
    tag = "academic",
    request_body = CreateCatalogActivityRequest,
    responses(
        (status = 201, description = "Catalog activity created", body = ApiResponse<CatalogActivity>),
        (status = 400, description = "Invalid catalog activity", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog management permission denied", body = ApiErrorResponse),
        (status = 409, description = "Catalog activity conflict", body = ApiErrorResponse)
    )
)]
pub async fn create_catalog_activity(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(request): Json<CreateCatalogActivityRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let filter = academic_catalog_access_policy::require_academic_catalog_list_access(
        &pool,
        &actor,
        CatalogAction::Manage,
    )
    .await?;
    if !catalog::owner_allowed(&filter, request.owning_organization_unit_id) {
        return Err(AppError::Forbidden(
            "ไม่มีสิทธิ์สร้างกิจกรรมในหน่วยงานนี้".to_string(),
        ));
    }
    let activity = catalog::create_activity(&pool, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "catalog_activity",
        Some(activity.id),
        None,
        None,
    );
    Ok(created(activity))
}

#[utoipa::path(
    get,
    path = "/api/academic/catalog/activities/{id}",
    operation_id = "getCatalogActivity",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Catalog activity ID")),
    responses(
        (status = 200, description = "Catalog activity", body = ApiResponse<CatalogActivity>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Catalog activity not found", body = ApiErrorResponse)
    )
)]
pub async fn get_catalog_activity(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Activity(id),
        CatalogAction::Read,
    )
    .await?;
    Ok(ok(catalog::get_activity(&pool, id).await?))
}

#[utoipa::path(
    patch,
    path = "/api/academic/catalog/activities/{id}",
    operation_id = "updateCatalogActivity",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Catalog activity ID")),
    request_body = UpdateCatalogActivityRequest,
    responses(
        (status = 200, description = "Catalog activity updated", body = ApiResponse<CatalogActivity>),
        (status = 400, description = "Invalid catalog activity", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Catalog activity not found", body = ApiErrorResponse),
        (status = 409, description = "Catalog activity row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn update_catalog_activity(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateCatalogActivityRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Activity(id),
        CatalogAction::Manage,
    )
    .await?;
    let filter = academic_catalog_access_policy::require_academic_catalog_list_access(
        &pool,
        &actor,
        CatalogAction::Manage,
    )
    .await?;
    if !catalog::owner_allowed(&filter, request.owning_organization_unit_id) {
        return Err(AppError::Forbidden(
            "ไม่มีสิทธิ์ย้ายเจ้าของกิจกรรมไปหน่วยงานนี้".to_string(),
        ));
    }
    let activity = catalog::update_activity(&pool, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "catalog_activity",
        Some(id),
        None,
        None,
    );
    Ok(ok(activity))
}

#[utoipa::path(
    get,
    path = "/api/academic/catalog/activities/{id}/versions",
    operation_id = "listActivityVersions",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Catalog activity ID")),
    responses(
        (status = 200, description = "Activity versions", body = ApiResponse<Vec<ActivityVersion>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Catalog activity not found", body = ApiErrorResponse)
    )
)]
pub async fn list_activity_versions(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(activity_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Activity(activity_id),
        CatalogAction::Read,
    )
    .await?;
    Ok(ok(
        catalog::list_activity_versions(&pool, activity_id).await?
    ))
}

#[utoipa::path(
    post,
    path = "/api/academic/catalog/activities/{id}/versions",
    operation_id = "createActivityVersion",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Catalog activity ID")),
    request_body = CreateActivityVersionRequest,
    responses(
        (status = 201, description = "Activity version created", body = ApiResponse<ActivityVersion>),
        (status = 400, description = "Invalid activity version", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Catalog activity not found", body = ApiErrorResponse),
        (status = 409, description = "Activity version conflict", body = ApiErrorResponse)
    )
)]
pub async fn create_activity_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(activity_id): Path<Uuid>,
    Json(request): Json<CreateActivityVersionRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Activity(activity_id),
        CatalogAction::Manage,
    )
    .await?;
    let version = catalog::create_activity_version(&pool, activity_id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "activity_version",
        Some(version.id),
        None,
        None,
    );
    Ok(created(version))
}

#[utoipa::path(
    get,
    path = "/api/academic/catalog/activity-versions/{id}",
    operation_id = "getActivityVersion",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity version ID")),
    responses(
        (status = 200, description = "Activity version", body = ApiResponse<ActivityVersion>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity version not found", body = ApiErrorResponse)
    )
)]
pub async fn get_activity_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let version = catalog::get_activity_version(&pool, id).await?;
    let activity_id = version.activity_id;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Activity(activity_id),
        CatalogAction::Read,
    )
    .await?;
    Ok(ok(version))
}

#[utoipa::path(
    patch,
    path = "/api/academic/catalog/activity-versions/{id}",
    operation_id = "updateActivityVersion",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity version ID")),
    request_body = UpdateActivityVersionRequest,
    responses(
        (status = 200, description = "Activity version updated", body = ApiResponse<ActivityVersion>),
        (status = 400, description = "Invalid or immutable activity version", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity version not found", body = ApiErrorResponse),
        (status = 409, description = "Activity version row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn update_activity_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateActivityVersionRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let activity_id = catalog::get_activity_version(&pool, id).await?.activity_id;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Activity(activity_id),
        CatalogAction::Manage,
    )
    .await?;
    let version = catalog::update_activity_version(&pool, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "activity_version",
        Some(id),
        None,
        None,
    );
    Ok(ok(version))
}

#[utoipa::path(
    post,
    path = "/api/academic/catalog/activity-versions/{id}/publish",
    operation_id = "publishActivityVersion",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Activity version ID")),
    request_body = PublishVersionRequest,
    responses(
        (status = 200, description = "Activity version published", body = ApiResponse<ActivityVersion>),
        (status = 400, description = "Activity version cannot be published", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Activity version not found", body = ApiErrorResponse),
        (status = 409, description = "Activity version row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn publish_activity_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<PublishVersionRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let activity_id = catalog::get_activity_version(&pool, id).await?.activity_id;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Activity(activity_id),
        CatalogAction::Manage,
    )
    .await?;
    let version = catalog::publish_activity_version(&pool, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "activity_version",
        Some(id),
        None,
        None,
    );
    Ok(ok(version))
}

#[utoipa::path(
    get,
    path = "/api/academic/catalog/activities/{id}/default-teachers",
    operation_id = "listActivityDefaultTeachers",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Catalog activity ID")),
    responses(
        (status = 200, description = "Activity default teachers", body = ApiResponse<Vec<DefaultTeacher>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Catalog activity not found", body = ApiErrorResponse)
    )
)]
pub async fn list_activity_default_teachers(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Activity(id),
        CatalogAction::Read,
    )
    .await?;
    Ok(ok(catalog::list_activity_default_teachers(&pool, id).await?))
}

#[utoipa::path(
    put,
    path = "/api/academic/catalog/activities/{id}/default-teachers",
    operation_id = "replaceActivityDefaultTeachers",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Catalog activity ID")),
    request_body = ReplaceDefaultTeachersRequest,
    responses(
        (status = 200, description = "Activity default teachers replaced", body = ApiResponse<Vec<DefaultTeacher>>),
        (status = 400, description = "Invalid default teachers", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic catalog management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Catalog activity not found", body = ApiErrorResponse),
        (status = 409, description = "Catalog activity row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn replace_activity_default_teachers(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<ReplaceDefaultTeachersRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    academic_catalog_access_policy::require_academic_catalog_access(
        &pool,
        &actor,
        CatalogResourceRef::Activity(id),
        CatalogAction::Manage,
    )
    .await?;
    let teachers = catalog::replace_activity_default_teachers(&pool, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "activity_default_teachers",
        Some(id),
        None,
        None,
    );
    Ok(ok(teachers))
}

#[utoipa::path(
    get,
    path = "/api/academic/curricula",
    operation_id = "listCurricula",
    tag = "academic",
    responses(
        (status = 200, description = "Curricula", body = ApiResponse<Vec<Curriculum>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic curriculum read permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_curricula(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let filter = academic_curriculum_access_policy::require_academic_curriculum_list_access(
        &pool,
        &actor,
        CurriculumAction::Read,
    )
    .await?;
    Ok(ok(curriculum::list(&pool, &filter).await?))
}

#[utoipa::path(
    get,
    path = "/api/academic/study-program-options",
    operation_id = "listStudyProgramOptionsForAcademicYear",
    tag = "academic",
    params(AcademicYearQuery),
    responses(
        (status = 200, description = "Published study programs effective in the selected year", body = ApiResponse<Vec<StudyProgramOption>>),
        (status = 400, description = "Invalid academic year query or workspace exceeds the supported size", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic curriculum read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic year not found", body = ApiErrorResponse)
    )
)]
pub async fn list_study_program_options_for_year(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<AcademicYearQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let filter = academic_curriculum_access_policy::require_academic_curriculum_list_access(
        &pool,
        &actor,
        CurriculumAction::Read,
    )
    .await?;
    Ok(ok(curriculum::list_study_program_options_for_year(
        &pool,
        query.academic_year_id,
        &filter,
    )
    .await?))
}

#[utoipa::path(
    post,
    path = "/api/academic/curricula",
    operation_id = "createCurriculum",
    tag = "academic",
    request_body = CreateCurriculumRequest,
    responses(
        (status = 201, description = "Curriculum created", body = ApiResponse<Curriculum>),
        (status = 400, description = "Invalid curriculum", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic curriculum management permission denied", body = ApiErrorResponse),
        (status = 409, description = "Curriculum conflict", body = ApiErrorResponse)
    )
)]
pub async fn create_curriculum(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(request): Json<CreateCurriculumRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let filter = academic_curriculum_access_policy::require_academic_curriculum_list_access(
        &pool,
        &actor,
        CurriculumAction::Manage,
    )
    .await?;
    if !catalog::owner_allowed(&filter, request.owning_organization_unit_id) {
        return Err(AppError::Forbidden(
            "ไม่มีสิทธิ์สร้างหลักสูตรในหน่วยงานนี้".to_string(),
        ));
    }
    let value = curriculum::create(&pool, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "curriculum",
        Some(value.id),
        None,
        None,
    );
    Ok(created(value))
}

#[utoipa::path(
    get,
    path = "/api/academic/curricula/{id}",
    operation_id = "getCurriculum",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Curriculum ID")),
    responses(
        (status = 200, description = "Curriculum", body = ApiResponse<Curriculum>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic curriculum read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Curriculum not found", body = ApiErrorResponse)
    )
)]
pub async fn get_curriculum(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    academic_curriculum_access_policy::require_academic_curriculum_access(
        &pool,
        &actor,
        id,
        CurriculumAction::Read,
    )
    .await?;
    Ok(ok(curriculum::get(&pool, id).await?))
}

#[utoipa::path(
    patch,
    path = "/api/academic/curricula/{id}",
    operation_id = "updateCurriculum",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Curriculum ID")),
    request_body = UpdateCurriculumRequest,
    responses(
        (status = 200, description = "Curriculum updated", body = ApiResponse<Curriculum>),
        (status = 400, description = "Invalid curriculum", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic curriculum management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Curriculum not found", body = ApiErrorResponse),
        (status = 409, description = "Curriculum row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn update_curriculum(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateCurriculumRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    academic_curriculum_access_policy::require_academic_curriculum_access(
        &pool,
        &actor,
        id,
        CurriculumAction::Manage,
    )
    .await?;
    let filter = academic_curriculum_access_policy::require_academic_curriculum_list_access(
        &pool,
        &actor,
        CurriculumAction::Manage,
    )
    .await?;
    if !catalog::owner_allowed(&filter, request.owning_organization_unit_id) {
        return Err(AppError::Forbidden(
            "ไม่มีสิทธิ์ย้ายเจ้าของหลักสูตรไปหน่วยงานนี้".to_string(),
        ));
    }
    let value = curriculum::update(&pool, id, request).await?;
    signal_core_changed(&state, &session, &actor, "curriculum", Some(id), None, None);
    Ok(ok(value))
}

#[utoipa::path(
    get,
    path = "/api/academic/curricula/{id}/versions",
    operation_id = "listCurriculumVersions",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Curriculum ID")),
    responses(
        (status = 200, description = "Curriculum versions", body = ApiResponse<Vec<CurriculumVersion>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic curriculum read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Curriculum not found", body = ApiErrorResponse)
    )
)]
pub async fn list_curriculum_versions(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(curriculum_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    academic_curriculum_access_policy::require_academic_curriculum_access(
        &pool,
        &actor,
        curriculum_id,
        CurriculumAction::Read,
    )
    .await?;
    Ok(ok(curriculum::list_versions(&pool, curriculum_id).await?))
}

#[utoipa::path(
    post,
    path = "/api/academic/curricula/{id}/versions",
    operation_id = "createCurriculumVersion",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Curriculum ID")),
    request_body = CreateCurriculumVersionRequest,
    responses(
        (status = 201, description = "Curriculum version created", body = ApiResponse<CurriculumVersion>),
        (status = 400, description = "Invalid curriculum version", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic curriculum management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Curriculum not found", body = ApiErrorResponse),
        (status = 409, description = "Curriculum version conflict", body = ApiErrorResponse)
    )
)]
pub async fn create_curriculum_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(curriculum_id): Path<Uuid>,
    Json(request): Json<CreateCurriculumVersionRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    academic_curriculum_access_policy::require_academic_curriculum_access(
        &pool,
        &actor,
        curriculum_id,
        CurriculumAction::Manage,
    )
    .await?;
    let value = curriculum::create_version(&pool, curriculum_id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "curriculum_version",
        Some(value.id),
        None,
        None,
    );
    Ok(created(value))
}

#[utoipa::path(
    get,
    path = "/api/academic/curriculum-versions/{id}",
    operation_id = "getCurriculumVersion",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Curriculum version ID")),
    responses(
        (status = 200, description = "Curriculum version", body = ApiResponse<CurriculumVersion>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic curriculum read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Curriculum version not found", body = ApiErrorResponse)
    )
)]
pub async fn get_curriculum_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let version = curriculum::get_version(&pool, id).await?;
    let curriculum_id = version.curriculum_id;
    academic_curriculum_access_policy::require_academic_curriculum_access(
        &pool,
        &actor,
        curriculum_id,
        CurriculumAction::Read,
    )
    .await?;
    Ok(ok(version))
}

#[utoipa::path(
    patch,
    path = "/api/academic/curriculum-versions/{id}",
    operation_id = "updateCurriculumVersion",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Curriculum version ID")),
    request_body = UpdateCurriculumVersionRequest,
    responses(
        (status = 200, description = "Curriculum version updated", body = ApiResponse<CurriculumVersion>),
        (status = 400, description = "Invalid or immutable curriculum version", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic curriculum management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Curriculum version not found", body = ApiErrorResponse),
        (status = 409, description = "Curriculum version row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn update_curriculum_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateCurriculumVersionRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let curriculum_id = curriculum::get_version(&pool, id).await?.curriculum_id;
    academic_curriculum_access_policy::require_academic_curriculum_access(
        &pool,
        &actor,
        curriculum_id,
        CurriculumAction::Manage,
    )
    .await?;
    let value = curriculum::update_version(&pool, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "curriculum_version",
        Some(id),
        None,
        None,
    );
    Ok(ok(value))
}

#[utoipa::path(
    post,
    path = "/api/academic/curriculum-versions/{id}/publish",
    operation_id = "publishCurriculumVersion",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Curriculum version ID")),
    request_body = PublishVersionRequest,
    responses(
        (status = 200, description = "Curriculum version published", body = ApiResponse<CurriculumVersion>),
        (status = 400, description = "Curriculum version cannot be published", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic curriculum management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Curriculum version not found", body = ApiErrorResponse),
        (status = 409, description = "Curriculum version row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn publish_curriculum_version(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<PublishVersionRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let curriculum_id = curriculum::get_version(&pool, id).await?.curriculum_id;
    academic_curriculum_access_policy::require_academic_curriculum_access(
        &pool,
        &actor,
        curriculum_id,
        CurriculumAction::Manage,
    )
    .await?;
    let value = curriculum::publish_version(&pool, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "curriculum_version",
        Some(id),
        None,
        None,
    );
    Ok(ok(value))
}

#[utoipa::path(
    get,
    path = "/api/academic/curriculum-versions/{id}/programs",
    operation_id = "listStudyPrograms",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Curriculum version ID")),
    responses(
        (status = 200, description = "Study programs", body = ApiResponse<Vec<StudyProgram>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic curriculum read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Curriculum version not found", body = ApiErrorResponse)
    )
)]
pub async fn list_study_programs(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(version_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let curriculum_id = curriculum::get_version(&pool, version_id)
        .await?
        .curriculum_id;
    academic_curriculum_access_policy::require_academic_curriculum_access(
        &pool,
        &actor,
        curriculum_id,
        CurriculumAction::Read,
    )
    .await?;
    Ok(ok(curriculum::list_programs(&pool, version_id).await?))
}

#[utoipa::path(
    post,
    path = "/api/academic/curriculum-versions/{id}/programs",
    operation_id = "createStudyProgram",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Curriculum version ID")),
    request_body = CreateStudyProgramRequest,
    responses(
        (status = 201, description = "Study program created", body = ApiResponse<StudyProgram>),
        (status = 400, description = "Invalid study program", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic curriculum management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Curriculum version not found", body = ApiErrorResponse),
        (status = 409, description = "Study program conflict", body = ApiErrorResponse)
    )
)]
pub async fn create_study_program(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(version_id): Path<Uuid>,
    Json(request): Json<CreateStudyProgramRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let curriculum_id = curriculum::get_version(&pool, version_id)
        .await?
        .curriculum_id;
    academic_curriculum_access_policy::require_academic_curriculum_access(
        &pool,
        &actor,
        curriculum_id,
        CurriculumAction::Manage,
    )
    .await?;
    let value = curriculum::create_program(&pool, version_id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "study_program",
        Some(value.id),
        None,
        None,
    );
    Ok(created(value))
}

#[utoipa::path(
    get,
    path = "/api/academic/study-programs/{id}",
    operation_id = "getStudyProgram",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Study program ID")),
    responses(
        (status = 200, description = "Study program", body = ApiResponse<StudyProgram>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic curriculum read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study program not found", body = ApiErrorResponse)
    )
)]
pub async fn get_study_program(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let program = curriculum::get_program(&pool, id).await?;
    let curriculum_id = curriculum::get_version(&pool, program.curriculum_version_id)
        .await?
        .curriculum_id;
    academic_curriculum_access_policy::require_academic_curriculum_access(
        &pool,
        &actor,
        curriculum_id,
        CurriculumAction::Read,
    )
    .await?;
    Ok(ok(program))
}

#[utoipa::path(
    patch,
    path = "/api/academic/study-programs/{id}",
    operation_id = "updateStudyProgram",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Study program ID")),
    request_body = UpdateStudyProgramRequest,
    responses(
        (status = 200, description = "Study program updated", body = ApiResponse<StudyProgram>),
        (status = 400, description = "Invalid or immutable study program", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic curriculum management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study program not found", body = ApiErrorResponse),
        (status = 409, description = "Study program row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn update_study_program(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateStudyProgramRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let program = curriculum::get_program(&pool, id).await?;
    let curriculum_id = curriculum::get_version(&pool, program.curriculum_version_id)
        .await?
        .curriculum_id;
    academic_curriculum_access_policy::require_academic_curriculum_access(
        &pool,
        &actor,
        curriculum_id,
        CurriculumAction::Manage,
    )
    .await?;
    let value = curriculum::update_program(&pool, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "study_program",
        Some(id),
        None,
        None,
    );
    Ok(ok(value))
}

#[utoipa::path(
    get,
    path = "/api/academic/study-programs/{id}/requirements",
    operation_id = "listProgramRequirements",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Study program ID")),
    responses(
        (status = 200, description = "Study program requirements", body = ApiResponse<Vec<ProgramRequirement>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic curriculum read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study program not found", body = ApiErrorResponse)
    )
)]
pub async fn list_program_requirements(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let program = curriculum::get_program(&pool, id).await?;
    let curriculum_id = curriculum::get_version(&pool, program.curriculum_version_id)
        .await?
        .curriculum_id;
    academic_curriculum_access_policy::require_academic_curriculum_access(
        &pool,
        &actor,
        curriculum_id,
        CurriculumAction::Read,
    )
    .await?;
    Ok(ok(curriculum::list_requirements(&pool, id).await?))
}

#[utoipa::path(
    put,
    path = "/api/academic/study-programs/{id}/requirements",
    operation_id = "replaceProgramRequirements",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Study program ID")),
    request_body = ReplaceProgramRequirementsRequest,
    responses(
        (status = 200, description = "Study program requirements replaced", body = ApiResponse<Vec<ProgramRequirement>>),
        (status = 400, description = "Invalid or immutable requirements", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Academic curriculum management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Study program not found", body = ApiErrorResponse),
        (status = 409, description = "Study program row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn replace_program_requirements(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<ReplaceProgramRequirementsRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let program = curriculum::get_program(&pool, id).await?;
    let curriculum_id = curriculum::get_version(&pool, program.curriculum_version_id)
        .await?
        .curriculum_id;
    academic_curriculum_access_policy::require_academic_curriculum_access(
        &pool,
        &actor,
        curriculum_id,
        CurriculumAction::Manage,
    )
    .await?;
    let values = curriculum::replace_requirements(&pool, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "program_requirements",
        Some(id),
        None,
        None,
    );
    Ok(ok(values))
}

#[utoipa::path(
    get,
    path = "/api/academic/homerooms",
    operation_id = "listHomerooms",
    tag = "academic",
    params(AcademicYearQuery),
    responses(
        (status = 200, description = "Homerooms in the selected year", body = ApiResponse<Vec<Homeroom>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Homeroom read permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_homerooms(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<AcademicYearQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_any_permission(&[codes::HOMEROOM_READ_SCHOOL, codes::HOMEROOM_MANAGE_SCHOOL])?;
    Ok(ok(student_years::list_homerooms(
        &pool,
        query.academic_year_id,
    )
    .await?))
}

#[utoipa::path(
    post,
    path = "/api/academic/homerooms",
    operation_id = "createHomeroom",
    tag = "academic",
    request_body = CreateHomeroomRequest,
    responses(
        (status = 201, description = "Homeroom created", body = ApiResponse<Homeroom>),
        (status = 400, description = "Invalid homeroom", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Homeroom management permission denied", body = ApiErrorResponse),
        (status = 409, description = "Homeroom conflict", body = ApiErrorResponse)
    )
)]
pub async fn create_homeroom(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(request): Json<CreateHomeroomRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::HOMEROOM_MANAGE_SCHOOL)?;
    let homeroom = student_years::create_homeroom(&pool, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "homeroom",
        Some(homeroom.id),
        Some(homeroom.academic_year_id),
        None,
    );
    Ok(created(homeroom))
}

#[utoipa::path(
    get,
    path = "/api/academic/homerooms/{id}",
    operation_id = "getHomeroom",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Homeroom ID")),
    responses(
        (status = 200, description = "Homeroom", body = ApiResponse<Homeroom>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Homeroom read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Homeroom not found", body = ApiErrorResponse)
    )
)]
pub async fn get_homeroom(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_any_permission(&[codes::HOMEROOM_READ_SCHOOL, codes::HOMEROOM_MANAGE_SCHOOL])?;
    Ok(ok(student_years::get_homeroom(&pool, id).await?))
}

#[utoipa::path(
    patch,
    path = "/api/academic/homerooms/{id}",
    operation_id = "updateHomeroom",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Homeroom ID")),
    request_body = UpdateHomeroomRequest,
    responses(
        (status = 200, description = "Homeroom updated", body = ApiResponse<Homeroom>),
        (status = 400, description = "Invalid homeroom", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Homeroom management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Homeroom not found", body = ApiErrorResponse),
        (status = 409, description = "Homeroom row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn update_homeroom(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateHomeroomRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::HOMEROOM_MANAGE_SCHOOL)?;
    let homeroom = student_years::update_homeroom(&pool, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "homeroom",
        Some(id),
        Some(homeroom.academic_year_id),
        None,
    );
    Ok(ok(homeroom))
}

#[utoipa::path(
    get,
    path = "/api/academic/homerooms/{id}/advisors",
    operation_id = "listHomeroomAdvisors",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Homeroom ID")),
    responses(
        (status = 200, description = "Homeroom advisors", body = ApiResponse<Vec<HomeroomAdvisor>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Homeroom read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Homeroom not found", body = ApiErrorResponse)
    )
)]
pub async fn list_homeroom_advisors(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_any_permission(&[codes::HOMEROOM_READ_SCHOOL, codes::HOMEROOM_MANAGE_SCHOOL])?;
    Ok(ok(student_years::list_advisors(&pool, id).await?))
}

#[utoipa::path(
    get,
    path = "/api/academic/homeroom-advisors",
    operation_id = "listHomeroomAdvisorsForAcademicYear",
    tag = "academic",
    params(AcademicYearQuery),
    responses(
        (status = 200, description = "Homeroom advisor assignments in the selected year", body = ApiResponse<Vec<HomeroomAdvisorAssignment>>),
        (status = 400, description = "Invalid academic year query or workspace exceeds the supported size", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Homeroom read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic year not found", body = ApiErrorResponse)
    )
)]
pub async fn list_homeroom_advisors_for_year(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<AcademicYearQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_any_permission(&[codes::HOMEROOM_READ_SCHOOL, codes::HOMEROOM_MANAGE_SCHOOL])?;
    Ok(ok(student_years::list_advisors_for_year(
        &pool,
        query.academic_year_id,
    )
    .await?))
}

#[utoipa::path(
    put,
    path = "/api/academic/homerooms/{id}/advisors",
    operation_id = "replaceHomeroomAdvisors",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Homeroom ID")),
    request_body = ReplaceHomeroomAdvisorsRequest,
    responses(
        (status = 200, description = "Homeroom advisors replaced", body = ApiResponse<Vec<HomeroomAdvisor>>),
        (status = 400, description = "Invalid homeroom advisors", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Homeroom management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Homeroom not found", body = ApiErrorResponse),
        (status = 409, description = "Homeroom row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn replace_homeroom_advisors(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<ReplaceHomeroomAdvisorsRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::HOMEROOM_MANAGE_SCHOOL)?;
    let advisors = student_years::replace_advisors(&pool, id, request).await?;
    let homeroom = student_years::get_homeroom(&pool, id).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "homeroom_advisors",
        Some(id),
        Some(homeroom.academic_year_id),
        None,
    );
    Ok(ok(advisors))
}

#[utoipa::path(
    get,
    path = "/api/academic/student-years",
    operation_id = "listStudentAcademicYears",
    tag = "academic",
    params(StudentAcademicYearFilter),
    responses(
        (status = 200, description = "Student academic years", body = ApiResponse<Vec<StudentAcademicYear>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student academic year read permission denied", body = ApiErrorResponse)
    )
)]
pub async fn list_student_years(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(filter): Query<StudentAcademicYearFilter>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_any_permission(&[
        codes::STUDENT_ACADEMIC_YEAR_READ_SCHOOL,
        codes::STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL,
    ])?;
    Ok(ok(student_years::list_student_years(&pool, filter).await?))
}

#[utoipa::path(
    post,
    path = "/api/academic/student-years",
    operation_id = "createStudentAcademicYear",
    tag = "academic",
    request_body = CreateStudentAcademicYearRequest,
    responses(
        (status = 201, description = "Student academic year created", body = ApiResponse<StudentAcademicYear>),
        (status = 400, description = "Invalid student academic year", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student academic year management permission denied", body = ApiErrorResponse),
        (status = 409, description = "Student academic year conflict", body = ApiErrorResponse)
    )
)]
pub async fn create_student_year(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(request): Json<CreateStudentAcademicYearRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL)?;
    let value = student_years::create_student_year(&pool, actor.user_id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "student_academic_year",
        Some(value.id),
        Some(value.academic_year_id),
        None,
    );
    Ok(created(value))
}

#[utoipa::path(
    get,
    path = "/api/academic/student-years/{id}",
    operation_id = "getStudentAcademicYear",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Student academic year ID")),
    responses(
        (status = 200, description = "Student academic year", body = ApiResponse<StudentAcademicYear>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student academic year read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Student academic year not found", body = ApiErrorResponse)
    )
)]
pub async fn get_student_year(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_any_permission(&[
        codes::STUDENT_ACADEMIC_YEAR_READ_SCHOOL,
        codes::STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL,
    ])?;
    Ok(ok(student_years::get_student_year(&pool, id).await?))
}

#[utoipa::path(
    patch,
    path = "/api/academic/student-years/{id}",
    operation_id = "updateStudentAcademicYear",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Student academic year ID")),
    request_body = UpdateStudentAcademicYearRequest,
    responses(
        (status = 200, description = "Student academic year updated", body = ApiResponse<StudentAcademicYear>),
        (status = 400, description = "Invalid student academic year", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student academic year management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Student academic year not found", body = ApiErrorResponse),
        (status = 409, description = "Student academic year row version conflict", body = ApiErrorResponse)
    )
)]
pub async fn update_student_year(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<UpdateStudentAcademicYearRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL)?;
    let value = student_years::update_student_year(&pool, actor.user_id, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "student_academic_year",
        Some(id),
        Some(value.academic_year_id),
        None,
    );
    Ok(ok(value))
}

#[utoipa::path(
    get,
    path = "/api/academic/student-years/{id}/placements",
    operation_id = "listHomeroomPlacements",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Student academic year ID")),
    responses(
        (status = 200, description = "Homeroom placement history", body = ApiResponse<Vec<HomeroomPlacement>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student academic year read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Student academic year not found", body = ApiErrorResponse)
    )
)]
pub async fn list_placements(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(student_year_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::STUDENT_ACADEMIC_YEAR_READ_SCHOOL)?;
    Ok(ok(
        student_years::list_placements(&pool, student_year_id).await?
    ))
}

#[utoipa::path(
    get,
    path = "/api/academic/placements",
    operation_id = "listPlacementsForAcademicYear",
    tag = "academic",
    params(AcademicYearQuery),
    responses(
        (status = 200, description = "Homeroom placements in the selected year", body = ApiResponse<Vec<HomeroomPlacement>>),
        (status = 400, description = "Invalid academic year query or workspace exceeds the supported size", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student academic year read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Academic year not found", body = ApiErrorResponse)
    )
)]
pub async fn list_placements_for_year(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<AcademicYearQuery>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::STUDENT_ACADEMIC_YEAR_READ_SCHOOL)?;
    Ok(ok(student_years::list_placements_for_year(
        &pool,
        query.academic_year_id,
    )
    .await?))
}

#[utoipa::path(
    post,
    path = "/api/academic/student-years/{id}/placements",
    operation_id = "createHomeroomPlacement",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Student academic year ID")),
    request_body = CreateHomeroomPlacementRequest,
    responses(
        (status = 201, description = "Homeroom placement created", body = ApiResponse<HomeroomPlacement>),
        (status = 400, description = "Invalid homeroom placement", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student academic year management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Student academic year or homeroom not found", body = ApiErrorResponse),
        (status = 409, description = "Homeroom placement conflict", body = ApiErrorResponse)
    )
)]
pub async fn create_placement(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(student_year_id): Path<Uuid>,
    Json(request): Json<CreateHomeroomPlacementRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL)?;
    let value =
        student_years::create_placement(&pool, actor.user_id, student_year_id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "homeroom_placement",
        Some(value.id),
        Some(value.academic_year_id),
        None,
    );
    Ok(created(value))
}

#[utoipa::path(
    post,
    path = "/api/academic/placements/{id}/transfer",
    operation_id = "transferHomeroomPlacement",
    tag = "academic",
    params(("id" = Uuid, Path, description = "Current homeroom placement ID")),
    request_body = TransferHomeroomPlacementRequest,
    responses(
        (status = 200, description = "Homeroom placement transferred", body = ApiResponse<HomeroomPlacementTransfer>),
        (status = 400, description = "Invalid homeroom placement transfer", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Student academic year management permission denied", body = ApiErrorResponse),
        (status = 404, description = "Homeroom placement or target homeroom not found", body = ApiErrorResponse),
        (status = 409, description = "Homeroom placement transfer conflict", body = ApiErrorResponse)
    )
)]
pub async fn transfer_placement(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(request): Json<TransferHomeroomPlacementRequest>,
) -> Result<Response, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::STUDENT_ACADEMIC_YEAR_MANAGE_SCHOOL)?;
    let value = student_years::transfer_placement(&pool, actor.user_id, id, request).await?;
    signal_core_changed(
        &state,
        &session,
        &actor,
        "homeroom_placement",
        Some(value.new_placement.id),
        Some(value.new_placement.academic_year_id),
        None,
    );
    Ok(ok(value))
}
