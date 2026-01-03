use crate::models::menu::{MenuGroup, MenuItem};
use crate::models::auth::User;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::utils::jwt::JwtService;
use crate::AppState;

use axum::{
    extract::{State, Path, Query},
    http::{StatusCode, HeaderMap},
    response::{Response, IntoResponse, Json as JsonResponse},
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

// ==================== Request/Response Types ====================

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
    pub required_permission: Option<String>, // Module name (e.g., "attendance", "staff")
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
}

#[derive(Debug, Deserialize)]
pub struct MenuItemFilter {
    pub group_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
pub struct MenuGroupListResponse {
    pub success: bool,
    pub data: Vec<MenuGroup>,
}

#[derive(Debug, Serialize)]
pub struct MenuItemListResponse {
    pub success: bool,
    pub data: Vec<MenuItem>,
}

#[derive(Debug, Serialize)]
pub struct MenuGroupResponse {
    pub success: bool,
    pub data: Option<MenuGroup>,
    pub message: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MenuItemResponse {
    pub success: bool,
    pub data: Option<MenuItem>,
    pub message: Option<String>,
}

// ==================== Menu Groups ====================

/// List all menu groups (no filtering needed - groups don't belong to modules)
pub async fn list_menu_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let (pool, _) = match get_pool_and_authenticate(&state, &headers).await {
        Ok(result) => result,
        Err(response) => return response,
    };

    let groups = match sqlx::query_as::<_, MenuGroup>(
        "SELECT id, code, name, name_en, icon, display_order, is_active
         FROM menu_groups
         ORDER BY display_order, name"
    )
    .fetch_all(&pool)
    .await
    {
        Ok(g) => g,
        Err(e) => return internal_error_response(&format!("Failed to fetch menu groups: {}", e)),
    };

    (
        StatusCode::OK,
        JsonResponse(MenuGroupListResponse {
            success: true,
            data: groups,
        })
    )
        .into_response()
}

/// Create menu group (any authenticated user can create groups)
pub async fn create_menu_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<CreateMenuGroupRequest>,
) -> Response {
    let (pool, _) = match get_pool_and_authenticate(&state, &headers).await {
        Ok(result) => result,
        Err(response) => return response,
    };

    let group = match sqlx::query_as::<_, MenuGroup>(
        "INSERT INTO menu_groups (code, name, name_en, description, icon, display_order)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, code, name, name_en, icon, display_order, is_active"
    )
    .bind(&data.code)
    .bind(&data.name)
    .bind(&data.name_en)
    .bind(&data.description)
    .bind(&data.icon)
    .bind(data.display_order.unwrap_or(0))
    .fetch_one(&pool)
    .await
    {
        Ok(g) => g,
        Err(e) => return internal_error_response(&format!("Failed to create menu group: {}", e)),
    };

    (
        StatusCode::CREATED,
        JsonResponse(MenuGroupResponse {
            success: true,
            data: Some(group),
            message: Some("Menu group created successfully".to_string()),
        })
    )
        .into_response()
}

/// Update menu group
pub async fn update_menu_group(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<UpdateMenuGroupRequest>,
) -> Response {
    let (pool, _) = match get_pool_and_authenticate(&state, &headers).await {
        Ok(result) => result,
        Err(response) => return response,
    };

    // Build dynamic update
    let mut updates = vec!["updated_at = NOW()".to_string()];
    let mut bind_count = 1;
    
    if data.name.is_some() {
        bind_count += 1;
        updates.push(format!("name = ${}", bind_count));
    }
    if data.name_en.is_some() {
        bind_count += 1;
        updates.push(format!("name_en = ${}", bind_count));
    }
    if data.icon.is_some() {
        bind_count += 1;
        updates.push(format!("icon = ${}", bind_count));
    }
    if data.display_order.is_some() {
        bind_count += 1;
        updates.push(format!("display_order = ${}", bind_count));
    }
    if data.is_active.is_some() {
        bind_count += 1;
        updates.push(format!("is_active = ${}", bind_count));
    }

    let query = format!(
        "UPDATE menu_groups SET {} WHERE id = $1 
         RETURNING id, code, name, name_en, icon, display_order, is_active",
        updates.join(", ")
    );

    let mut query_builder = sqlx::query_as::<_, MenuGroup>(&query).bind(id);
    
    if let Some(name) = &data.name {
        query_builder = query_builder.bind(name);
    }
    if let Some(name_en) = &data.name_en {
        query_builder = query_builder.bind(name_en);
    }
    if let Some(icon) = &data.icon {
        query_builder = query_builder.bind(icon);
    }
    if let Some(order) = data.display_order {
        query_builder = query_builder.bind(order);
    }
    if let Some(active) = data.is_active {
        query_builder = query_builder.bind(active);
    }

    let group = match query_builder.fetch_optional(&pool).await {
        Ok(Some(g)) => g,
        Ok(None) => return not_found_response("Menu group not found"),
        Err(e) => return internal_error_response(&format!("Failed to update menu group: {}", e)),
    };

    (
        StatusCode::OK,
        JsonResponse(MenuGroupResponse {
            success: true,
            data: Some(group),
            message: Some("Menu group updated successfully".to_string()),
        })
    )
        .into_response()
}

