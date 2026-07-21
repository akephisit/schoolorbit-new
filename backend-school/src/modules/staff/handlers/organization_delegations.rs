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
use crate::policies::organization_access_policy;
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
    organization_access_policy::can_approve_organization_work(&pool, &actor, organization_unit_id)
        .await?;

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
    organization_access_policy::can_approve_organization_work(&pool, &actor, organization_unit_id)
        .await?;

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
    let tenant = context.tenant.subdomain.clone();
    let pool = context.tenant.pool;
    let actor = context.actor;
    organization_access_policy::can_approve_organization_work(&pool, &actor, organization_unit_id)
        .await?;

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

    let user_id = body.to_user_id;
    state.permission_cache.invalidate_user(&tenant, user_id);
    state.notify_permission_changed(&tenant, user_id);

    Ok(Json(ApiResponse::ok(DelegationIdData { delegation_id: id })).into_response())
}

pub async fn revoke_delegation(
    State(state): State<AppState>,
    Path(delegation_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let tenant = context.tenant.subdomain.clone();
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

    if !organization_access_policy::can_revoke_organization_delegation(&actor, from_user_id) {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(ApiErrorResponse::new("ไม่มีสิทธิ์ยกเลิกการมอบหมายนี้")),
        )
            .into_response());
    }

    organization_delegation_service::revoke_delegation(&pool, delegation_id).await?;
    state.permission_cache.invalidate_user(&tenant, to_user_id);
    state.notify_permission_changed(&tenant, to_user_id);

    Ok(Json(ApiResponse::empty()).into_response())
}
