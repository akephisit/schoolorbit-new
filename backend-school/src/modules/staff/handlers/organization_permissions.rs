use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::staff::models::UpdateOrganizationPermissionsRequest;
use crate::modules::staff::services::organization_permission_service;
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

// GET /api/organization/units/{id}/permissions
pub async fn get_organization_permissions(
    State(state): State<AppState>,
    Path(organization_unit_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_READ_ALL)?;

    let grants = organization_permission_service::list_organization_permission_grants(
        &pool,
        organization_unit_id,
    )
    .await?;

    Ok(Json(ApiResponse::ok(grants)))
}

// PUT /api/organization/units/{id}/permissions
pub async fn update_organization_permissions(
    State(state): State<AppState>,
    Path(organization_unit_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<UpdateOrganizationPermissionsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let tenant = context.tenant.subdomain.clone();
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_UPDATE_ALL)?;

    organization_permission_service::replace_organization_permission_grants(
        &pool,
        organization_unit_id,
        payload.grants,
    )
    .await?;

    // Organization permission grants changed; all cached effective permissions are stale.
    state.permission_cache.invalidate_tenant(&tenant);
    state.notify_all_permissions_changed(&tenant);

    Ok(Json(ApiResponse::empty_with_message(
        "Update organization permissions successfully",
    )))
}