/// Delete menu group (moves items to "other" group)
pub async fn delete_menu_group(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    let (pool, _) = match get_pool_and_check_module(&state, &headers, "settings").await {
        Ok(result) => result,
        Err(response) => return response,
    };

    // Get the group being deleted
    let group = match sqlx::query_as::<_, MenuGroup>(
        "SELECT id, code, name, name_en, description, icon, display_order, is_active FROM menu_groups WHERE id = $1"
    )
    .bind(id)
    .fetch_one(&pool)
    .await
    {
        Ok(g) => g,
        Err(_) => return not_found_response("Group not found"),
    };

    // Prevent deletion of "other" group
    if group.code == "other" {
        return (
            StatusCode::BAD_REQUEST,
            JsonResponse(serde_json::json!({
                "success": false,
                "error": "Cannot delete 'other' group - it serves as fallback for orphaned items"
            }))
        ).into_response();
    }

    // Get "other" group ID
    let other_group = match sqlx::query_as::<_, MenuGroup>(
        "SELECT id, code, name, name_en, description, icon, display_order, is_active FROM menu_groups WHERE code = 'other'"
    )
    .fetch_one(&pool)
    .await
    {
        Ok(g) => g,
        Err(_) => {
            return internal_error_response("'other' group not found in database");
        }
    };

    // Begin transaction
    let mut tx = match pool.begin().await {
        Ok(t) => t,
        Err(e) => return internal_error_response(&format!("Transaction failed: {}", e)),
    };

    // Move all menu items to "other" group
    let move_result = sqlx::query(
        "UPDATE menu_items SET group_id = $1 WHERE group_id = $2"
    )
    .bind(other_group.id)
    .bind(id)
    .execute(&mut *tx)
    .await;

    let moved_count = match move_result {
        Ok(result) => result.rows_affected(),
        Err(e) => {
            let _ = tx.rollback().await;
            return internal_error_response(&format!("Failed to move items: {}", e));
        }
    };

    // Delete the group
    let delete_result = sqlx::query(
        "DELETE FROM menu_groups WHERE id = $1"
    )
    .bind(id)
    .execute(&mut *tx)
    .await;

    match delete_result {
        Ok(_) => {
            if let Err(e) = tx.commit().await {
                return internal_error_response(&format!("Failed to commit: {}", e));
            }

            (
                StatusCode::OK,
                JsonResponse(serde_json::json!({
                    "success": true,
                    "message": format!("Deleted group and moved {} items to 'other'", moved_count),
                    "moved_count": moved_count
                }))
            ).into_response()
        }
        Err(e) => {
            let _ = tx.rollback().await;
            internal_error_response(&format!("Failed to delete group: {}", e))
        }
    }
}

// ==================== Menu Items ====================

