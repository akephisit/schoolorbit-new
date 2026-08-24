use axum::{
    extract::{Extension, Path, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::api_response::{ApiErrorResponse, ApiResponse, EmptyData};
use crate::error::AppError;
use crate::modules::academic::models::timetable::{
    ApplyTemplateRequest, ClearTimetableRequest, CreateTemplateRequest, FromCurrentRequest,
    UpdateTemplateRequest,
};
use crate::modules::academic::services::timetable_template_service;
use crate::modules::academic::websockets::TimetableEvent;
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;

#[utoipa::path(
    get,
    path = "/api/academic/timetable-templates",
    operation_id = "listTimetableTemplates",
    responses(
        (status = 200, description = "Timetable templates", body = ApiResponse<Vec<crate::modules::academic::models::timetable::TimetableTemplate>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable template read permission denied", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn list_templates(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::LEARNING_OFFERING_READ_SCHOOL)?;
    Ok(Json(ApiResponse::ok(
        timetable_template_service::list_templates(&context.tenant.pool).await?,
    ))
    .into_response())
}

#[utoipa::path(
    get,
    path = "/api/academic/timetable-templates/{id}",
    operation_id = "getTimetableTemplate",
    params(("id" = Uuid, Path, description = "Timetable template ID")),
    responses(
        (status = 200, description = "Timetable template with entries", body = ApiResponse<crate::modules::academic::models::timetable::TemplateWithEntries>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable template read permission denied", body = ApiErrorResponse),
        (status = 404, description = "Timetable template not found", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn get_template(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::LEARNING_OFFERING_READ_SCHOOL)?;
    Ok(Json(ApiResponse::ok(
        timetable_template_service::get_template(&context.tenant.pool, id).await?,
    ))
    .into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/timetable-templates",
    operation_id = "createTimetableTemplate",
    request_body = CreateTemplateRequest,
    responses(
        (status = 200, description = "Created timetable template", body = ApiResponse<crate::modules::academic::models::timetable::TimetableTemplate>),
        (status = 400, description = "Invalid timetable template", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable template manage permission denied", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn create_template(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<CreateTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::LEARNING_OFFERING_MANAGE_SCHOOL)?;
    let template = timetable_template_service::create_template(
        &context.tenant.pool,
        context.actor.user_id,
        payload,
    )
    .await?;
    Ok(Json(ApiResponse::ok(template)).into_response())
}

#[utoipa::path(
    put,
    path = "/api/academic/timetable-templates/{id}",
    operation_id = "updateTimetableTemplate",
    params(("id" = Uuid, Path, description = "Timetable template ID")),
    request_body = UpdateTemplateRequest,
    responses(
        (status = 200, description = "Updated timetable template", body = ApiResponse<crate::modules::academic::models::timetable::TimetableTemplate>),
        (status = 400, description = "Invalid timetable template", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable template manage permission denied", body = ApiErrorResponse),
        (status = 404, description = "Timetable template not found", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn update_template(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::LEARNING_OFFERING_MANAGE_SCHOOL)?;
    let template =
        timetable_template_service::update_template(&context.tenant.pool, id, payload).await?;
    Ok(Json(ApiResponse::ok(template)).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/academic/timetable-templates/{id}",
    operation_id = "deleteTimetableTemplate",
    params(("id" = Uuid, Path, description = "Timetable template ID")),
    responses(
        (status = 200, description = "Deleted timetable template", body = ApiResponse<EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable template manage permission denied", body = ApiErrorResponse),
        (status = 404, description = "Timetable template not found", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn delete_template(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::LEARNING_OFFERING_MANAGE_SCHOOL)?;
    timetable_template_service::delete_template(&context.tenant.pool, id).await?;
    Ok(Json(ApiResponse::empty()).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/timetable-templates/from-current",
    operation_id = "createTimetableTemplateFromCurrent",
    request_body = FromCurrentRequest,
    responses(
        (status = 200, description = "Created timetable template from selected term", body = ApiResponse<crate::modules::academic::models::timetable::TimetableTemplate>),
        (status = 400, description = "Invalid timetable template", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable template manage permission denied", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn from_current(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<FromCurrentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::LEARNING_OFFERING_MANAGE_SCHOOL)?;
    let template = timetable_template_service::from_current(
        &context.tenant.pool,
        context.actor.user_id,
        payload,
    )
    .await?;
    Ok(Json(ApiResponse::ok(template)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/academic/timetable-templates/{template_id}/apply",
    operation_id = "applyTimetableTemplate",
    params(("template_id" = Uuid, Path, description = "Timetable template ID")),
    request_body = ApplyTemplateRequest,
    responses(
        (status = 200, description = "Applied timetable template", body = ApiResponse<crate::modules::academic::models::timetable::TemplateApplyResult>),
        (status = 400, description = "Template cannot be applied", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable template manage permission denied", body = ApiErrorResponse),
        (status = 409, description = "Timetable conflict", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn apply_template(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Path(template_id): Path<Uuid>,
    Json(payload): Json<ApplyTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::LEARNING_OFFERING_MANAGE_SCHOOL)?;
    let academic_term_id = payload.academic_term_id;
    let result = timetable_template_service::apply_template(
        &context.tenant.pool,
        context.actor.user_id,
        template_id,
        payload,
    )
    .await?;
    broadcast_reload(
        &state,
        &headers,
        context.actor.user_id,
        academic_term_id,
        result.applied as i64,
    );
    Ok(Json(ApiResponse::ok(result)).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/academic/timetable/clear",
    operation_id = "clearTimetable",
    request_body = ClearTimetableRequest,
    responses(
        (status = 200, description = "Cleared timetable entries", body = ApiResponse<Vec<crate::modules::academic::models::timetable::TimetableEntry>>),
        (status = 400, description = "Invalid clear request", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Timetable manage permission denied", body = ApiErrorResponse)
    ),
    tag = "academic"
)]
pub async fn clear_timetable(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    headers: HeaderMap,
    Json(payload): Json<ClearTimetableRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session).await?;
    context
        .actor
        .require_permission(codes::LEARNING_OFFERING_MANAGE_SCHOOL)?;
    let academic_term_id = payload.academic_term_id;
    let entries = timetable_template_service::clear_timetable(
        &context.tenant.pool,
        context.actor.user_id,
        payload,
    )
    .await?;
    let revision = entries
        .iter()
        .map(|entry| entry.row_version)
        .max()
        .unwrap_or(0);
    broadcast_reload(
        &state,
        &headers,
        context.actor.user_id,
        academic_term_id,
        revision,
    );
    Ok(Json(ApiResponse::ok(entries)).into_response())
}

fn broadcast_reload(
    state: &AppState,
    headers: &HeaderMap,
    user_id: Uuid,
    academic_term_id: Uuid,
    revision: i64,
) {
    let subdomain =
        extract_subdomain_from_request(headers).unwrap_or_else(|_| "default".to_string());
    state.websocket_manager.broadcast_mutation(
        subdomain,
        academic_term_id,
        TimetableEvent::TimetableChanged {
            user_id,
            academic_term_id,
            learning_group_id: None,
            revision,
        },
    );
}
