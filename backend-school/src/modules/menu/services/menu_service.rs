use crate::error::AppError;
use crate::middleware::permission::module_permission_matches;
use crate::modules::menu::models::{MenuGroup, MenuItem};
use sqlx::PgPool;
use uuid::Uuid;

// ============================================
// Menu Groups
// ============================================

pub async fn list_menu_groups(pool: &PgPool) -> Result<Vec<MenuGroup>, AppError> {
    sqlx::query_as::<_, MenuGroup>(
        "SELECT id, code, name, name_en, icon, display_order, is_active
         FROM menu_groups ORDER BY display_order, name",
    )
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to fetch menu groups: {}", e)))
}

pub struct CreateMenuGroupInput {
    pub code: String,
    pub name: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub display_order: Option<i32>,
}

pub async fn create_menu_group(
    pool: &PgPool,
    input: CreateMenuGroupInput,
) -> Result<MenuGroup, AppError> {
    sqlx::query_as::<_, MenuGroup>(
        "INSERT INTO menu_groups (code, name, name_en, description, icon, display_order)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id, code, name, name_en, icon, display_order, is_active",
    )
    .bind(&input.code)
    .bind(&input.name)
    .bind(&input.name_en)
    .bind(&input.description)
    .bind(&input.icon)
    .bind(menu_display_order(input.display_order))
    .fetch_one(pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to create menu group: {}", e)))
}

pub struct UpdateMenuGroupInput {
    pub name: Option<String>,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub icon: Option<String>,
    pub display_order: Option<i32>,
    pub is_active: Option<bool>,
}

pub async fn update_menu_group(
    pool: &PgPool,
    id: Uuid,
    data: UpdateMenuGroupInput,
) -> Result<MenuGroup, AppError> {
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
        "UPDATE menu_groups SET {} WHERE id = $1 RETURNING id, code, name, name_en, icon, display_order, is_active",
        updates.join(", ")
    );
    let mut qb = sqlx::query_as::<_, MenuGroup>(&query).bind(id);
    if let Some(v) = &data.name {
        qb = qb.bind(v);
    }
    if let Some(v) = &data.name_en {
        qb = qb.bind(v);
    }
    if let Some(v) = &data.description {
        qb = qb.bind(v);
    }
    if let Some(v) = &data.icon {
        qb = qb.bind(v);
    }
    if let Some(v) = data.display_order {
        qb = qb.bind(v);
    }
    if let Some(v) = data.is_active {
        qb = qb.bind(v);
    }

    qb.fetch_optional(pool)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to update menu group: {}", e)))?
        .ok_or(AppError::NotFound("Menu group not found".to_string()))
}

pub async fn delete_menu_group(pool: &PgPool, id: Uuid) -> Result<u64, AppError> {
    let group = sqlx::query_as::<_, MenuGroup>(
        "SELECT id, code, name, name_en, description, icon, display_order, is_active FROM menu_groups WHERE id = $1"
    ).bind(id).fetch_optional(pool).await
    .map_err(|_| AppError::NotFound("Group not found".to_string()))?
    .ok_or(AppError::NotFound("Group not found".to_string()))?;

    if group.code == "other" {
        return Err(AppError::BadRequest(
            "Cannot delete 'other' group - it serves as fallback for orphaned items".to_string(),
        ));
    }

    let other_group = sqlx::query_as::<_, MenuGroup>(
        "SELECT id, code, name, name_en, description, icon, display_order, is_active FROM menu_groups WHERE code = 'other'"
    ).fetch_one(pool).await
    .map_err(|_| AppError::InternalServerError("'other' group not found in database".to_string()))?;

    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(format!("Transaction failed: {}", e)))?;

    let moved = sqlx::query("UPDATE menu_items SET group_id = $1 WHERE group_id = $2")
        .bind(other_group.id)
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to move items: {}", e)))?
        .rows_affected();

    sqlx::query("DELETE FROM menu_groups WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to delete group: {}", e)))?;

    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to commit: {}", e)))?;

    Ok(moved)
}

// ============================================
// Menu Items
// ============================================