/// List menu items (filtered by user's module permissions)
pub async fn list_menu_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<MenuItemFilter>,
) -> Response {
    let (pool, _, permissions) = match get_pool_and_authenticate_with_perms(&state, &headers).await {
        Ok(result) => result,
        Err(response) => return response,
    };

    // Fetch all or filtered items
    let all_items = if let Some(group_id) = filter.group_id {
        sqlx::query_as::<_, MenuItem>(
            "SELECT id, code, name, name_en, path, icon, required_permission, 
                    group_id, parent_id, display_order, is_active
             FROM menu_items
             WHERE group_id = $1
             ORDER BY display_order, name"
        )
        .bind(group_id)
        .fetch_all(&pool)
        .await
    } else {
        sqlx::query_as::<_, MenuItem>(
            "SELECT id, code, name, name_en, path, icon, required_permission, 
                    group_id, parent_id, display_order, is_active
             FROM menu_items
             ORDER BY group_id, display_order, name"
        )
        .fetch_all(&pool)
        .await
    };

    let all_items = match all_items {
        Ok(i) => i,
        Err(e) => return internal_error_response(&format!("Failed to fetch menu items: {}", e)),
    };

    // Filter by module permission
    let items: Vec<MenuItem> = all_items
        .into_iter()
        .filter(|item| {
            if let Some(ref module) = item.required_permission {
                has_module_permission(&permissions, module)
            } else {
                true // No permission required
            }
        })
        .collect();

    (
        StatusCode::OK,
        JsonResponse(MenuItemListResponse {
            success: true,
            data: items,
        })
    )
        .into_response()
}

/// Create menu item (requires module permission)
pub async fn create_menu_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<CreateMenuItemRequest>,
) -> Response {
    // Check module permission if required_permission is set
    if let Some(ref module) = data.required_permission {
        let (pool, _) = match get_pool_and_check_module(&state, &headers, module).await {
            Ok(result) => result,
            Err(response) => return response,
        };

        let item = match sqlx::query_as::<_, MenuItem>(
            "INSERT INTO menu_items 
             (code, name, name_en, description, path, icon, group_id, parent_id, 
              required_permission, display_order)
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
             RETURNING id, code, name, name_en, path, icon, required_permission, 
                       group_id, parent_id, display_order, is_active"
        )
        .bind(&data.code)
        .bind(&data.name)
        .bind(&data.name_en)
        .bind(&data.description)
        .bind(&data.path)
        .bind(&data.icon)
        .bind(data.group_id)
        .bind(data.parent_id)
        .bind(&data.required_permission)
        .bind(data.display_order.unwrap_or(0))
        .fetch_one(&pool)
        .await
        {
            Ok(i) => i,
            Err(e) => return internal_error_response(&format!("Failed to create menu item: {}", e)),
        };

        return (
            StatusCode::CREATED,
            JsonResponse(MenuItemResponse {
                success: true,
                data: Some(item),
                message: Some("Menu item created successfully".to_string()),
            })
        )
            .into_response();
    }

    // No permission required - just authenticate
    let (pool, _) = match get_pool_and_authenticate(&state, &headers).await {
        Ok(result) => result,
        Err(response) => return response,
    };

    let item = match sqlx::query_as::<_, MenuItem>(
        "INSERT INTO menu_items 
         (code, name, name_en, description, path, icon, group_id, parent_id, 
          required_permission, display_order)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING id, code, name, name_en, path, icon, required_permission, 
                   group_id, parent_id, display_order, is_active"
    )
    .bind(&data.code)
    .bind(&data.name)
    .bind(&data.name_en)
    .bind(&data.description)
    .bind(&data.path)
    .bind(&data.icon)
    .bind(data.group_id)
    .bind(data.parent_id)
    .bind(&data.required_permission)
    .bind(data.display_order.unwrap_or(0))
    .fetch_one(&pool)
    .await
    {
        Ok(i) => i,
        Err(e) => return internal_error_response(&format!("Failed to create menu item: {}", e)),
    };

    (
        StatusCode::CREATED,
        JsonResponse(MenuItemResponse {
            success: true,
            data: Some(item),
            message: Some("Menu item created successfully".to_string()),
        })
    )
        .into_response()
}

