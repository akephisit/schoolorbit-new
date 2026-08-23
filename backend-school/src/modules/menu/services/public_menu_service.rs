use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::menu::models::{MenuGroupResponse, MenuItemResponse};
use sqlx::FromRow;
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

pub async fn get_user_type(pool: &PgPool, user_id: Uuid) -> Result<String, AppError> {
    sqlx::query_scalar(
        "SELECT user_type
         FROM users
         WHERE id = $1",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to get user type for menu: {}", e);
        AppError::InternalServerError("ไม่สามารถตรวจสอบประเภทผู้ใช้ได้".to_string())
    })
}

#[derive(Debug, FromRow)]
pub struct MenuRow {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub path: String,
    pub icon: Option<String>,
    pub required_permission: Option<String>,
    pub group_code: String,
    pub group_name: String,
    pub group_icon: Option<String>,
    pub group_order: i32,
    pub group_workspace_code: String,
    pub workspace_name: String,
    pub workspace_icon: Option<String>,
    pub workspace_order: i32,
    pub item_order: i32,
}

pub async fn fetch_menu_items(pool: &PgPool, user_type: &str) -> Result<Vec<MenuRow>, AppError> {
    sqlx::query_as::<_, MenuRow>(
        r#"SELECT mi.id, mi.code, mi.name, mi.path, mi.icon, mi.required_permission,
                  mg.code as group_code, mg.name as group_name, mg.icon as group_icon,
                  mg.display_order as group_order, mg.workspace_code as group_workspace_code,
                  mw.name as workspace_name, mw.icon as workspace_icon,
                  mw.display_order as workspace_order, mi.display_order as item_order
           FROM menu_items mi
           JOIN menu_groups mg ON mi.group_id = mg.id
           JOIN menu_workspaces mw ON mw.code = mg.workspace_code
           WHERE mi.is_active = true
             AND mg.is_active = true
             AND mw.is_active = true
             AND mi.user_type = $1
           ORDER BY mw.display_order, mg.display_order, mi.display_order"#,
    )
    .bind(user_type)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to fetch menu items: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลเมนูได้".to_string())
    })
}

