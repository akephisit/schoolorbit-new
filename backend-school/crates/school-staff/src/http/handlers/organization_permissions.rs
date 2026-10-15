use crate::http::state::StaffHttpState;
use crate::models::UpdateOrganizationPermissionsRequest;
use crate::services::organization_permission_service;
#[cfg(feature = "api-docs")]
use crate::services::organization_permission_service::OrganizationPermissionGrant;
use school_auth::session_service::AuthenticatedSession;
use school_auth_http::context::actor_tenant_context_from_session;
use school_http::ApiResponse;
use school_http::HttpError as AppError;
#[cfg(feature = "api-docs")]
use school_http::{ApiErrorResponse, EmptyData};
use school_permissions::registry::codes;

use axum::{
    extract::{Extension, Path, State},
    response::IntoResponse,
    Json,
};
use uuid::Uuid;

// GET /api/organization/units/{id}/permissions
#[cfg_attr(feature = "api-docs", utoipa::path(
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
))]
pub async fn get_organization_permissions(
    State(state): State<StaffHttpState>,
    Path(organization_unit_id): Path<Uuid>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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
#[cfg_attr(feature = "api-docs", utoipa::path(
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
))]
pub async fn update_organization_permissions(
    State(state): State<StaffHttpState>,
    Path(organization_unit_id): Path<Uuid>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<UpdateOrganizationPermissionsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state.auth_runtime, &session).await?;
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
    state.auth_runtime.invalidate_permission_tenant(&tenant);
    state.auth_runtime.notify_all_permissions_changed(&tenant);

    Ok(Json(ApiResponse::empty_with_message(
        "Update organization permissions successfully",
    )))
}
