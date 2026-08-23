use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::api_response::{ApiResponse, EmptyData};
use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::permissions::registry::codes;
use crate::policies::{
    academic_catalog_access_policy::{self, CatalogAction, CatalogResourceRef},
    academic_curriculum_access_policy::{self, CurriculumAction},
};
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::AppState;

use super::models::*;
use super::services::{
    bell_schedules, catalog, context, curriculum, progressions, student_years, years_terms,
};

#[derive(Debug, Deserialize)]
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
