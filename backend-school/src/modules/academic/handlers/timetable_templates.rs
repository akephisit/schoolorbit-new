use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::check_permission;
use crate::modules::academic::services::timetable_template_service;
use crate::modules::academic::websockets::TimetableEvent;
use crate::permissions::registry::codes;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    resolve_tenant_pool(state, headers).await
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TimetableTemplateView {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub entry_count: i64,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct TimetableTemplateEntry {
    pub id: Uuid,
    pub template_id: Uuid,
    pub day_of_week: String,
    pub period_id: Uuid,
    pub entry_type: String,
    pub title: Option<String>,
    pub activity_slot_id: Option<Uuid>,
    pub grade_level_ids: serde_json::Value,
    pub classroom_ids: serde_json::Value,
    pub instructor_ids: serde_json::Value,
    pub room_id: Option<Uuid>,
}

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
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_READ_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let rows = timetable_template_service::list_templates(&pool).await?;
    Ok(Json(serde_json::json!({ "success": true, "data": rows })).into_response())
}

pub async fn get_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_READ_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let (template, entries) = timetable_template_service::get_template(&pool, id).await?;
    Ok(Json(serde_json::json!({ "success": true, "data": { "template": template, "entries": entries } })).into_response())
}

pub async fn create_template(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
        .await
        .ok();
    let id = timetable_template_service::create_template(
        &pool,
        &payload.name,
        payload.description.as_deref(),
        user_id,
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
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
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
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    timetable_template_service::delete_template(&pool, id).await?;
    Ok(Json(serde_json::json!({ "success": true, "data": {} })).into_response())
}

pub async fn from_current(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<FromCurrentRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
        .await
        .ok();
    let entry_types = payload.entry_types.unwrap_or_else(default_non_course_types);

    let id = timetable_template_service::from_current(
        &pool,
        payload.semester_id,
        &payload.name,
        payload.description.as_deref(),
        entry_types,
        user_id,
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
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
        .await
        .ok();
    let total_inserted = timetable_template_service::apply_template(
        &pool,
        template_id,
        payload.semester_id,
        user_id,
    )
    .await?;

    let subdomain =
        extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    state.websocket_manager.broadcast_mutation(
        subdomain,
        payload.semester_id,
        TimetableEvent::TableRefresh {
            user_id: user_id.unwrap_or_default(),
        },
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
    let pool = get_pool(&state, &headers).await?;
    if let Err(r) = check_permission(
        &headers,
        &pool,
        codes::ACADEMIC_COURSE_PLAN_MANAGE_ALL,
        &state.permission_cache,
    )
    .await
    {
        return Ok(r);
    }
    let user_id = crate::middleware::auth::extract_user_id(&headers, &pool)
        .await
        .ok();
    let entry_types = payload.entry_types.unwrap_or_else(default_non_course_types);
    let deleted =
        timetable_template_service::clear_timetable(&pool, payload.semester_id, entry_types)
            .await?;

    let subdomain =
        extract_subdomain_from_request(&headers).unwrap_or_else(|_| "default".to_string());
    state.websocket_manager.broadcast_mutation(
        subdomain,
        payload.semester_id,
        TimetableEvent::TableRefresh {
            user_id: user_id.unwrap_or_default(),
        },
    );

    Ok(
        Json(serde_json::json!({ "success": true, "data": { "deleted": deleted } }))
            .into_response(),
    )
}