/// Update menu item (requires module permission for existing item)
pub async fn update_menu_item(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<UpdateMenuItemRequest>,
) -> Response {
    let (pool, _, permissions) = match get_pool_and_authenticate_with_perms(&state, &headers).await {
        Ok(result) => result,
        Err(response) => return response,
    };

    // First, fetch existing item to check its module
    let existing_item = match sqlx::query_as::<_, MenuItem>(
        "SELECT id, code, name, name_en, path, icon, required_permission, 
                group_id, parent_id, display_order, is_active
         FROM menu_items WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(item)) => item,
        Ok(None) => return not_found_response("Menu item not found"),
        Err(e) => return internal_error_response(&format!("Database error: {}", e)),
    };

    // Check module permission for existing item
    if let Some(ref module) = existing_item.required_permission {
        if !has_module_permission(&permissions, module) {
            return forbidden_response(&format!("No permission for module '{}'", module));
        }
    }

    // Build dynamic update
    let mut updates = vec!["updated_at = NOW()".to_string()];
    let mut bind_count = 1;

    if data.name.is_some() {
        bind_count += 1;
        updates.push(format!("name = ${}", bind_count));
    }
    if data.path.is_some() {
        bind_count += 1;
        updates.push(format!("path = ${}", bind_count));
    }
    if data.required_permission.is_some() {
        bind_count += 1;
        updates.push(format!("required_permission = ${}", bind_count));
    }
    if data.display_order.is_some() {
        bind_count += 1;
        updates.push(format!("display_order = ${}", bind_count));
    }
    if data.is_active.is_some() {
        bind_count += 1;
        updates.push(format!("is_active = ${}", bind_count));
    }

    let query = format!(
        "UPDATE menu_items SET {} WHERE id = $1
         RETURNING id, code, name, name_en, path, icon, required_permission,
                   group_id, parent_id, display_order, is_active",
        updates.join(", ")
    );

    let mut query_builder = sqlx::query_as::<_, MenuItem>(&query).bind(id);
    
    if let Some(name) = &data.name {
        query_builder = query_builder.bind(name);
    }
    if let Some(path) = &data.path {
        query_builder = query_builder.bind(path);
    }
    if let Some(perm) = &data.required_permission {
        query_builder = query_builder.bind(perm);
    }
    if let Some(order) = data.display_order {
        query_builder = query_builder.bind(order);
    }
    if let Some(active) = data.is_active {
        query_builder = query_builder.bind(active);
    }

    let item = match query_builder.fetch_optional(&pool).await {
        Ok(Some(i)) => i,
        Ok(None) => return not_found_response("Menu item not found"),
        Err(e) => return internal_error_response(&format!("Failed to update menu item: {}", e)),
    };

    (
        StatusCode::OK,
        JsonResponse(MenuItemResponse {
            success: true,
            data: Some(item),
            message: Some("Menu item updated successfully".to_string()),
        })
    )
        .into_response()
}

