use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
#[cfg(feature = "api-docs")]
use utoipa::ToSchema;
use uuid::Uuid;

use crate::http::state::StaffHttpState;
#[cfg(feature = "api-docs")]
use crate::models::DelegationItem;
use crate::organization_access_policy;
use crate::services::organization_delegation_service;
#[cfg(feature = "api-docs")]
use crate::services::organization_delegation_service::DelegatablePermission;
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
#[cfg(feature = "api-docs")]
use school_http::EmptyData;
use school_http::HttpError as AppError;
use school_http::{ApiErrorResponse, ApiResponse};

#[derive(Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct CreateDelegationRequest {
    pub to_user_id: Uuid,
    pub permission_id: Uuid,
    pub reason: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct DelegationIdData {
    pub delegation_id: Uuid,
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    get,
    path = "/api/organization/units/{id}/delegatable-permissions",
    operation_id = "listDelegatablePermissions",
    tag = "organization",
    params(("id" = Uuid, Path, description = "Organization unit ID")),
    responses(
        (status = 200, description = "Permissions this unit may delegate", body = ApiResponse<Vec<DelegatablePermission>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Organization approval access required", body = ApiErrorResponse)
    )
))]
pub async fn list_delegatable_permissions(
    State(state): State<StaffHttpState>,
    Path(organization_unit_id): Path<Uuid>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    organization_access_policy::can_approve_organization_work(&pool, &actor, organization_unit_id)
        .await?;

    let perms =
        organization_delegation_service::list_delegatable_permissions(&pool, organization_unit_id)
            .await?;
    Ok(Json(ApiResponse::ok(perms)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    get,
    path = "/api/organization/units/{id}/delegations",
    operation_id = "listOrganizationDelegations",
    tag = "organization",
    params(("id" = Uuid, Path, description = "Organization unit ID")),
    responses(
        (status = 200, description = "Active organization delegations", body = ApiResponse<Vec<DelegationItem>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Organization approval access required", body = ApiErrorResponse)
    )
))]
pub async fn list_delegations(
    State(state): State<StaffHttpState>,
    Path(organization_unit_id): Path<Uuid>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    organization_access_policy::can_approve_organization_work(&pool, &actor, organization_unit_id)
        .await?;

    let delegations =
        organization_delegation_service::list_delegations(&pool, organization_unit_id).await?;
    Ok(Json(ApiResponse::ok(delegations)).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    post,
    path = "/api/organization/units/{id}/delegations",
    operation_id = "createOrganizationDelegation",
    tag = "organization",
    params(("id" = Uuid, Path, description = "Organization unit ID")),
    request_body = CreateDelegationRequest,
    responses(
        (status = 200, description = "Delegation created", body = ApiResponse<DelegationIdData>),
        (status = 400, description = "Invalid delegation target or permission", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Organization approval access required", body = ApiErrorResponse)
    )
))]
pub async fn create_delegation(
    State(state): State<StaffHttpState>,
    Path(organization_unit_id): Path<Uuid>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(body): Json<CreateDelegationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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
    state
        .auth_runtime
        .invalidate_permission_user(&tenant, user_id);
    state
        .auth_runtime
        .notify_permission_changed(&tenant, user_id);

    Ok(Json(ApiResponse::ok(DelegationIdData { delegation_id: id })).into_response())
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    delete,
    path = "/api/organization/delegations/{id}",
    operation_id = "revokeOrganizationDelegation",
    tag = "organization",
    params(("id" = Uuid, Path, description = "Delegation ID")),
    responses(
        (status = 200, description = "Delegation revoked", body = ApiResponse<EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Delegation ownership required", body = ApiErrorResponse),
        (status = 404, description = "Delegation not found", body = ApiErrorResponse)
    )
))]
pub async fn revoke_delegation(
    State(state): State<StaffHttpState>,
    Path(delegation_id): Path<Uuid>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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
    state
        .auth_runtime
        .invalidate_permission_user(&tenant, to_user_id);
    state
        .auth_runtime
        .notify_permission_changed(&tenant, to_user_id);

    Ok(Json(ApiResponse::empty()).into_response())
}