pub async fn list_menu_items(
    pool: &PgPool,
    group_id: Option<Uuid>,
    permissions: &[String],
) -> Result<Vec<MenuItem>, AppError> {
    let all_items = if let Some(gid) = group_id {
        sqlx::query_as::<_, MenuItem>(
            "SELECT id, code, name, name_en, path, icon, required_permission, user_type,
                    group_id, parent_id, display_order, is_active
             FROM menu_items WHERE group_id = $1 ORDER BY display_order, name",
        )
        .bind(gid)
        .fetch_all(pool)
        .await
    } else {
        sqlx::query_as::<_, MenuItem>(
            "SELECT id, code, name, name_en, path, icon, required_permission, user_type,
                    group_id, parent_id, display_order, is_active
             FROM menu_items ORDER BY group_id, display_order, name",
        )
        .fetch_all(pool)
        .await
    }
    .map_err(|e| AppError::InternalServerError(format!("Failed to fetch menu items: {}", e)))?;

    Ok(all_items
        .into_iter()
        .filter(|item| menu_item_allowed_by_permissions(item, permissions))
        .collect())
}

pub struct CreateMenuItemInput {
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

pub async fn create_menu_item(
    pool: &PgPool,
    input: CreateMenuItemInput,
) -> Result<MenuItem, AppError> {
    sqlx::query_as::<_, MenuItem>(
        "INSERT INTO menu_items
            (code, name, name_en, description, path, icon, group_id, parent_id, required_permission, display_order)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
         RETURNING id, code, name, name_en, path, icon, required_permission,
                   group_id, parent_id, user_type, display_order, is_active"
    )
    .bind(&input.code).bind(&input.name).bind(&input.name_en).bind(&input.description)
    .bind(&input.path).bind(&input.icon).bind(input.group_id).bind(input.parent_id)
    .bind(&input.required_permission).bind(menu_display_order(input.display_order))
    .fetch_one(pool).await
    .map_err(|e| AppError::InternalServerError(format!("Failed to create menu item: {}", e)))
}

pub async fn get_menu_item(pool: &PgPool, id: Uuid) -> Result<MenuItem, AppError> {
    sqlx::query_as::<_, MenuItem>(
        "SELECT id, code, name, name_en, path, icon, required_permission, user_type,
                group_id, parent_id, display_order, is_active
         FROM menu_items WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Database error: {}", e)))?
    .ok_or(AppError::NotFound("Menu item not found".to_string()))
}

pub struct UpdateMenuItemInput {
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

pub async fn update_menu_item(
    pool: &PgPool,
    id: Uuid,
    data: UpdateMenuItemInput,
) -> Result<MenuItem, AppError> {
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
    let mut qb = sqlx::query_as::<_, MenuItem>(&query).bind(id);
    if let Some(v) = &data.name {
        qb = qb.bind(v);
    }
    if let Some(v) = &data.name_en {
        qb = qb.bind(v);
    }
    if let Some(v) = &data.description {
        qb = qb.bind(v);
    }
    if let Some(v) = &data.path {
        qb = qb.bind(v);
    }
    if let Some(v) = &data.icon {
        qb = qb.bind(v);
    }
    if let Some(v) = data.group_id {
        qb = qb.bind(v);
    }
    if let Some(v) = data.parent_id {
        qb = qb.bind(v);
    }
    if let Some(v) = &data.required_permission {
        qb = qb.bind(v);
    }
    if let Some(v) = data.display_order {
        qb = qb.bind(v);
    }
    if let Some(v) = data.is_active {
        qb = qb.bind(v);
    }

    qb.fetch_optional(pool)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to update menu item: {}", e)))?
        .ok_or(AppError::NotFound("Menu item not found".to_string()))
}

pub async fn delete_menu_item(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM menu_items WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to delete menu item: {}", e)))?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("Menu item not found".to_string()));
    }
    Ok(())
}

pub async fn reorder_menu_items(
    pool: &PgPool,
    items: Vec<(Uuid, i32, Option<Uuid>)>,
    permissions: &[String],
) -> Result<usize, AppError> {
    if items.is_empty() {
        return Ok(0);
    }
    let item_ids: Vec<Uuid> = items.iter().map(|(id, _, _)| *id).collect();
    let existing_items = sqlx::query_as::<_, MenuItem>(
        "SELECT id, code, name, name_en, path, icon, required_permission, user_type,
                group_id, parent_id, display_order, is_active
         FROM menu_items WHERE id = ANY($1)",
    )
    .bind(&item_ids)
    .fetch_all(pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to fetch items batch: {}", e)))?;

    for item in &existing_items {
        if let Some(ref module) = item.required_permission {
            if !menu_item_allowed_by_permissions(item, permissions) {
                return Err(AppError::Forbidden(format!(
                    "No permission for module '{}' on item '{}'",
                    module, item.name
                )));
            }
        }
    }

    use std::collections::HashMap;
    let current_groups: HashMap<Uuid, Option<Uuid>> =
        existing_items.iter().map(|i| (i.id, i.group_id)).collect();

    let mut ids: Vec<Uuid> = Vec::with_capacity(items.len());
    let mut orders: Vec<i32> = Vec::with_capacity(items.len());
    let mut group_ids: Vec<Option<Uuid>> = Vec::with_capacity(items.len());
    for (id, order, group_id) in &items {
        ids.push(*id);
        orders.push(*order);
        if let Some(gid) = group_id {
            group_ids.push(Some(*gid));
        } else {
            group_ids.push(current_groups.get(id).cloned().flatten());
        }
    }

    sqlx::query(
        "UPDATE menu_items AS m
         SET display_order = c.display_order, group_id = c.group_id, updated_at = NOW()
         FROM (SELECT unnest($1::uuid[]) as id,
                      unnest($2::int[]) as display_order,
                      unnest($3::uuid[]) as group_id) as c
         WHERE m.id = c.id",
    )
    .bind(&ids)
    .bind(&orders)
    .bind(&group_ids)
    .execute(pool)
    .await
    .map_err(|e| AppError::InternalServerError(format!("Failed to batch reorder items: {}", e)))?;

    Ok(ids.len())
}

pub async fn reorder_menu_groups(
    pool: &PgPool,
    groups: Vec<(Uuid, i32)>,
) -> Result<usize, AppError> {
    let mut tx = pool
        .begin()
        .await
        .map_err(|e| AppError::InternalServerError(format!("Transaction failed: {}", e)))?;

    for (id, display_order) in &groups {
        if let Err(e) = sqlx::query("UPDATE menu_groups SET display_order = $1 WHERE id = $2")
            .bind(display_order)
            .bind(id)
            .execute(&mut *tx)
            .await
        {
            if let Err(rb_err) = tx.rollback().await {
                tracing::error!("⚠️ Transaction rollback failed: {}", rb_err);
            }
            return Err(AppError::InternalServerError(format!(
                "Failed to reorder: {}",
                e
            )));
        }
    }
    tx.commit()
        .await
        .map_err(|e| AppError::InternalServerError(format!("Failed to commit: {}", e)))?;
    Ok(groups.len())
}

pub async fn move_item_to_group(
    pool: &PgPool,
    id: Uuid,
    group_id: Uuid,
) -> Result<MenuItem, AppError> {
    sqlx::query_as::<_, MenuItem>(
        r#"UPDATE menu_items SET group_id = $1 WHERE id = $2
           RETURNING id, code, name, name_en, description, path, icon,
                     group_id, parent_id, required_permission, user_type, display_order, is_active"#
    )
    .bind(group_id).bind(id).fetch_one(pool).await
    .map_err(|e| AppError::InternalServerError(format!("Failed to move item: {}", e)))
}

fn menu_display_order(display_order: Option<i32>) -> i32 {
    display_order.unwrap_or(0)
}

fn menu_item_allowed_by_permissions(item: &MenuItem, permissions: &[String]) -> bool {
    if let Some(ref module) = item.required_permission {
        module_permission_matches(permissions, module)
    } else {
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn menu_item(required_permission: Option<&str>) -> MenuItem {
        MenuItem {
            id: Uuid::new_v4(),
            code: "dashboard".to_string(),
            name: "Dashboard".to_string(),
            name_en: None,
            path: "/dashboard".to_string(),
            icon: None,
            required_permission: required_permission.map(str::to_string),
            user_type: "staff".to_string(),
            group_id: None,
            parent_id: None,
            display_order: 1,
            is_active: true,
        }
    }

    #[test]
    fn menu_display_order_defaults_to_zero() {
        assert_eq!(menu_display_order(None), 0);
        assert_eq!(menu_display_order(Some(12)), 12);
    }

    #[test]
    fn menu_item_is_allowed_when_no_permission_is_required() {
        assert!(menu_item_allowed_by_permissions(
            &menu_item(None),
            &Vec::<String>::new()
        ));
    }

    #[test]
    fn menu_item_permission_filter_uses_module_permission_matching() {
        let permissions =
            vec![crate::permissions::registry::codes::ACADEMIC_CURRICULUM_READ_ALL.to_string()];

        assert!(menu_item_allowed_by_permissions(
            &menu_item(Some("academic_curriculum")),
            &permissions
        ));
        assert!(!menu_item_allowed_by_permissions(
            &menu_item(Some("staff")),
            &permissions
        ));
    }
}