/// Delete menu item (requires module permission)
pub async fn delete_menu_item(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    let (pool, _, permissions) = match get_pool_and_authenticate_with_perms(&state, &headers).await {
        Ok(result) => result,
        Err(response) => return response,
    };

    // First, fetch the item to check its module
    let existing_item = match sqlx::query_as::<_, MenuItem>(
        "SELECT id, code, name, name_en, path, icon, required_permission, 
                group_id, parent_id, display_order, is_active
         FROM menu_items WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    {
        Ok(Some(item)) => item,
        Ok(None) => return not_found_response("Menu item not found"),
        Err(e) => return internal_error_response(&format!("Database error: {}", e)),
    };

    // Check module permission
    if let Some(ref module) = existing_item.required_permission {
        if !has_module_permission(&permissions, module) {
            return forbidden_response(&format!("No permission for module '{}'", module));
        }
    }

    match sqlx::query("DELETE FROM menu_items WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
    {
        Ok(result) if result.rows_affected() == 0 => {
            return not_found_response("Menu item not found");
        }
        Ok(_) => {}
        Err(e) => return internal_error_response(&format!("Failed to delete menu item: {}", e)),
    }

    (
        StatusCode::OK,
        JsonResponse(serde_json::json!({
            "success": true,
            "message": "Menu item deleted successfully"
        }))
    )
        .into_response()
}

/// Reorder menu items (requires module permission for each item)
pub async fn reorder_menu_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<ReorderRequest>,
) -> Response {
    let (pool, _, permissions) = match get_pool_and_authenticate_with_perms(&state, &headers).await {
        Ok(result) => result,
        Err(response) => return response,
    };

    // Update display_order for each item (with permission check)
    for item in &data.items {
        // Fetch item to check permission
        let existing_item = match sqlx::query_as::<_, MenuItem>(
            "SELECT id, code, name, name_en, path, icon, required_permission, 
                    group_id, parent_id, display_order, is_active
             FROM menu_items WHERE id = $1"
        )
        .bind(item.id)
        .fetch_optional(&pool)
        .await
        {
            Ok(Some(i)) => i,
            Ok(None) => continue, // Skip if not found
            Err(e) => return internal_error_response(&format!("Failed to fetch item: {}", e)),
        };

        // Check permission
        if let Some(ref module) = existing_item.required_permission {
            if !has_module_permission(&permissions, module) {
                return forbidden_response(&format!("No permission for module '{}'", module));
            }
        }

        // Update order
        match sqlx::query(
            "UPDATE menu_items SET display_order = $1, updated_at = NOW() WHERE id = $2"
        )
        .bind(item.display_order)
        .bind(item.id)
        .execute(&pool)
        .await
        {
            Ok(_) => {}
            Err(e) => return internal_error_response(&format!("Failed to reorder items: {}", e)),
        }
    }

    (
        StatusCode::OK,
        JsonResponse(serde_json::json!({
            "success": true,
            "message": "Menu items reordered successfully"
        }))
    )
        .into_response()
}

// ==================== Helper Functions ====================

/// Helper: Check if user has ANY permission in the specified module
fn has_module_permission(user_permissions: &[String], module: &str) -> bool {
    if module.is_empty() {
        return true; // No permission required
    }
    
    // Check for wildcard permission first
    if user_permissions.contains(&"*".to_string()) {
        return true;
    }
    
    // Check for module-specific permissions
    let prefix = format!("{}.", module);
    user_permissions.iter().any(|perm| perm.starts_with(&prefix))
}

/// Helper: Get pool and check module permission
async fn get_pool_and_check_module(
    state: &AppState,
    headers: &HeaderMap,
    module: &str,
) -> Result<(PgPool, User), Response> {
    let (pool, user, permissions) = get_pool_and_authenticate_with_perms(state, headers).await?;

    // Check module permission
    if !has_module_permission(&permissions, module) {
        return Err(forbidden_response(&format!("No permission for module '{}'", module)));
    }

    Ok((pool, user))
}

/// Helper: Get pool and authenticate (returns pool + user)
async fn get_pool_and_authenticate(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(PgPool, User), Response> {
    let (pool, user, _) = get_pool_and_authenticate_with_perms(state, headers).await?;
    Ok((pool, user))
}

/// Helper: Get pool and authenticate (returns pool + user + permissions)
async fn get_pool_and_authenticate_with_perms(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(PgPool, User, Vec<String>), Response> {
    // Extract subdomain
    let subdomain = match extract_subdomain_from_request(headers) {
        Ok(s) => s,
        Err(response) => return Err(response),
    };

    // Get database URL
    let db_url = match crate::db::school_mapping::get_school_database_url(&state.admin_pool, &subdomain).await {
        Ok(url) => url,
        Err(e) => {
            return Err(bad_request_response(&format!("School not found: {}", e)));
        }
    };

    // Get pool
    let pool = match state.pool_manager.get_pool(&db_url, &subdomain).await {
        Ok(p) => p,
        Err(e) => {
            return Err(internal_error_response(&format!("Database error: {}", e)));
        }
    };

    // Get user and permissions
    let (user, permissions) = match authenticate_user(headers, &pool).await {
        Ok(result) => result,
        Err(e) => return Err(e),
    };

    Ok((pool, user, permissions))
}

/// Helper: Authenticate user and get permissions
async fn authenticate_user(
    headers: &HeaderMap,
    pool: &PgPool,
) -> Result<(User, Vec<String>), Response> {
    // Try to extract token from Authorization header first
    let auth_header = headers
        .get("Authorization")
        .and_then(|h| h.to_str().ok());
    
    let token_from_header = auth_header
        .and_then(|h| {
            if h.starts_with("Bearer ") {
                Some(h[7..].to_string())
            } else {
                None
            }
        });

    // Fallback to cookie
    let token_from_cookie = headers
        .get("Cookie")
        .and_then(|h| h.to_str().ok())
        .and_then(|cookie| JwtService::extract_token_from_cookie(Some(cookie)));

    // Use Authorization header first, then cookie
    let token = match token_from_header.or(token_from_cookie) {
        Some(t) => t,
        None => return Err(unauthorized_response("No authentication token found")),
    };


    // Validate token and extract claims
    let claims = match JwtService::verify_token(&token) {
        Ok(c) => c,
        Err(_) => return Err(unauthorized_response("Invalid or expired token")),
    };

    // Get user from database
    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => return Err(unauthorized_response("Invalid user ID in token")),
    };

    let user = match sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = $1")
        .bind(user_id)
        .fetch_optional(pool)
        .await
    {
        Ok(Some(u)) => u,
        Ok(None) => return Err(unauthorized_response("User not found")),
        Err(e) => return Err(internal_error_response(&format!("Database error: {}", e))),
    };

    // Get user permissions (use unnest to handle wildcard and all permissions)
    let permissions: Vec<String> = match sqlx::query_scalar(
        "SELECT DISTINCT unnest(r.permissions) as permission
         FROM user_roles ur
         JOIN roles r ON ur.role_id = r.id
         WHERE ur.user_id = $1 
           AND ur.ended_at IS NULL
           AND r.is_active = true"
    )
    .bind(user.id)
    .fetch_all(pool)
    .await
    {
        Ok(p) => p,
        Err(e) => {
            return Err(internal_error_response(&format!("Failed to fetch permissions: {}", e)));
        }
    };

    Ok((user, permissions))
}

// Response helpers
fn bad_request_response(message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        JsonResponse(serde_json::json!({
            "success": false,
            "error": message
        }))
    )
        .into_response()
}

fn unauthorized_response(message: &str) -> Response {
    (
        StatusCode::UNAUTHORIZED,
        JsonResponse(serde_json::json!({
            "success": false,
            "error": message
        }))
    )
        .into_response()
}

fn forbidden_response(message: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        JsonResponse(serde_json::json!({
            "success": false,
            "error": message
        }))
    )
        .into_response()
}

