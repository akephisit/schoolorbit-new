use crate::api_response::{ApiErrorResponse, ApiResponse, EmptyData};
use crate::error::AppError;
use crate::modules::staff::models::UpdateOrganizationPermissionsRequest;
use crate::modules::staff::services::organization_permission_service::{
    self, OrganizationPermissionGrant,
};
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
#[utoipa::path(
    get,
    path = "/api/organization/units/{id}/permissions",
    operation_id = "getOrganizationPermissions",
    tag = "organization",
    params(("id" = Uuid, Path, description = "Organization unit ID")),
    responses(
        (status = 200, description = "Organization permission grants", body = ApiResponse<Vec<OrganizationPermissionGrant>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
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
#[utoipa::path(
    put,
    path = "/api/organization/units/{id}/permissions",
    operation_id = "updateOrganizationPermissions",
    tag = "organization",
    params(("id" = Uuid, Path, description = "Organization unit ID")),
    request_body = UpdateOrganizationPermissionsRequest,
    responses(
        (status = 200, description = "Organization permission grants replaced", body = ApiResponse<EmptyData>),
        (status = 400, description = "Invalid permission grants", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
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
