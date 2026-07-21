use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::api_response::{ApiErrorResponse, ApiResponse, EmptyData};
use crate::error::AppError;
use crate::modules::staff::services::organization_member_service;
use crate::permissions::registry::codes;
use crate::utils::request_context::{actor_tenant_context, tenant_pool};
use crate::AppState;

#[derive(Serialize, ToSchema)]
pub struct OrganizationMemberItem {
    pub user_id: Uuid,
    pub organization_unit_id: Uuid,
    pub organization_unit_name: String,
    pub name: String,
    pub title: String,
    pub position_code: String,
    #[schema(required = true)]
    pub position_title: Option<String>,
    pub is_primary: bool,
    #[schema(required = true)]
    pub responsibilities: Option<String>,
    pub started_at: NaiveDate,
}

#[derive(Deserialize, ToSchema)]
pub struct ListMembersQuery {
    pub include_children: Option<bool>,
}

#[derive(Deserialize, ToSchema)]
pub struct AddMemberRequest {
    pub user_id: Uuid,
    pub position_code: String,
    pub position_title: Option<String>,
    pub is_primary: Option<bool>,
    pub responsibilities: Option<String>,
}

#[derive(Deserialize, ToSchema)]
pub struct UpdateMemberRequest {
    pub position_code: String,
    pub position_title: Option<String>,
    pub is_primary: Option<bool>,
    pub responsibilities: Option<String>,
    pub new_organization_unit_id: Option<Uuid>,
}

#[utoipa::path(
    get,
    path = "/api/organization/units/{id}/members",
    operation_id = "listOrganizationMembers",
    tag = "organization",
    params(
        ("id" = Uuid, Path, description = "Organization unit ID"),
        ("include_children" = Option<bool>, Query, description = "Include direct child units")
    ),
    responses(
        (status = 200, description = "Organization members", body = ApiResponse<Vec<OrganizationMemberItem>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse)
    )
)]
pub async fn list_members(
    State(state): State<AppState>,
    Path(organization_unit_id): Path<Uuid>,
    Query(query): Query<ListMembersQuery>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = tenant_pool(&state, &headers).await?;
    let members = organization_member_service::list_members(
        &pool,
        organization_unit_id,
        query.include_children.unwrap_or(false),
    )
    .await?;
    Ok(Json(ApiResponse::ok(members)).into_response())
}

#[utoipa::path(
    post,
    path = "/api/organization/units/{id}/members",
    operation_id = "addOrganizationMember",
    tag = "organization",
    params(("id" = Uuid, Path, description = "Organization unit ID")),
    request_body = AddMemberRequest,
    responses(
        (status = 200, description = "Organization member added", body = ApiResponse<EmptyData>),
        (status = 400, description = "User is already a member", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn add_member(
    State(state): State<AppState>,
    Path(organization_unit_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<AddMemberRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let tenant = context.tenant.subdomain.clone();
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_ASSIGN_ALL)?;

    if organization_member_service::already_member(&pool, body.user_id, organization_unit_id)
        .await?
    {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(ApiErrorResponse::new("บุคลากรนี้เป็นสมาชิกของหน่วยงานนี้อยู่แล้ว")),
        )
            .into_response());
    }

    organization_member_service::add_member(
        &pool,
        body.user_id,
        organization_unit_id,
        &body.position_code,
        body.position_title,
        body.is_primary.unwrap_or(false),
        body.responsibilities,
    )
    .await?;

    let user_id = body.user_id;
    state.permission_cache.invalidate_user(&tenant, user_id);
    state.notify_permission_changed(&tenant, user_id);
    Ok(Json(ApiResponse::empty()).into_response())
}

#[utoipa::path(
    put,
    path = "/api/organization/units/{id}/members/{user_id}",
    operation_id = "updateOrganizationMember",
    tag = "organization",
    params(
        ("id" = Uuid, Path, description = "Organization unit ID"),
        ("user_id" = Uuid, Path, description = "User ID")
    ),
    request_body = UpdateMemberRequest,
    responses(
        (status = 200, description = "Organization member updated", body = ApiResponse<EmptyData>),
        (status = 400, description = "Invalid membership", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse),
        (status = 404, description = "Membership not found", body = ApiErrorResponse)
    )
)]
pub async fn update_member(
    State(state): State<AppState>,
    Path((organization_unit_id, user_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(body): Json<UpdateMemberRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let tenant = context.tenant.subdomain.clone();
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_ASSIGN_ALL)?;

    let target_unit = body
        .new_organization_unit_id
        .unwrap_or(organization_unit_id);
    let updated = organization_member_service::update_member(
        &pool,
        organization_member_service::UpdateMemberInput {
            organization_unit_id,
            user_id,
            position_code: body.position_code,
            position_title: body.position_title,
            is_primary: body.is_primary.unwrap_or(false),
            responsibilities: body.responsibilities,
            new_organization_unit_id: target_unit,
        },
    )
    .await?;

    if updated == 0 {
        return Ok((
            StatusCode::NOT_FOUND,
            Json(ApiErrorResponse::new("ไม่พบสมาชิกนี้ในหน่วยงาน")),
        )
            .into_response());
    }

    state.permission_cache.invalidate_user(&tenant, user_id);
    state.notify_permission_changed(&tenant, user_id);
    Ok(Json(ApiResponse::empty()).into_response())
}

#[utoipa::path(
    delete,
    path = "/api/organization/units/{id}/members/{user_id}",
    operation_id = "removeOrganizationMember",
    tag = "organization",
    params(
        ("id" = Uuid, Path, description = "Organization unit ID"),
        ("user_id" = Uuid, Path, description = "User ID")
    ),
    responses(
        (status = 200, description = "Organization member removed", body = ApiResponse<EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Permission denied", body = ApiErrorResponse)
    )
)]
pub async fn remove_member(
    State(state): State<AppState>,
    Path((organization_unit_id, user_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers).await?;
    let tenant = context.tenant.subdomain.clone();
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::ROLES_ASSIGN_ALL)?;
    organization_member_service::remove_member(&pool, organization_unit_id, user_id).await?;
    state.permission_cache.invalidate_user(&tenant, user_id);
    state.notify_permission_changed(&tenant, user_id);
    Ok(Json(ApiResponse::empty()).into_response())
}