pub fn group_and_filter_menu(rows: Vec<MenuRow>, actor: &ActorContext) -> Vec<MenuGroupResponse> {
    struct GroupWithOrder {
        order: i32,
        code: String,
        name: String,
        icon: Option<String>,
        workspace_code: String,
        workspace_name: String,
        workspace_icon: Option<String>,
        workspace_order: i32,
        items: Vec<(i32, MenuItemResponse)>,
    }

    let mut groups_map: HashMap<String, GroupWithOrder> = HashMap::new();

    for row in rows {
        if let Some(required_permission) = &row.required_permission {
            let can_open_menu = if required_permission.contains('|') {
                required_permission
                    .split('|')
                    .map(str::trim)
                    .filter(|permission| !permission.is_empty())
                    .any(|permission| actor.has_permission(permission))
            } else {
                actor.has_module_permission(required_permission)
            };

            if !can_open_menu {
                continue;
            }
        }

        let group = groups_map
            .entry(row.group_code.clone())
            .or_insert_with(|| GroupWithOrder {
                order: row.group_order,
                code: row.group_code.clone(),
                name: row.group_name.clone(),
                icon: row.group_icon.clone(),
                workspace_code: row.group_workspace_code.clone(),
                workspace_name: row.workspace_name.clone(),
                workspace_icon: row.workspace_icon.clone(),
                workspace_order: row.workspace_order,
                items: vec![],
            });

        group.items.push((
            row.item_order,
            MenuItemResponse {
                id: row.id,
                code: row.code,
                name: row.name,
                path: row.path,
                icon: row.icon,
            },
        ));
    }

    let mut groups: Vec<GroupWithOrder> = groups_map
        .into_values()
        .filter(|group| !group.items.is_empty())
        .collect();
    groups.sort_by(|left, right| {
        (left.workspace_order, left.order)
            .cmp(&(right.workspace_order, right.order))
            .then_with(|| left.name.cmp(&right.name))
    });

    groups
        .into_iter()
        .map(|mut group| {
            group
                .items
                .sort_by(|(left_order, left_item), (right_order, right_item)| {
                    left_order
                        .cmp(right_order)
                        .then_with(|| left_item.name.cmp(&right_item.name))
                });
            MenuGroupResponse {
                code: group.code,
                name: group.name,
                icon: group.icon,
                display_order: group.order,
                workspace_code: group.workspace_code,
                workspace_name: group.workspace_name,
                workspace_icon: group.workspace_icon,
                workspace_order: group.workspace_order,
                items: group.items.into_iter().map(|(_, item)| item).collect(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn actor(permissions: &[&str]) -> ActorContext {
        ActorContext {
            user_id: Uuid::new_v4(),
            permissions: permissions
                .iter()
                .map(|permission| permission.to_string())
                .collect(),
        }
    }

    fn menu_row(
        code: &str,
        required_permission: Option<&str>,
        group_code: &str,
        group_order: i32,
        workspace_code: &str,
        workspace_order: i32,
        item_order: i32,
    ) -> MenuRow {
        MenuRow {
            id: Uuid::new_v4(),
            code: code.to_string(),
            name: code.to_string(),
            path: format!("/staff/{code}"),
            icon: None,
            required_permission: required_permission.map(str::to_string),
            group_code: group_code.to_string(),
            group_name: group_code.to_string(),
            group_icon: None,
            group_order,
            group_workspace_code: workspace_code.to_string(),
            workspace_name: workspace_code.to_string(),
            workspace_icon: None,
            workspace_order,
            item_order,
        }
    }

    #[test]
    fn keeps_navigation_metadata_and_filters_by_effective_module_permission() {
        let rows = vec![
            menu_row(
                "curriculum-later",
                Some("academic_curriculum"),
                "curriculum",
                20,
                "academic",
                20,
                20,
            ),
            menu_row(
                "curriculum-first",
                Some("academic_curriculum"),
                "curriculum",
                20,
                "academic",
                20,
                10,
            ),
            menu_row("staff", Some("staff"), "personnel", 10, "personnel", 40, 10),
        ];

        let groups = group_and_filter_menu(
            rows,
            &actor(&[crate::permissions::registry::codes::ACADEMIC_CURRICULUM_READ_SCHOOL]),
        );

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].code, "curriculum");
        assert_eq!(groups[0].workspace_code, "academic");
        assert_eq!(groups[0].workspace_order, 20);
        assert_eq!(
            groups[0]
                .items
                .iter()
                .map(|item| item.code.as_str())
                .collect::<Vec<_>>(),
            vec!["curriculum-first", "curriculum-later"]
        );
    }

    #[test]
    fn keeps_routes_without_a_required_module() {
        let groups = group_and_filter_menu(
            vec![menu_row("my-home", None, "daily", 10, "home", 10, 10)],
            &actor(&[]),
        );

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].items[0].code, "my-home");
    }

    #[test]
    fn alternative_exact_permissions_keep_menu_for_either_scope_only() {
        let required = "certificate.read.organization_unit|certificate.read.school";

        for permission in [
            crate::permissions::registry::codes::CERTIFICATE_READ_ORGANIZATION_UNIT,
            crate::permissions::registry::codes::CERTIFICATE_READ_SCHOOL,
        ] {
            let groups = group_and_filter_menu(
                vec![menu_row(
                    "certificates",
                    Some(required),
                    "academic-services",
                    10,
                    "academic",
                    20,
                    10,
                )],
                &actor(&[permission]),
            );

            assert_eq!(groups.len(), 1, "expected menu for {permission}");
        }

        let own_only = group_and_filter_menu(
            vec![menu_row(
                "certificates",
                Some(required),
                "academic-services",
                10,
                "academic",
                20,
                10,
            )],
            &actor(&[crate::permissions::registry::codes::CERTIFICATE_READ_OWN]),
        );

        assert!(own_only.is_empty());
    }
}