fn not_found_response(message: &str) -> Response {
    (
        StatusCode::NOT_FOUND,
        JsonResponse(serde_json::json!({
            "success": false,
            "error": message
        }))
    )
        .into_response()
}

fn internal_error_response(message: &str) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        JsonResponse(serde_json::json!({
            "success": false,
            "error": message
        }))
    )
        .into_response()
}


// ==================== Group Reordering ====================

/// Reorder menu groups
#[derive(Debug, Deserialize)]
pub struct ReorderGroupsRequest {
    pub groups: Vec<ReorderItem>,
}

pub async fn reorder_menu_groups(
    State(state): State<AppState>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<ReorderGroupsRequest>,
) -> Response {
    let (pool, _) = match get_pool_and_check_module(&state, &headers, "settings").await {
        Ok(result) => result,
        Err(response) => return response,
    };

    let mut tx = match pool.begin().await {
        Ok(t) => t,
        Err(e) => return internal_error_response(&format!("Transaction failed: {}", e)),
    };

    for item in &data.groups {
        if let Err(e) = sqlx::query("UPDATE menu_groups SET display_order = $1 WHERE id = $2")
            .bind(item.display_order).bind(item.id).execute(&mut *tx).await {
            let _ = tx.rollback().await;
            return internal_error_response(&format!("Failed to reorder: {}", e));
        }
    }

    match tx.commit().await {
        Ok(_) => (StatusCode::OK, JsonResponse(serde_json::json!({
            "success": true, "message": format!("Reordered {} groups", data.groups.len())
        }))).into_response(),
        Err(e) => internal_error_response(&format!("Failed to commit: {}", e)),
    }
}

/// Move menu item to different group
pub async fn move_item_to_group(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<serde_json::Value>,
) -> Response {
    let (pool, _) = match get_pool_and_check_module(&state, &headers, "settings").await {
        Ok(result) => result,
        Err(response) => return response,
    };

    let group_id = match data.get("group_id").and_then(|v| v.as_str()) {
        Some(id_str) => match Uuid::parse_str(id_str) {
            Ok(uuid) => uuid,
            Err(_) => return (StatusCode::BAD_REQUEST, JsonResponse(serde_json::json!({
                "success": false, "error": "Invalid group_id format"
            }))).into_response(),
        },
        None => return (StatusCode::BAD_REQUEST, JsonResponse(serde_json::json!({
            "success": false, "error": "group_id required"
        }))).into_response(),
    };

    let result = sqlx::query_as::<_, MenuItem>(
        r#"UPDATE menu_items 
           SET group_id = $1 
           WHERE id = $2 
           RETURNING id, code, name, name_en, description, path, icon, 
                     group_id, parent_id, required_permission, display_order, is_active"#
    )
    .bind(group_id)
    .bind(id)
    .fetch_one(&pool)
    .await;

    match result {
        Ok(item) => (StatusCode::OK, JsonResponse(serde_json::json!({
            "success": true,
            "data": item
        }))).into_response(),
        Err(e) => internal_error_response(&format!("Failed to move item: {}", e)),
    }
}
