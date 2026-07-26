use crate::error::AppError;
use crate::modules::menu::models::RouteRegistration;
use sqlx::PgPool;

pub struct RouteRegistrationOutcome {
    pub registered: usize,
}

pub async fn sync_routes(
    pool: &PgPool,
    data: &RouteRegistration,
) -> Result<RouteRegistrationOutcome, AppError> {
    tracing::info!(
        route_count = data.routes.len(),
        environment = data.environment.as_deref().unwrap_or("unknown"),
        "Registering frontend routes"
    );

    let mut registered_count = 0;
    let mut active_codes: Vec<String> = Vec::new();

    for route in &data.routes {
        let code = route_code(&route.path);
        active_codes.push(code.clone());

        let user_type = route_user_type(route.user_type.as_deref());
        let workspace_code = route_workspace_code(route.workspace.as_deref(), &route.group);
        ensure_route_navigation_defaults(pool, &route.group, workspace_code).await?;

        let result = sqlx::query(
            r#"
            INSERT INTO menu_items (
                id, code, name, name_en, path, icon, 
                required_permission, user_type, group_id, display_order, is_active
            )
            VALUES (
                gen_random_uuid(),
                $1, $2, NULL, $3, $4, $5, $6,
                (SELECT id FROM menu_groups WHERE code = $7),
                $8,
                true
            )
            ON CONFLICT (code) DO UPDATE SET
                path = EXCLUDED.path,
                required_permission = EXCLUDED.required_permission,
                user_type = EXCLUDED.user_type,
                group_id = COALESCE(menu_items.group_id, EXCLUDED.group_id),
                display_order = COALESCE(menu_items.display_order, EXCLUDED.display_order),
                is_active = menu_items.is_active
            "#,
        )
        .bind(&code)
        .bind(&route.title)
        .bind(&route.path)
        .bind(&route.icon)
        .bind(&route.permission)
        .bind(user_type)
        .bind(&route.group)
        .bind(route.order)
        .execute(pool)
        .await;

        match result {
            Ok(result) => {
                if result.rows_affected() > 0 {
                    registered_count += 1;
                    tracing::debug!(title = %route.title, path = %route.path, "Synced menu route");
                }
            }
            Err(error) => {
                tracing::error!(
                    title = %route.title,
                    path = %route.path,
                    "Failed to sync menu route: {}",
                    error
                );
            }
        }
    }

    cleanup_orphaned_menu_items(pool, &active_codes).await;

    tracing::info!(
        registered = registered_count,
        total = data.routes.len(),
        "Frontend route registration completed"
    );

    Ok(RouteRegistrationOutcome {
        registered: registered_count,
    })
}

async fn ensure_route_navigation_defaults(
    pool: &PgPool,
    group_code: &str,
    workspace_code: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO menu_workspaces (code, name, name_en, icon, display_order)
         VALUES ($1, $1, $1, 'panel-left', 900)
         ON CONFLICT (code) DO NOTHING",
    )
    .bind(workspace_code)
    .execute(pool)
    .await
    .map_err(|error| {
        AppError::InternalServerError(format!(
            "Failed to ensure route workspace '{}': {}",
            workspace_code, error
        ))
    })?;

    sqlx::query(
        "INSERT INTO menu_groups
            (code, name, name_en, icon, display_order, workspace_code)
         VALUES ($1, $1, $1, 'folder', 900, $2)
         ON CONFLICT (code) DO NOTHING",
    )
    .bind(group_code)
    .bind(workspace_code)
    .execute(pool)
    .await
    .map_err(|error| {
        AppError::InternalServerError(format!(
            "Failed to ensure route menu group '{}': {}",
            group_code, error
        ))
    })?;

    Ok(())
}

async fn cleanup_orphaned_menu_items(pool: &PgPool, active_codes: &[String]) {
    if active_codes.is_empty() {
        return;
    }

    let in_clause = cleanup_placeholder_clause(active_codes.len());

    let delete_query = format!("DELETE FROM menu_items WHERE code NOT IN ({in_clause})");

    let mut query = sqlx::query(&delete_query);
    for code in active_codes {
        query = query.bind(code);
    }

    match query.execute(pool).await {
        Ok(result) => {
            let deleted = result.rows_affected();
            if deleted > 0 {
                tracing::info!(deleted, "Removed orphaned menu items");
            }
        }
        Err(error) => {
            tracing::warn!("Failed to clean orphaned menu items: {}", error);
        }
    }
}

fn route_code(path: &str) -> String {
    path.trim_start_matches('/').replace('/', "-")
}

fn route_user_type(user_type: Option<&str>) -> &str {
    user_type.unwrap_or("staff")
}

fn route_workspace_code<'a>(workspace: Option<&'a str>, group: &'a str) -> &'a str {
    workspace.unwrap_or(match group {
        "main" => "home",
        "academic" => "academic",
        "personnel" => "personnel",
        "budget" => "budget",
        "settings" => "settings",
        "general_admin" => "operations",
        _ => "operations",
    })
}

fn cleanup_placeholder_clause(count: usize) -> String {
    (1..=count)
        .map(|index| format!("${index}"))
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_code_removes_leading_slash_and_replaces_nested_slashes() {
        assert_eq!(route_code("/academic/timetable"), "academic-timetable");
        assert_eq!(route_code("staff"), "staff");
    }

    #[test]
    fn route_user_type_defaults_to_staff() {
        assert_eq!(route_user_type(None), "staff");
        assert_eq!(route_user_type(Some("student")), "student");
    }

    #[test]
    fn route_workspace_code_uses_explicit_value_or_group_default() {
        assert_eq!(route_workspace_code(Some("teaching"), "main"), "teaching");
        assert_eq!(route_workspace_code(None, "main"), "home");
        assert_eq!(route_workspace_code(None, "academic"), "academic");
        assert_eq!(route_workspace_code(None, "budget"), "budget");
        assert_eq!(route_workspace_code(None, "general_admin"), "operations");
    }

    #[test]
    fn cleanup_placeholder_clause_uses_one_based_placeholders() {
        assert_eq!(cleanup_placeholder_clause(3), "$1, $2, $3");
        assert_eq!(cleanup_placeholder_clause(0), "");
    }
}
