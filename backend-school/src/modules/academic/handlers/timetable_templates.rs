use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::services::timetable_template_service;
use crate::modules::academic::websockets::TimetableEvent;
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct CreateTemplateRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct FromCurrentRequest {
    pub semester_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub entry_types: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct ApplyTemplateRequest {
    pub semester_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct ClearTimetableRequest {
    pub semester_id: Uuid,
    pub entry_types: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTemplateRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

fn default_non_course_types() -> Vec<String> {
    vec![
        "BREAK".into(),
        "HOMEROOM".into(),
        "ACTIVITY".into(),
        "ACADEMIC".into(),
    ]
}

pub async fn list_templates(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let rows = timetable_template_service::list_templates(&pool).await?;
    Ok(Json(serde_json::json!({ "success": true, "data": rows })).into_response())
}

pub async fn get_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_READ_ALL)?;
    let (template, entries) = timetable_template_service::get_template(&pool, id).await?;
    Ok(Json(serde_json::json!({ "success": true, "data": { "template": template, "entries": entries } })).into_response())
}

pub async fn create_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    let user_id = actor.user_id;
    let id = timetable_template_service::create_template(
        &pool,
        &payload.name,
        payload.description.as_deref(),
        Some(user_id),
    )
    .await?;
    Ok(Json(serde_json::json!({ "success": true, "data": { "id": id } })).into_response())
}

pub async fn update_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(payload): Json<UpdateTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    timetable_template_service::update_template(
        &pool,
        id,
        payload.name.as_deref(),
        payload.description.as_deref(),
    )
    .await?;
    Ok(Json(serde_json::json!({ "success": true, "data": {} })).into_response())
}

pub async fn delete_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    timetable_template_service::delete_template(&pool, id).await?;
    Ok(Json(serde_json::json!({ "success": true, "data": {} })).into_response())
}

pub async fn from_current(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<FromCurrentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    let user_id = actor.user_id;
    let entry_types = payload.entry_types.unwrap_or_else(default_non_course_types);

    let id = timetable_template_service::from_current(
        &pool,
        payload.semester_id,
        &payload.name,
        payload.description.as_deref(),
        entry_types,
        Some(user_id),
    )
    .await?;

    Ok(Json(serde_json::json!({ "success": true, "data": { "id": id } })).into_response())
}

pub async fn apply_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(template_id): Path<Uuid>,
    Json(payload): Json<ApplyTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    let user_id = actor.user_id;
    let total_inserted = timetable_template_service::apply_template(
        &pool,
        template_id,
        payload.semester_id,
        Some(user_id),
    )
    .await?;

    let subdomain =
        extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    state.websocket_manager.broadcast_mutation(
        subdomain,
        payload.semester_id,
        TimetableEvent::TableRefresh { user_id },
    );

    Ok(
        Json(serde_json::json!({ "success": true, "data": { "applied": total_inserted } }))
            .into_response(),
    )
}

pub async fn clear_timetable(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<ClearTimetableRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL)?;
    let user_id = actor.user_id;
    let entry_types = payload.entry_types.unwrap_or_else(default_non_course_types);
    let deleted =
        timetable_template_service::clear_timetable(&pool, payload.semester_id, entry_types)
            .await?;

    let subdomain =
        extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    state.websocket_manager.broadcast_mutation(
        subdomain,
        payload.semester_id,
        TimetableEvent::TableRefresh { user_id },
    );

    Ok(
        Json(serde_json::json!({ "success": true, "data": { "deleted": deleted } }))
            .into_response(),
    )
}
