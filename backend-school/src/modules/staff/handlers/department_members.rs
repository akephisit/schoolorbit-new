use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::staff::services::department_member_service;
use crate::permissions::registry::codes;
use crate::utils::request_context::{actor_tenant_context, tenant_pool};
use crate::AppState;

#[derive(Serialize)]
pub struct DeptMemberItem {
    pub user_id: Uuid,
    pub department_id: Uuid,
    pub department_name: String,
    pub name: String,
    pub title: String,
    pub position: String,
    pub is_primary: bool,
    pub responsibilities: Option<String>,
    pub started_at: NaiveDate,
}

#[derive(Deserialize)]
pub struct ListMembersQuery {
    pub include_children: Option<bool>,
}

#[derive(Deserialize)]
pub struct AddMemberRequest {
    pub user_id: Uuid,
    pub position: String,
    pub is_primary: Option<bool>,
    pub responsibilities: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateMemberRequest {
    pub position: String,
    pub is_primary: Option<bool>,
    pub responsibilities: Option<String>,
    pub new_department_id: Option<Uuid>,
}

pub async fn list_members(
    State(state): State<AppState>,
    Path(department_id): Path<Uuid>,
    Query(query): Query<ListMembersQuery>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let members = department_member_service::list_members(
        &pool,
        department_id,
        query.include_children.unwrap_or(false),
    )
    .await?;
    Ok(Json(json!({ "success": true, "data": members })).into_response())
}

pub async fn add_member(
    State(state): State<AppState>,
    Path(department_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<AddMemberRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_ASSIGN_ALL)?;

    if department_member_service::already_member(&pool, body.user_id, department_id).await? {
        return Ok(
            Json(json!({ "success": false, "error": "บุคลากรนี้เป็นสมาชิกของกลุ่มนี้อยู่แล้ว" }))
                .into_response(),
        );
    }

    department_member_service::add_member(
        &pool,
        body.user_id,
        department_id,
        &body.position,
        body.is_primary.unwrap_or(false),
        body.responsibilities,
    )
    .await?;

    state.permission_cache.invalidate(&body.user_id);
    state.notify_permission_changed(body.user_id);
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

pub async fn update_member(
    State(state): State<AppState>,
    Path((department_id, user_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(body): Json<UpdateMemberRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_ASSIGN_ALL)?;

    let target_dept = body.new_department_id.unwrap_or(department_id);
    let updated = department_member_service::update_member(
        &pool,
        department_id,
        user_id,
        &body.position,
        body.is_primary.unwrap_or(false),
        body.responsibilities,
        target_dept,
    )
    .await?;

    if updated == 0 {
        return Ok(Json(json!({ "success": false, "error": "ไม่พบสมาชิกนี้ในกลุ่ม" })).into_response());
    }

    state.permission_cache.invalidate(&user_id);
    state.notify_permission_changed(user_id);
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}

pub async fn remove_member(
    State(state): State<AppState>,
    Path((department_id, user_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_ASSIGN_ALL)?;
    department_member_service::remove_member(&pool, department_id, user_id).await?;
    state.permission_cache.invalidate(&user_id);
    state.notify_permission_changed(user_id);
    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}
