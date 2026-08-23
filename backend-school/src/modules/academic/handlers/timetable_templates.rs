use axum::{
    extract::{Extension, Path, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

use crate::api_response::ApiResponse;
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
