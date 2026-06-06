use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::menu::services::menu_service;
use crate::utils::request_context::{actor_tenant_context, ActorTenantContext};
use crate::AppState;

use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json as JsonResponse},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Deserialize)]
pub struct CreateMenuGroupRequest {
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub display_order: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateMenuGroupRequest {
    pub name: Option<String>,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub display_order: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
pub struct ReorderRequest {
    pub items: Vec<ReorderItem>,
}

#[derive(Debug, Deserialize)]
pub struct ReorderItem {
    pub id: Uuid,
    pub display_order: i32,
    pub group_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct MenuItemFilter {
    pub group_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct ReorderGroupsRequest {
    pub groups: Vec<ReorderItem>,
}

#[derive(Debug, Serialize)]
struct MovedCountData {
    moved_count: u64,
}

#[derive(Debug, Deserialize)]
pub struct MoveItemToGroupRequest {
    pub group_id: Uuid,
}

async fn auth(state: &AppState, headers: &HeaderMap) -> Result<ActorTenantContext, AppError> {
    actor_tenant_context(state, headers).await
}

async fn auth_check_module(
    state: &AppState,
    headers: &HeaderMap,
    module: &str,
) -> Result<ActorTenantContext, AppError> {
    let context = auth(state, headers).await?;
    if !context.actor.has_module_permission(module) {
        return Err(AppError::Forbidden(format!(
            "No permission for module '{}'",
            module
        )));
    }
    Ok(context)
}

// ==================== Menu Groups ====================

pub async fn list_menu_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = auth(&state, &headers).await?;
    let groups = menu_service::list_menu_groups(&context.tenant.pool).await?;
    Ok((StatusCode::OK, JsonResponse(ApiResponse::ok(groups))))
}

pub async fn create_menu_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<CreateMenuGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth(&state, &headers).await?;
    let group = menu_service::create_menu_group(
        &context.tenant.pool,
        menu_service::CreateMenuGroupInput {
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
            group,
            "Menu group created successfully",
        )),
    ))
}

pub async fn update_menu_group(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<UpdateMenuGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth(&state, &headers).await?;
    let group = menu_service::update_menu_group(
        &context.tenant.pool,
        id,
        menu_service::UpdateMenuGroupInput {
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
            group,
            "Menu group updated successfully",
        )),
    ))
}

pub async fn delete_menu_group(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_check_module(&state, &headers, "settings").await?;
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

pub async fn list_menu_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<MenuItemFilter>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth(&state, &headers).await?;
    let items = menu_service::list_menu_items(
        &context.tenant.pool,
        filter.group_id,
        &context.actor.permissions,
    )
    .await?;
    Ok((StatusCode::OK, JsonResponse(ApiResponse::ok(items))))
}

pub async fn create_menu_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<CreateMenuItemRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = if let Some(ref module) = data.required_permission {
        auth_check_module(&state, &headers, module).await?
    } else {
        auth(&state, &headers).await?
    };
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

pub async fn update_menu_item(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<UpdateMenuItemRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth(&state, &headers).await?;
    let existing = menu_service::get_menu_item(&context.tenant.pool, id).await?;
    if let Some(ref module) = existing.required_permission {
        if !context.actor.has_module_permission(module) {
            return Err(AppError::Forbidden(format!(
                "No permission for module '{}'",
                module
            )));
        }
    }
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

pub async fn delete_menu_item(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = auth(&state, &headers).await?;
    let existing = menu_service::get_menu_item(&context.tenant.pool, id).await?;
    if let Some(ref module) = existing.required_permission {
        if !context.actor.has_module_permission(module) {
            return Err(AppError::Forbidden(format!(
                "No permission for module '{}'",
                module
            )));
        }
    }
    menu_service::delete_menu_item(&context.tenant.pool, id).await?;
    Ok((
        StatusCode::OK,
        JsonResponse(ApiResponse::empty_with_message(
            "Menu item deleted successfully",
        )),
    ))
}

pub async fn reorder_menu_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<ReorderRequest>,
) -> Result<impl IntoResponse, AppError> {
    if data.items.is_empty() {
        return Ok((
            StatusCode::OK,
            JsonResponse(ApiResponse::empty_with_message("No items to reorder")),
        ));
    }
    let context = auth(&state, &headers).await?;
    let items: Vec<(Uuid, i32, Option<Uuid>)> = data
        .items
        .into_iter()
        .map(|i| (i.id, i.display_order, i.group_id))
        .collect();
    let count =
        menu_service::reorder_menu_items(&context.tenant.pool, items, &context.actor.permissions)
            .await?;
    Ok((
        StatusCode::OK,
        JsonResponse(ApiResponse::empty_with_message(format!(
            "Reordered {} items successfully",
            count
        ))),
    ))
}

pub async fn reorder_menu_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<ReorderGroupsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_check_module(&state, &headers, "settings").await?;
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

pub async fn move_item_to_group(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<MoveItemToGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = auth_check_module(&state, &headers, "settings").await?;
    let item = menu_service::move_item_to_group(&context.tenant.pool, id, data.group_id).await?;
    Ok((StatusCode::OK, JsonResponse(ApiResponse::ok(item))))
}
