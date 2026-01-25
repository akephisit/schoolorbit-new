use crate::modules::menu::models::{MenuGroup, MenuItem};
use crate::modules::auth::models::User;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::utils::jwt::JwtService;
use crate::utils::field_encryption;
use crate::AppState;
use crate::error::AppError;

use axum::{
    extract::{State, Path, Query},
    http::{StatusCode, HeaderMap},
    response::{IntoResponse, Json as JsonResponse},
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
    pub group_id: Option<Uuid>,
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
) -> Result<impl IntoResponse, AppError> {
    let (pool, _) = get_pool_and_authenticate(&state, &headers).await?;

    let groups = sqlx::query_as::<_, MenuGroup>(
        "SELECT id, code, name, name_en, icon, display_order, is_active
         FROM menu_groups
         ORDER BY display_order, name"
    )
    .fetch_all(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to fetch menu groups: {}", e)))?;

    Ok((
        StatusCode::OK,
        JsonResponse(MenuGroupListResponse {
            success: true,
            data: groups,
        })
    ))
}

/// Create menu group (any authenticated user can create groups)
pub async fn create_menu_group(
    State(state): State<AppState>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<CreateMenuGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (pool, _) = get_pool_and_authenticate(&state, &headers).await?;

    let group = sqlx::query_as::<_, MenuGroup>(
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
    .map_err(|e| AppError::InternalServerError(format!("Failed to create menu group: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        JsonResponse(MenuGroupResponse {
            success: true,
            data: Some(group),
            message: Some("Menu group created successfully".to_string()),
        })
    ))
}

/// Update menu group
pub async fn update_menu_group(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<UpdateMenuGroupRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (pool, _) = get_pool_and_authenticate(&state, &headers).await?;

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
    if data.description.is_some() {
        bind_count += 1;
        updates.push(format!("description = ${}", bind_count));
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
    if let Some(desc) = &data.description {
        query_builder = query_builder.bind(desc);
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

    let group = query_builder.fetch_optional(&pool).await
        .map_err(|e| AppError::InternalServerError(format!("Failed to update menu group: {}", e)))?
        .ok_or(AppError::NotFound("Menu group not found".to_string()))?;

    Ok((
        StatusCode::OK,
        JsonResponse(MenuGroupResponse {
            success: true,
            data: Some(group),
            message: Some("Menu group updated successfully".to_string()),
        })
    ))
}

/// Delete menu group (moves items to "other" group)
pub async fn delete_menu_group(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (pool, _) = get_pool_and_check_module(&state, &headers, "settings").await?;

    // Get the group being deleted
    let group = sqlx::query_as::<_, MenuGroup>(
        "SELECT id, code, name, name_en, description, icon, display_order, is_active FROM menu_groups WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|_| AppError::NotFound("Group not found".to_string()))?
    .ok_or(AppError::NotFound("Group not found".to_string()))?;

    // Prevent deletion of "other" group
    if group.code == "other" {
        return Err(AppError::BadRequest("Cannot delete 'other' group - it serves as fallback for orphaned items".to_string()));
    }

    // Get "other" group ID
    let other_group = sqlx::query_as::<_, MenuGroup>(
        "SELECT id, code, name, name_en, description, icon, display_order, is_active FROM menu_groups WHERE code = 'other'"
    )
    .fetch_one(&pool)
    .await
    .map_err(|_| AppError::InternalServerError("'other' group not found in database".to_string()))?;

    // Begin transaction
    let mut tx = pool.begin().await.map_err(|e| AppError::InternalServerError(format!("Transaction failed: {}", e)))?;

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
            return Err(AppError::InternalServerError(format!("Failed to move items: {}", e)));
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
                return Err(AppError::InternalServerError(format!("Failed to commit: {}", e)));
            }

            Ok((
                StatusCode::OK,
                JsonResponse(serde_json::json!({
                    "success": true,
                    "message": format!("Deleted group and moved {} items to 'other'", moved_count),
                    "moved_count": moved_count
                }))
            ))
        }
        Err(e) => {
            let _ = tx.rollback().await;
            Err(AppError::InternalServerError(format!("Failed to delete group: {}", e)))
        }
    }
}

// ==================== Menu Items ====================

/// List menu items (filtered by user's module permissions)
pub async fn list_menu_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(filter): Query<MenuItemFilter>,
) -> Result<impl IntoResponse, AppError> {
    let (pool, _, permissions) = get_pool_and_authenticate_with_perms(&state, &headers).await?;

    // Fetch all or filtered items
    let all_items = if let Some(group_id) = filter.group_id {
        sqlx::query_as::<_, MenuItem>(
            "SELECT id, code, name, name_en, path, icon, required_permission, user_type, 
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
            "SELECT id, code, name, name_en, path, icon, required_permission, user_type, 
                    group_id, parent_id, display_order, is_active
             FROM menu_items
             ORDER BY group_id, display_order, name"
        )
        .fetch_all(&pool)
        .await
    };

    let all_items = all_items.map_err(|e| AppError::InternalServerError(format!("Failed to fetch menu items: {}", e)))?;

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

    Ok((
        StatusCode::OK,
        JsonResponse(MenuItemListResponse {
            success: true,
            data: items,
        })
    ))
}

