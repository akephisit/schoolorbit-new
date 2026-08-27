use crate::api_response::{ApiErrorResponse, ApiResponse, EmptyData};
use crate::error::AppError;
use crate::modules::auth::session_service::AuthenticatedSession;
use crate::modules::menu::models::{
    AcademicMenuTemplateApplyResult, AcademicMenuTemplatePreview, MenuGroup, MenuItem,
    MenuWorkspace,
};
use crate::modules::menu::services::{academic_template_service, menu_service};
use crate::permissions::registry::codes;
use crate::utils::request_context::{actor_tenant_context_from_session, ActorTenantContext};
use crate::AppState;

use axum::{
    extract::{Extension, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json as JsonResponse},
};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMenuWorkspaceRequest {
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub display_order: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMenuWorkspaceRequest {
    pub name: Option<String>,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub display_order: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMenuGroupRequest {
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub workspace_code: String,
    pub display_order: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMenuGroupRequest {
    pub name: Option<String>,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub workspace_code: Option<String>,
    pub display_order: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateMenuItemRequest {
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub path: String,
    pub icon: Option<String>,
    pub group_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub required_permission: Option<String>,
    pub display_order: Option<i32>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateMenuItemRequest {
    pub name: Option<String>,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub path: Option<String>,
    pub icon: Option<String>,
    pub group_id: Option<Uuid>,
    pub parent_id: Option<Uuid>,
    pub required_permission: Option<String>,
    pub display_order: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReorderRequest {
    pub items: Vec<ReorderItem>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReorderItem {
    pub id: Uuid,
    pub display_order: i32,
    pub group_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct MenuItemFilter {
    pub group_id: Option<Uuid>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReorderGroupsRequest {
    pub groups: Vec<ReorderItem>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct ReorderWorkspacesRequest {
    pub workspaces: Vec<ReorderItem>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct MovedCountData {
    moved_count: u64,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct MoveItemToGroupRequest {
    pub group_id: Uuid,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyAcademicMenuTemplateRequest {
    pub revision: String,
}

async fn auth_with_permission(
    state: &AppState,
    session: &AuthenticatedSession,
    permission: &str,
) -> Result<ActorTenantContext, AppError> {
    let context = actor_tenant_context_from_session(state, session).await?;
    context.actor.require_permission(permission)?;
    Ok(context)
}

// ==================== Recommended Academic Template ====================

#[utoipa::path(
    get,
    path = "/api/admin/menu/templates/academic/recommended",
    operation_id = "previewRecommendedAcademicMenuTemplate",
    tag = "menu",
    responses(
        (status = 200, description = "Recommended academic menu template preview", body = ApiResponse<AcademicMenuTemplatePreview>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu read permission required", body = ApiErrorResponse)
    )
)]
pub async fn preview_recommended_academic_menu_template(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_with_permission(&state, &session, codes::MENU_READ_ALL).await?;
    let preview =
        academic_template_service::preview_academic_template(&context.tenant.pool).await?;
    Ok((StatusCode::OK, JsonResponse(ApiResponse::ok(preview))))
}

#[utoipa::path(
    post,
    path = "/api/admin/menu/templates/academic/recommended/apply",
    operation_id = "applyRecommendedAcademicMenuTemplate",
    tag = "menu",
    request_body = ApplyAcademicMenuTemplateRequest,
    responses(
        (status = 200, description = "Recommended academic menu template applied", body = ApiResponse<AcademicMenuTemplateApplyResult>),
        (status = 400, description = "Route recommendations are not ready", body = ApiErrorResponse),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu update permission required", body = ApiErrorResponse),
        (status = 409, description = "Preview revision is stale", body = ApiErrorResponse)
    )
)]
pub async fn apply_recommended_academic_menu_template(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    JsonResponse(data): JsonResponse<ApplyAcademicMenuTemplateRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_with_permission(&state, &session, codes::MENU_UPDATE_ALL).await?;
    let result = academic_template_service::apply_academic_template(
        &context.tenant.pool,
        data.revision.trim(),
    )
    .await?;
    Ok((
        StatusCode::OK,
        JsonResponse(ApiResponse::with_message(
            result,
            "Applied recommended academic menu template",
        )),
    ))
}

// ==================== Menu Workspaces ====================

#[utoipa::path(
    get,
    path = "/api/admin/menu/workspaces",
    operation_id = "listMenuWorkspaces",
    tag = "menu",
    responses(
        (status = 200, description = "Menu workspaces", body = ApiResponse<Vec<MenuWorkspace>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu read permission required", body = ApiErrorResponse)
    )
)]
pub async fn list_menu_workspaces(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_with_permission(&state, &session, codes::MENU_READ_ALL).await?;
    let workspaces = menu_service::list_menu_workspaces(&context.tenant.pool).await?;
    Ok((StatusCode::OK, JsonResponse(ApiResponse::ok(workspaces))))
}

#[utoipa::path(
    post,
    path = "/api/admin/menu/workspaces",
    operation_id = "createMenuWorkspace",
    tag = "menu",
    request_body = CreateMenuWorkspaceRequest,
    responses(
        (status = 201, description = "Menu workspace created", body = ApiResponse<MenuWorkspace>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu create permission required", body = ApiErrorResponse)
    )
)]
pub async fn create_menu_workspace(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    JsonResponse(data): JsonResponse<CreateMenuWorkspaceRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_with_permission(&state, &session, codes::MENU_CREATE_ALL).await?;
    let workspace = menu_service::create_menu_workspace(
        &context.tenant.pool,
        menu_service::CreateMenuWorkspaceInput {
            code: data.code,
            name: data.name,
            name_en: data.name_en,
            description: data.description,
            icon: data.icon,
            display_order: data.display_order,
        },
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        JsonResponse(ApiResponse::with_message(
            workspace,
            "Menu workspace created successfully",
        )),
    ))
}

#[utoipa::path(
    put,
    path = "/api/admin/menu/workspaces/{id}",
    operation_id = "updateMenuWorkspace",
    tag = "menu",
    params(("id" = Uuid, Path, description = "Menu workspace ID")),
    request_body = UpdateMenuWorkspaceRequest,
    responses(
        (status = 200, description = "Menu workspace updated", body = ApiResponse<MenuWorkspace>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu update permission required", body = ApiErrorResponse),
        (status = 404, description = "Menu workspace not found", body = ApiErrorResponse)
    )
)]
pub async fn update_menu_workspace(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(session): Extension<AuthenticatedSession>,
    JsonResponse(data): JsonResponse<UpdateMenuWorkspaceRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_with_permission(&state, &session, codes::MENU_UPDATE_ALL).await?;
    let workspace = menu_service::update_menu_workspace(
        &context.tenant.pool,
        id,
        menu_service::UpdateMenuWorkspaceInput {
            name: data.name,
            name_en: data.name_en,
            description: data.description,
            icon: data.icon,
            display_order: data.display_order,
            is_active: data.is_active,
        },
    )
    .await?;
    Ok((
        StatusCode::OK,
        JsonResponse(ApiResponse::with_message(
            workspace,
            "Menu workspace updated successfully",
        )),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/admin/menu/workspaces/{id}",
    operation_id = "deleteMenuWorkspace",
    tag = "menu",
    params(("id" = Uuid, Path, description = "Menu workspace ID")),
    responses(
        (status = 200, description = "Menu workspace deleted", body = ApiResponse<MovedCountData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu delete permission required", body = ApiErrorResponse),
        (status = 404, description = "Menu workspace not found", body = ApiErrorResponse)
    )
)]
pub async fn delete_menu_workspace(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_with_permission(&state, &session, codes::MENU_DELETE_ALL).await?;
    let moved = menu_service::delete_menu_workspace(&context.tenant.pool, id).await?;
    Ok((
        StatusCode::OK,
        JsonResponse(ApiResponse::with_message(
            MovedCountData { moved_count: moved },
            format!(
                "Deleted workspace and moved {} groups to general administration",
                moved
            ),
        )),
    ))
}

#[utoipa::path(
    post,
    path = "/api/admin/menu/workspaces/reorder",
    operation_id = "reorderMenuWorkspaces",
    tag = "menu",
    request_body = ReorderWorkspacesRequest,
    responses(
        (status = 200, description = "Menu workspaces reordered", body = ApiResponse<EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu update permission required", body = ApiErrorResponse)
    )
)]
pub async fn reorder_menu_workspaces(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    JsonResponse(data): JsonResponse<ReorderWorkspacesRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_with_permission(&state, &session, codes::MENU_UPDATE_ALL).await?;
    let workspaces = data
        .workspaces
        .into_iter()
        .map(|workspace| (workspace.id, workspace.display_order))
        .collect();
    let count = menu_service::reorder_menu_workspaces(&context.tenant.pool, workspaces).await?;
    Ok((
        StatusCode::OK,
        JsonResponse(ApiResponse::empty_with_message(format!(
            "Reordered {} workspaces",
            count
        ))),
    ))
}

// ==================== Menu Groups ====================

#[utoipa::path(
    get,
    path = "/api/admin/menu/groups",
    operation_id = "listMenuGroups",
    tag = "menu",
    responses(
        (status = 200, description = "Menu groups", body = ApiResponse<Vec<MenuGroup>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu read permission required", body = ApiErrorResponse)
    )
)]
pub async fn list_menu_groups(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_with_permission(&state, &session, codes::MENU_READ_ALL).await?;
    let groups = menu_service::list_menu_groups(&context.tenant.pool).await?;
    Ok((StatusCode::OK, JsonResponse(ApiResponse::ok(groups))))
}

#[utoipa::path(
    post,
    path = "/api/admin/menu/groups",
    operation_id = "createMenuGroup",
    tag = "menu",
    request_body = CreateMenuGroupRequest,
    responses(
        (status = 201, description = "Menu group created", body = ApiResponse<MenuGroup>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu create permission required", body = ApiErrorResponse)
    )
)]
pub async fn create_menu_group(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    JsonResponse(data): JsonResponse<CreateMenuGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_with_permission(&state, &session, codes::MENU_CREATE_ALL).await?;
    let group = menu_service::create_menu_group(
        &context.tenant.pool,
        menu_service::CreateMenuGroupInput {
            code: data.code,
            name: data.name,
            name_en: data.name_en,
            description: data.description,
            icon: data.icon,
            workspace_code: data.workspace_code,
            display_order: data.display_order,
        },
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        JsonResponse(ApiResponse::with_message(
            group,
            "Menu group created successfully",
        )),
    ))
}

#[utoipa::path(
    put,
    path = "/api/admin/menu/groups/{id}",
    operation_id = "updateMenuGroup",
    tag = "menu",
    params(("id" = Uuid, Path, description = "Menu group ID")),
    request_body = UpdateMenuGroupRequest,
    responses(
        (status = 200, description = "Menu group updated", body = ApiResponse<MenuGroup>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu update permission required", body = ApiErrorResponse),
        (status = 404, description = "Menu group not found", body = ApiErrorResponse)
    )
)]
pub async fn update_menu_group(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(session): Extension<AuthenticatedSession>,
    JsonResponse(data): JsonResponse<UpdateMenuGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_with_permission(&state, &session, codes::MENU_UPDATE_ALL).await?;
    let group = menu_service::update_menu_group(
        &context.tenant.pool,
        id,
        menu_service::UpdateMenuGroupInput {
            name: data.name,
            name_en: data.name_en,
            description: data.description,
            icon: data.icon,
            workspace_code: data.workspace_code,
            display_order: data.display_order,
            is_active: data.is_active,
        },
    )
    .await?;
    Ok((
        StatusCode::OK,
        JsonResponse(ApiResponse::with_message(
            group,
            "Menu group updated successfully",
        )),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/admin/menu/groups/{id}",
    operation_id = "deleteMenuGroup",
    tag = "menu",
    params(("id" = Uuid, Path, description = "Menu group ID")),
    responses(
        (status = 200, description = "Menu group deleted", body = ApiResponse<MovedCountData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu delete permission required", body = ApiErrorResponse),
        (status = 404, description = "Menu group not found", body = ApiErrorResponse)
    )
)]
pub async fn delete_menu_group(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_with_permission(&state, &session, codes::MENU_DELETE_ALL).await?;
    let moved = menu_service::delete_menu_group(&context.tenant.pool, id).await?;
    Ok((
        StatusCode::OK,
        JsonResponse(ApiResponse::with_message(
            MovedCountData { moved_count: moved },
            format!("Deleted group and moved {} items to 'other'", moved),
        )),
    ))
}

// ==================== Menu Items ====================

#[utoipa::path(
    get,
    path = "/api/admin/menu/items",
    operation_id = "listMenuItems",
    tag = "menu",
    params(MenuItemFilter),
    responses(
        (status = 200, description = "Menu items visible to the current administrator", body = ApiResponse<Vec<MenuItem>>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu read permission required", body = ApiErrorResponse)
    )
)]
pub async fn list_menu_items(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Query(filter): Query<MenuItemFilter>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_with_permission(&state, &session, codes::MENU_READ_ALL).await?;
    let items = menu_service::list_menu_items(&context.tenant.pool, filter.group_id).await?;
    Ok((StatusCode::OK, JsonResponse(ApiResponse::ok(items))))
}

#[utoipa::path(
    post,
    path = "/api/admin/menu/items",
    operation_id = "createMenuItem",
    tag = "menu",
    request_body = CreateMenuItemRequest,
    responses(
        (status = 201, description = "Menu item created", body = ApiResponse<MenuItem>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu create permission required", body = ApiErrorResponse)
    )
)]
pub async fn create_menu_item(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    JsonResponse(data): JsonResponse<CreateMenuItemRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_with_permission(&state, &session, codes::MENU_CREATE_ALL).await?;
    let item = menu_service::create_menu_item(
        &context.tenant.pool,
        menu_service::CreateMenuItemInput {
            code: data.code,
            name: data.name,
            name_en: data.name_en,
            description: data.description,
            path: data.path,
            icon: data.icon,
            group_id: data.group_id,
            parent_id: data.parent_id,
            required_permission: data.required_permission,
            display_order: data.display_order,
        },
    )
    .await?;
    Ok((
        StatusCode::CREATED,
        JsonResponse(ApiResponse::with_message(
            item,
            "Menu item created successfully",
        )),
    ))
}

#[utoipa::path(
    put,
    path = "/api/admin/menu/items/{id}",
    operation_id = "updateMenuItem",
    tag = "menu",
    params(("id" = Uuid, Path, description = "Menu item ID")),
    request_body = UpdateMenuItemRequest,
    responses(
        (status = 200, description = "Menu item updated", body = ApiResponse<MenuItem>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu update permission required", body = ApiErrorResponse),
        (status = 404, description = "Menu item not found", body = ApiErrorResponse)
    )
)]
pub async fn update_menu_item(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(session): Extension<AuthenticatedSession>,
    JsonResponse(data): JsonResponse<UpdateMenuItemRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_with_permission(&state, &session, codes::MENU_UPDATE_ALL).await?;
    let item = menu_service::update_menu_item(
        &context.tenant.pool,
        id,
        menu_service::UpdateMenuItemInput {
            name: data.name,
            name_en: data.name_en,
            description: data.description,
            path: data.path,
            icon: data.icon,
            group_id: data.group_id,
            parent_id: data.parent_id,
            required_permission: data.required_permission,
            display_order: data.display_order,
            is_active: data.is_active,
        },
    )
    .await?;
    Ok((
        StatusCode::OK,
        JsonResponse(ApiResponse::with_message(
            item,
            "Menu item updated successfully",
        )),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/admin/menu/items/{id}",
    operation_id = "deleteMenuItem",
    tag = "menu",
    params(("id" = Uuid, Path, description = "Menu item ID")),
    responses(
        (status = 200, description = "Menu item deleted", body = ApiResponse<EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu delete permission required", body = ApiErrorResponse),
        (status = 404, description = "Menu item not found", body = ApiErrorResponse)
    )
)]
pub async fn delete_menu_item(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_with_permission(&state, &session, codes::MENU_DELETE_ALL).await?;
    menu_service::delete_menu_item(&context.tenant.pool, id).await?;
    Ok((
        StatusCode::OK,
        JsonResponse(ApiResponse::empty_with_message(
            "Menu item deleted successfully",
        )),
    ))
}

#[utoipa::path(
    post,
    path = "/api/admin/menu/items/reorder",
    operation_id = "reorderMenuItems",
    tag = "menu",
    request_body = ReorderRequest,
    responses(
        (status = 200, description = "Menu items reordered", body = ApiResponse<EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu update permission required", body = ApiErrorResponse)
    )
)]
pub async fn reorder_menu_items(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    JsonResponse(data): JsonResponse<ReorderRequest>,
) -> Result<impl IntoResponse, AppError> {
    if data.items.is_empty() {
        return Ok((
            StatusCode::OK,
            JsonResponse(ApiResponse::empty_with_message("No items to reorder")),
        ));
    }
    let context = auth_with_permission(&state, &session, codes::MENU_UPDATE_ALL).await?;
    let items: Vec<(Uuid, i32, Option<Uuid>)> = data
        .items
        .into_iter()
        .map(|i| (i.id, i.display_order, i.group_id))
        .collect();
    let count = menu_service::reorder_menu_items(&context.tenant.pool, items).await?;
    Ok((
        StatusCode::OK,
        JsonResponse(ApiResponse::empty_with_message(format!(
            "Reordered {} items successfully",
            count
        ))),
    ))
}

#[utoipa::path(
    post,
    path = "/api/admin/menu/groups/reorder",
    operation_id = "reorderMenuGroups",
    tag = "menu",
    request_body = ReorderGroupsRequest,
    responses(
        (status = 200, description = "Menu groups reordered", body = ApiResponse<EmptyData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu update permission required", body = ApiErrorResponse)
    )
)]
pub async fn reorder_menu_groups(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    JsonResponse(data): JsonResponse<ReorderGroupsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_with_permission(&state, &session, codes::MENU_UPDATE_ALL).await?;
    let groups: Vec<(Uuid, i32)> = data
        .groups
        .into_iter()
        .map(|g| (g.id, g.display_order))
        .collect();
    let count = menu_service::reorder_menu_groups(&context.tenant.pool, groups).await?;
    Ok((
        StatusCode::OK,
        JsonResponse(ApiResponse::empty_with_message(format!(
            "Reordered {} groups",
            count
        ))),
    ))
}

#[utoipa::path(
    put,
    path = "/api/admin/menu/items/{id}/group",
    operation_id = "moveMenuItemToGroup",
    tag = "menu",
    params(("id" = Uuid, Path, description = "Menu item ID")),
    request_body = MoveItemToGroupRequest,
    responses(
        (status = 200, description = "Menu item moved", body = ApiResponse<MenuItem>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse),
        (status = 403, description = "Menu update permission required", body = ApiErrorResponse),
        (status = 404, description = "Menu item not found", body = ApiErrorResponse)
    )
)]
pub async fn move_item_to_group(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Extension(session): Extension<AuthenticatedSession>,
    JsonResponse(data): JsonResponse<MoveItemToGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_with_permission(&state, &session, codes::MENU_UPDATE_ALL).await?;
    let item = menu_service::move_item_to_group(&context.tenant.pool, id, data.group_id).await?;
    Ok((StatusCode::OK, JsonResponse(ApiResponse::ok(item))))
}
