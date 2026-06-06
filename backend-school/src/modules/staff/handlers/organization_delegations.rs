use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::staff::services::organization_delegation_service;
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context;
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

#[derive(Serialize)]
struct DelegationIdData {
    delegation_id: Uuid,
}

pub async fn list_delegatable_permissions(
    State(state): State<AppState>,
    Path(organization_unit_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let can_approve_organization_work =
        actor.has_permission(codes::ORGANIZATION_WORK_APPROVE_ORGANIZATION_UNIT);
    if !can_approve_organization_work {
        return Ok((StatusCode::FORBIDDEN, Json(ApiErrorResponse::new("ไม่มีสิทธิ์"))).into_response());
    }

    let perms =
        organization_delegation_service::list_delegatable_permissions(&pool, organization_unit_id)
            .await?;
    Ok(Json(ApiResponse::ok(perms)).into_response())
}

pub async fn list_delegations(
    State(state): State<AppState>,
    Path(organization_unit_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let can_approve_organization_work =
        actor.has_permission(codes::ORGANIZATION_WORK_APPROVE_ORGANIZATION_UNIT);
    if !can_approve_organization_work {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(ApiErrorResponse::new("ไม่มีสิทธิ์มอบหมายงานในกลุ่มนี้")),
        )
            .into_response());
    }

    let delegations =
        organization_delegation_service::list_delegations(&pool, organization_unit_id).await?;
    Ok(Json(ApiResponse::ok(delegations)).into_response())
}

pub async fn create_delegation(
    State(state): State<AppState>,
    Path(organization_unit_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<CreateDelegationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let can_approve_organization_work =
        actor.has_permission(codes::ORGANIZATION_WORK_APPROVE_ORGANIZATION_UNIT);
    if !can_approve_organization_work {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(ApiErrorResponse::new("ไม่มีสิทธิ์มอบหมายงานในกลุ่มนี้")),
        )
            .into_response());
    }

    if !organization_delegation_service::is_organization_unit_leader(
        &pool,
        actor.user_id,
        organization_unit_id,
    )
    .await?
    {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(ApiErrorResponse::new(
                "เฉพาะหัวหน้าหรือรองหัวหน้าหน่วยงานเท่านั้นที่สามารถมอบหมายสิทธิ์ได้",
            )),
        )
            .into_response());
    }

    if !organization_delegation_service::is_organization_member(
        &pool,
        body.to_user_id,
        organization_unit_id,
    )
    .await?
    {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse::new("ผู้รับมอบหมายต้องเป็นสมาชิกของหน่วยงานนี้")),
        )
            .into_response());
    }

    if !organization_delegation_service::organization_permission_grant_exists(
        &pool,
        organization_unit_id,
        body.permission_id,
    )
    .await?
    {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse::new("สิทธิ์นี้ไม่ได้ถูกกำหนดให้หน่วยงานนี้มอบหมายได้")),
        )
            .into_response());
    }

    let id = organization_delegation_service::create_delegation(
        &pool,
        actor.user_id,
        body.to_user_id,
        body.permission_id,
        organization_unit_id,
        body.reason,
        body.expires_at,
    )
    .await?;

    state.permission_cache.invalidate(&body.to_user_id);
    state.notify_permission_changed(body.to_user_id);

    Ok(Json(ApiResponse::ok(DelegationIdData { delegation_id: id })).into_response())
}

pub async fn revoke_delegation(
    State(state): State<AppState>,
    Path(delegation_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;

    let (from_user_id, to_user_id) =
        match organization_delegation_service::get_delegation_users(&pool, delegation_id).await? {
            Some(t) => t,
            None => {
                return Ok((
                    StatusCode::NOT_FOUND,
                    Json(ApiErrorResponse::new("ไม่พบการมอบหมายสิทธิ์นี้")),
                )
                    .into_response())
            }
        };

    let can_revoke = actor.user_id == from_user_id || actor.has_permission(codes::WILDCARD);
    if !can_revoke {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(ApiErrorResponse::new("ไม่มีสิทธิ์ยกเลิกการมอบหมายนี้")),
        )
            .into_response());
    }

    organization_delegation_service::revoke_delegation(&pool, delegation_id).await?;
    state.permission_cache.invalidate(&to_user_id);
    state.notify_permission_changed(to_user_id);

    Ok(Json(ApiResponse::empty()).into_response())
}