/// Create menu item (requires module permission)
pub async fn create_menu_item(
    State(state): State<AppState>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<CreateMenuItemRequest>,
) -> Result<impl IntoResponse, AppError> {
    // Check module permission if required_permission is set
    let pool = if let Some(ref module) = data.required_permission {
        let (pool, _) = get_pool_and_check_module(&state, &headers, module).await?;
        pool
    } else {
        // No permission required - just authenticate
        let (pool, _) = get_pool_and_authenticate(&state, &headers).await?;
        pool
    };

    let item = sqlx::query_as::<_, MenuItem>(
        "INSERT INTO menu_items 
            (code, name, name_en, description, path, icon, group_id, parent_id, 
            required_permission, display_order)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING id, code, name, name_en, path, icon, required_permission, 
                    group_id, parent_id, user_type, display_order, is_active"
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
    .map_err(|e| AppError::InternalServerError(format!("Failed to create menu item: {}", e)))?;

    Ok((
        StatusCode::CREATED,
        JsonResponse(MenuItemResponse {
            success: true,
            data: Some(item),
            message: Some("Menu item created successfully".to_string()),
        })
    ))
}

/// Update menu item (requires module permission for existing item)
pub async fn update_menu_item(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<UpdateMenuItemRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (pool, _, permissions) = get_pool_and_authenticate_with_perms(&state, &headers).await?;

    // First, fetch existing item to check its module
    let existing_item = sqlx::query_as::<_, MenuItem>(
        "SELECT id, code, name, name_en, path, icon, required_permission, user_type, 
                group_id, parent_id, display_order, is_active
         FROM menu_items WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Database error: {}", e)))?
    .ok_or(AppError::NotFound("Menu item not found".to_string()))?;

    // Check module permission for existing item
    if let Some(ref module) = existing_item.required_permission {
        if !has_module_permission(&permissions, module) {
            return Err(AppError::Forbidden(format!("No permission for module '{}'", module)));
        }
    }

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
    if data.description.is_some() {
        bind_count += 1;
        updates.push(format!("description = ${}", bind_count));
    }
    if data.path.is_some() {
        bind_count += 1;
        updates.push(format!("path = ${}", bind_count));
    }
    if data.icon.is_some() {
        bind_count += 1;
        updates.push(format!("icon = ${}", bind_count));
    }
    if data.group_id.is_some() {
        bind_count += 1;
        updates.push(format!("group_id = ${}", bind_count));
    }
    if data.parent_id.is_some() {
        bind_count += 1;
        updates.push(format!("parent_id = ${}", bind_count));
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
                   group_id, parent_id, user_type, display_order, is_active",
        updates.join(", ")
    );

    let mut query_builder = sqlx::query_as::<_, MenuItem>(&query).bind(id);
    
    if let Some(name) = &data.name {
        query_builder = query_builder.bind(name);
    }
    if let Some(name_en) = &data.name_en {
        query_builder = query_builder.bind(name_en);
    }
    if let Some(desc) = &data.description {
        query_builder = query_builder.bind(desc);
    }
    if let Some(path) = &data.path {
        query_builder = query_builder.bind(path);
    }
    if let Some(icon) = &data.icon {
        query_builder = query_builder.bind(icon);
    }
    if let Some(group_id) = data.group_id {
        query_builder = query_builder.bind(group_id);
    }
    if let Some(parent_id) = data.parent_id {
        query_builder = query_builder.bind(parent_id);
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

    let item = query_builder.fetch_optional(&pool).await
        .map_err(|e| AppError::InternalServerError(format!("Failed to update menu item: {}", e)))?
        .ok_or(AppError::NotFound("Menu item not found".to_string()))?;

    Ok((
        StatusCode::OK,
        JsonResponse(MenuItemResponse {
            success: true,
            data: Some(item),
            message: Some("Menu item updated successfully".to_string()),
        })
    ))
}

/// Delete menu item (requires module permission)
pub async fn delete_menu_item(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let (pool, _, permissions) = get_pool_and_authenticate_with_perms(&state, &headers).await?;

    // First, fetch the item to check its module
    let existing_item = sqlx::query_as::<_, MenuItem>(
        "SELECT id, code, name, name_en, path, icon, required_permission, user_type, 
                group_id, parent_id, display_order, is_active
         FROM menu_items WHERE id = $1"
    )
    .bind(id)
    .fetch_optional(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Database error: {}", e)))?
    .ok_or(AppError::NotFound("Menu item not found".to_string()))?;

    // Check module permission
    if let Some(ref module) = existing_item.required_permission {
        if !has_module_permission(&permissions, module) {
            return Err(AppError::Forbidden(format!("No permission for module '{}'", module)));
        }
    }

    let result = sqlx::query("DELETE FROM menu_items WHERE id = $1")
        .bind(id)
        .execute(&pool)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to delete menu item: {}", e)))?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Menu item not found".to_string()));
    }

    Ok((
        StatusCode::OK,
        JsonResponse(serde_json::json!({
            "success": true,
            "message": "Menu item deleted successfully"
        }))
    ))
}

/// Reorder menu items (requires module permission for each item)
pub async fn reorder_menu_items(
    State(state): State<AppState>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<ReorderRequest>,
) -> Result<impl IntoResponse, AppError> {
    let (pool, _, permissions) = get_pool_and_authenticate_with_perms(&state, &headers).await?;

    // Update display_order for each item (with permission check)
    for item in &data.items {
        // Fetch item to check permission
        let existing_item = match sqlx::query_as::<_, MenuItem>(
            "SELECT id, code, name, name_en, path, icon, required_permission, user_type, 
                    group_id, parent_id, display_order, is_active
             FROM menu_items WHERE id = $1"
        )
        .bind(item.id)
        .fetch_optional(&pool)
        .await
        {
            Ok(Some(i)) => i,
            Ok(None) => continue, // Skip if not found
            Err(e) => return Err(AppError::InternalServerError(format!("Failed to fetch item: {}", e))),
        };

        // Check permission
        if let Some(ref module) = existing_item.required_permission {
            if !has_module_permission(&permissions, module) {
                return Err(AppError::Forbidden(format!("No permission for module '{}'", module)));
            }
        }

        // Update order and group (if provided)
        if let Some(group_id) = item.group_id {
            sqlx::query(
                "UPDATE menu_items SET display_order = $1, group_id = $2, updated_at = NOW() WHERE id = $3"
            )
            .bind(item.display_order)
            .bind(group_id)
            .bind(item.id)
            .execute(&pool)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to reorder/move item: {}", e)))?;
        } else {
            sqlx::query(
                "UPDATE menu_items SET display_order = $1, updated_at = NOW() WHERE id = $2"
            )
            .bind(item.display_order)
            .bind(item.id)
            .execute(&pool)
            .await
            .map_err(|e| AppError::InternalServerError(format!("Failed to reorder item: {}", e)))?;
        }
    }

    Ok((
        StatusCode::OK,
        JsonResponse(serde_json::json!({
            "success": true,
            "message": "Menu items reordered successfully"
        }))
    ))
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
    user_permissions.iter().any(|perm| {
        perm.starts_with(&prefix) || perm.starts_with("*.")
    })
}

/// Helper: Get pool and check module permission
async fn get_pool_and_check_module(
    state: &AppState,
    headers: &HeaderMap,
    module: &str,
) -> Result<(PgPool, User), AppError> {
    let (pool, user, permissions) = get_pool_and_authenticate_with_perms(state, headers).await?;

    // Check module permission
    if !has_module_permission(&permissions, module) {
        return Err(AppError::Forbidden(format!("No permission for module '{}'", module)));
    }

    Ok((pool, user))
}

/// Helper: Get pool and authenticate (returns pool + user)
async fn get_pool_and_authenticate(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(PgPool, User), AppError> {
    let (pool, user, _) = get_pool_and_authenticate_with_perms(state, headers).await?;
    Ok((pool, user))
}

/// Helper: Get pool and authenticate (returns pool + user + permissions)
async fn get_pool_and_authenticate_with_perms(
    state: &AppState,
    headers: &HeaderMap,
) -> Result<(PgPool, User, Vec<String>), AppError> {
    // Extract subdomain
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    // Get database URL
    let db_url = crate::db::school_mapping::get_school_database_url(&state.admin_pool, &subdomain).await
        .map_err(|e| AppError::NotFound(format!("School not found: {}", e)))?;

    // Get pool
    let pool = state.pool_manager.get_pool(&db_url, &subdomain).await
        .map_err(|e| AppError::InternalServerError(format!("Database error: {}", e)))?;

    // Get user and permissions
    let (user, permissions) = authenticate_user(headers, &pool).await?;

    Ok((pool, user, permissions))
}

/// Helper: Authenticate user and get permissions
async fn authenticate_user(
    headers: &HeaderMap,
    pool: &PgPool,
) -> Result<(User, Vec<String>), AppError> {
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
    let token = token_from_header.or(token_from_cookie)
        .ok_or(AppError::AuthError("No authentication token found".to_string()))?;

    // Validate token and extract claims
    let claims = JwtService::verify_token(&token)
        .map_err(|_| AppError::AuthError("Invalid or expired token".to_string()))?;

    // Get user from database
    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::AuthError("Invalid user ID in token".to_string()))?;

    let mut user = sqlx::query_as::<_, User>(
        "SELECT 
            id,
            username,
            national_id,
            email,
            password_hash,
            first_name,
            last_name,
            user_type,
            phone,
            date_of_birth,
            address,
            status,
            metadata,
            created_at,
            updated_at,
            title,
            nickname,
            emergency_contact,
            line_id,
            gender,
            profile_image_url,
            hired_date,
            resigned_date
         FROM users 
         WHERE id = $1"
    )
        .bind(user_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Database error: {}", e)))?
        .ok_or(AppError::AuthError("User not found".to_string()))?;

    // Decrypt national_id
    if let Some(ref nid) = user.national_id {
        if let Ok(dec) = field_encryption::decrypt(nid) {
            user.national_id = Some(dec);
        }
    }

    // Get user permissions (use unnest to handle wildcard and all permissions)
    let permissions: Vec<String> = sqlx::query_scalar(
        "SELECT DISTINCT p.code
         FROM user_roles ur
         JOIN roles r ON ur.role_id = r.id
         JOIN role_permissions rp ON r.id = rp.role_id
         JOIN permissions p ON rp.permission_id = p.id
         WHERE ur.user_id = $1 
           AND ur.ended_at IS NULL
           AND r.is_active = true"
    )
    .bind(user.id)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to fetch permissions: {}", e)))?;

    Ok((user, permissions))
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
) -> Result<impl IntoResponse, AppError> {
    let (pool, _) = get_pool_and_check_module(&state, &headers, "settings").await?;

    let mut tx = pool.begin().await
        .map_err(|e| AppError::InternalServerError(format!("Transaction failed: {}", e)))?;

    for item in &data.groups {
        if let Err(e) = sqlx::query("UPDATE menu_groups SET display_order = $1 WHERE id = $2")
            .bind(item.display_order).bind(item.id).execute(&mut *tx).await {
            let _ = tx.rollback().await;
            return Err(AppError::InternalServerError(format!("Failed to reorder: {}", e)));
        }
    }

    if let Err(e) = tx.commit().await {
         return Err(AppError::InternalServerError(format!("Failed to commit: {}", e)));
    }
    
    Ok((
        StatusCode::OK,
        JsonResponse(serde_json::json!({
            "success": true, "message": format!("Reordered {} groups", data.groups.len())
        }))
    ))
}

/// Move menu item to different group
pub async fn move_item_to_group(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    JsonResponse(data): JsonResponse<serde_json::Value>,
) -> Result<impl IntoResponse, AppError> {
    let (pool, _) = get_pool_and_check_module(&state, &headers, "settings").await?;

    let group_id = match data.get("group_id").and_then(|v| v.as_str()) {
        Some(id_str) => match Uuid::parse_str(id_str) {
            Ok(uuid) => uuid,
            Err(_) => return Err(AppError::BadRequest("Invalid group_id format".to_string())),
        },
        None => return Err(AppError::BadRequest("group_id required".to_string())),
    };

    let item = sqlx::query_as::<_, MenuItem>(
        r#"UPDATE menu_items 
           SET group_id = $1 
           WHERE id = $2 
           RETURNING id, code, name, name_en, description, path, icon, 
                     group_id, parent_id, required_permission, user_type, display_order, is_active"#
    )
    .bind(group_id)
    .bind(id)
    .fetch_one(&pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to move item: {}", e)))?;

    Ok((
        StatusCode::OK,
        JsonResponse(serde_json::json!({
            "success": true,
            "data": item
        }))
    ))
}
