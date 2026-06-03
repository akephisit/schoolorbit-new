use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

use crate::db::school_mapping::get_school_database_url;
use crate::error::AppError;
use crate::middleware::permission::get_user_with_permissions;
use crate::modules::staff::services::delegation_service;
use crate::permissions::registry::codes;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;

#[derive(Serialize)]
pub struct DelegationItem {
    pub id: Uuid,
    pub from_user_id: Uuid,
    pub from_user_name: String,
    pub to_user_id: Uuid,
    pub to_user_name: String,
    pub permission_id: Uuid,
    pub permission_code: String,
    pub permission_name: String,
    pub reason: Option<String>,
    pub started_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
pub struct CreateDelegationRequest {
    pub to_user_id: Uuid,
    pub permission_id: Uuid,
    pub reason: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain).await
        .map_err(|_| AppError::NotFound("ไม่พบโรงเรียน".to_string()))?;
    state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|_| AppError::InternalServerError("Database connection error".to_string()))
}

pub async fn list_delegatable_permissions(
    State(state): State<AppState>,
    Path(department_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let (_, permissions) = match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
        Ok(r) => r, Err(resp) => return Ok(resp),
    };
    let has_access = permissions.contains(&"*".to_string()) || permissions.contains(&codes::DEPT_WORK_APPROVE.to_string());
    if !has_access {
        return Ok((StatusCode::FORBIDDEN, Json(json!({ "success": false, "error": "ไม่มีสิทธิ์" }))).into_response());
    }

    let perms = delegation_service::list_delegatable_permissions(&pool, department_id).await?;
    Ok(Json(json!({ "success": true, "data": perms })).into_response())
}

pub async fn list_delegations(
    State(state): State<AppState>,
    Path(department_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let (_, permissions) = match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
        Ok(r) => r, Err(resp) => return Ok(resp),
    };
    let has_access = permissions.contains(&"*".to_string()) || permissions.contains(&codes::DEPT_WORK_APPROVE.to_string());
    if !has_access {
        return Ok((StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "ไม่มีสิทธิ์มอบหมายงานในกลุ่มนี้" }))).into_response());
    }

    let delegations = delegation_service::list_delegations(&pool, department_id).await?;
    Ok(Json(json!({ "success": true, "data": delegations })).into_response())
}

pub async fn create_delegation(
    State(state): State<AppState>,
    Path(department_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<CreateDelegationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let (user_id, permissions) = match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
        Ok(r) => r, Err(resp) => return Ok(resp),
    };
    let has_access = permissions.contains(&"*".to_string()) || permissions.contains(&codes::DEPT_WORK_APPROVE.to_string());
    if !has_access {
        return Ok((StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "ไม่มีสิทธิ์มอบหมายงานในกลุ่มนี้" }))).into_response());
    }

    if !delegation_service::is_department_head(&pool, user_id, department_id).await? {
        return Ok((StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "เฉพาะหัวหน้าหรือรองหัวหน้ากลุ่มเท่านั้นที่สามารถมอบหมายสิทธิ์ได้" }))).into_response());
    }

    if !delegation_service::is_department_member(&pool, body.to_user_id, department_id).await? {
        return Ok((StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "ผู้รับมอบหมายต้องเป็นสมาชิกของกลุ่มนี้" }))).into_response());
    }

    let id = delegation_service::create_delegation(
        &pool, user_id, body.to_user_id, body.permission_id, department_id, body.reason, body.expires_at,
    ).await?;

    state.permission_cache.invalidate(&body.to_user_id);

    Ok(Json(json!({ "success": true, "data": { "delegation_id": id } })).into_response())
}

pub async fn revoke_delegation(
    State(state): State<AppState>,
    Path(delegation_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let (user_id, permissions) = match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
        Ok(r) => r, Err(resp) => return Ok(resp),
    };

    let (from_user_id, to_user_id) = match delegation_service::get_delegation_users(&pool, delegation_id).await? {
        Some(t) => t,
        None => return Ok((StatusCode::NOT_FOUND,
            Json(json!({ "success": false, "error": "ไม่พบการมอบหมายสิทธิ์นี้" }))).into_response()),
    };

    let can_revoke = user_id == from_user_id || permissions.contains(&"*".to_string());
    if !can_revoke {
        return Ok((StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "ไม่มีสิทธิ์ยกเลิกการมอบหมายนี้" }))).into_response());
    }

    delegation_service::revoke_delegation(&pool, delegation_id).await?;
    state.permission_cache.invalidate(&to_user_id);

    Ok(Json(json!({ "success": true, "data": {} })).into_response())
}
