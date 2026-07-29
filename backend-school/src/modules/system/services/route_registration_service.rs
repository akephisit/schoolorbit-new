use crate::error::AppError;
use crate::modules::menu::models::{RouteItem, RouteRegistration};
use sqlx::{PgPool, Postgres, Transaction};
use std::collections::HashSet;

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

    let active_codes = desired_route_codes(&data.routes)?;
    let mut transaction = pool.begin().await.map_err(|error| {
        tracing::error!(
            "Failed to begin route synchronization transaction: {}",
            error
        );
        AppError::InternalServerError("Failed to synchronize menu routes".to_string())
    })?;

    let mut registered_count = 0;

    for route in &data.routes {
        let code = route_code(&route.path);

        let user_type = route_user_type(route.user_type.as_deref());
        let workspace_code = route_workspace_code(route.workspace.as_deref(), &route.group);
        ensure_route_navigation_defaults(&mut transaction, &route.group, workspace_code).await?;

        let result = sqlx::query(
            r#"
            INSERT INTO menu_items (
                id, code, name, name_en, path, icon, 
                required_permission, user_type, group_id, display_order, is_active, managed_by
            )
            VALUES (
                gen_random_uuid(),
                $1, $2, NULL, $3, $4, $5, $6,
                (SELECT id FROM menu_groups WHERE code = $7),
                $8,
                true,
                'frontend'
            )
            ON CONFLICT (code) DO UPDATE SET
                path = EXCLUDED.path,
                required_permission = EXCLUDED.required_permission,
                user_type = EXCLUDED.user_type,
                managed_by = EXCLUDED.managed_by,
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
        .execute(&mut *transaction)
        .await
        .map_err(|error| {
            tracing::error!(
                route_code = %code,
                "Failed to synchronize menu route: {}",
                error
            );
            AppError::InternalServerError("Failed to synchronize menu routes".to_string())
        })?;

        if result.rows_affected() > 0 {
            registered_count += 1;
            tracing::debug!(route_code = %code, "Synchronized menu route");
        }
    }

    let deleted_count = cleanup_orphaned_menu_items(&mut transaction, &active_codes).await?;

    transaction.commit().await.map_err(|error| {
        tracing::error!(
            "Failed to commit route synchronization transaction: {}",
            error
        );
        AppError::InternalServerError("Failed to synchronize menu routes".to_string())
    })?;

    if deleted_count > 0 {
        tracing::info!(
            deleted = deleted_count,
            "Removed stale frontend-owned menu items"
        );
    }

    tracing::info!(
        registered = registered_count,
        deleted = deleted_count,
        total = data.routes.len(),
        "Frontend route registration completed"
    );

    Ok(RouteRegistrationOutcome {
        registered: registered_count,
    })
}

fn desired_route_codes(routes: &[RouteItem]) -> Result<Vec<String>, AppError> {
    if routes.is_empty() {
        return Err(AppError::BadRequest(
            "Route synchronization requires a non-empty desired state".to_string(),
        ));
    }

    let mut seen_codes = HashSet::with_capacity(routes.len());
    let mut active_codes = Vec::with_capacity(routes.len());

    for route in routes {
        let code = route_code(&route.path);
        if code.is_empty() {
            return Err(AppError::BadRequest(
                "Route synchronization contains an empty route code".to_string(),
            ));
        }
        if !seen_codes.insert(code.clone()) {
            return Err(AppError::BadRequest(format!(
                "Route synchronization contains duplicate route code: {code}"
            )));
        }
        active_codes.push(code);
    }

    Ok(active_codes)
}

async fn ensure_route_navigation_defaults(
    transaction: &mut Transaction<'_, Postgres>,
    group_code: &str,
    workspace_code: &str,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO menu_workspaces (code, name, name_en, icon, display_order)
         VALUES ($1, $1, $1, 'panel-left', 900)
         ON CONFLICT (code) DO NOTHING",
    )
    .bind(workspace_code)
    .execute(&mut **transaction)
    .await
    .map_err(|error| {
        tracing::error!(
            workspace_code,
            "Failed to ensure route workspace: {}",
            error
        );
        AppError::InternalServerError("Failed to synchronize menu routes".to_string())
    })?;

    sqlx::query(
        "INSERT INTO menu_groups
            (code, name, name_en, icon, display_order, workspace_code)
         VALUES ($1, $1, $1, 'folder', 900, $2)
         ON CONFLICT (code) DO NOTHING",
    )
    .bind(group_code)
    .bind(workspace_code)
    .execute(&mut **transaction)
    .await
    .map_err(|error| {
        tracing::error!(group_code, "Failed to ensure route menu group: {}", error);
        AppError::InternalServerError("Failed to synchronize menu routes".to_string())
    })?;

    Ok(())
}

async fn cleanup_orphaned_menu_items(
    transaction: &mut Transaction<'_, Postgres>,
    active_codes: &[String],
) -> Result<u64, AppError> {
    sqlx::query(
        "DELETE FROM menu_items
         WHERE managed_by = 'frontend'
           AND NOT (code = ANY($1::varchar[]))",
    )
    .bind(active_codes)
    .execute(&mut **transaction)
    .await
    .map(|result| result.rows_affected())
    .map_err(|error| {
        tracing::error!(
            "Failed to clean up stale frontend-owned menu items: {}",
            error
        );
        AppError::InternalServerError("Failed to synchronize menu routes".to_string())
    })
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::menu::models::{RouteItem, RouteRegistration};
    use crate::test_helpers::{create_named_test_pool, run_test_migrations};
    use sqlx::{FromRow, PgPool};
    use uuid::Uuid;

    #[derive(Debug, FromRow)]
    struct StoredMenuRoute {
        name: String,
        icon: Option<String>,
        group_id: Option<Uuid>,
        display_order: i32,
        is_active: bool,
        path: String,
        required_permission: Option<String>,
        user_type: String,
        managed_by: String,
    }

    async fn route_sync_test_pool(test_name: &str) -> PgPool {
        let pool = create_named_test_pool(test_name).await;
        run_test_migrations(&pool).await;
        pool
    }

    fn route(path: &str, group: &str, workspace: &str, user_type: &str) -> RouteItem {
        RouteItem {
            path: path.to_string(),
            title: "Frontend title".to_string(),
            icon: Some("frontend-icon".to_string()),
            group: group.to_string(),
            workspace: Some(workspace.to_string()),
            order: 10,
            permission: Some("menu.read.all".to_string()),
            user_type: Some(user_type.to_string()),
        }
    }

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

    #[tokio::test]
    async fn synchronization_preserves_school_owned_records_and_customization() {
        let pool = route_sync_test_pool("route_sync_ownership").await;

        let custom_group_id: Uuid = sqlx::query_scalar(
            "INSERT INTO menu_groups
                (code, name, workspace_code, display_order, is_active)
             VALUES ('route_sync_custom_group', 'Custom group', 'operations', 77, true)
             RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .expect("custom group should be inserted");

        sqlx::query(
            "INSERT INTO menu_items
                (code, name, path, icon, group_id, required_permission,
                 user_type, display_order, is_active, managed_by)
             VALUES
                ('staff-route-sync-active', 'School label', '/old-path', 'school-icon', $1,
                 'old.permission', 'staff', 77, false, 'school'),
                ('school-document-link', 'School document', '/school-document', NULL, $1,
                 NULL, 'staff', 78, true, 'school'),
                ('integration-document-link', 'Integration document', '/integration-document',
                 NULL, $1, NULL, 'staff', 79, true, 'integration'),
                ('stale-frontend-route', 'Stale route', '/stale-route', NULL, $1,
                 NULL, 'staff', 80, true, 'frontend')",
        )
        .bind(custom_group_id)
        .execute(&pool)
        .await
        .expect("menu fixtures should be inserted");

        let payload = RouteRegistration {
            routes: vec![route(
                "/staff/route-sync-active",
                "academic_foundation",
                "academic",
                "student",
            )],
            environment: Some("test".to_string()),
        };

        sync_routes(&pool, &payload)
            .await
            .expect("complete desired state should synchronize");

        let active = sqlx::query_as::<_, StoredMenuRoute>(
            "SELECT name, icon, group_id, display_order, is_active, path,
                    required_permission, user_type, managed_by
             FROM menu_items
             WHERE code = 'staff-route-sync-active'",
        )
        .fetch_one(&pool)
        .await
        .expect("active frontend route should remain");

        assert_eq!(active.name, "School label");
        assert_eq!(active.icon.as_deref(), Some("school-icon"));
        assert_eq!(active.group_id, Some(custom_group_id));
        assert_eq!(active.display_order, 77);
        assert!(!active.is_active);
        assert_eq!(active.path, "/staff/route-sync-active");
        assert_eq!(active.required_permission.as_deref(), Some("menu.read.all"));
        assert_eq!(active.user_type, "student");
        assert_eq!(active.managed_by, "frontend");

        let school_owned_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM menu_items WHERE code = 'school-document-link'",
        )
        .fetch_one(&pool)
        .await
        .expect("school-owned row count should load");
        assert_eq!(school_owned_count, 1);

        let integration_owned_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM menu_items WHERE code = 'integration-document-link'",
        )
        .fetch_one(&pool)
        .await
        .expect("integration-owned row count should load");
        assert_eq!(integration_owned_count, 1);

        let stale_frontend_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM menu_items WHERE code = 'stale-frontend-route'",
        )
        .fetch_one(&pool)
        .await
        .expect("stale frontend row count should load");
        assert_eq!(stale_frontend_count, 0);
    }

    #[tokio::test]
    async fn synchronization_rolls_back_when_any_route_fails() {
        let pool = route_sync_test_pool("route_sync_rollback").await;

        sqlx::query(
            "INSERT INTO menu_items
                (code, name, path, user_type, display_order, is_active, managed_by)
             VALUES
                ('rollback-stale-route', 'Stale route', '/rollback-stale',
                 'staff', 1, true, 'frontend')",
        )
        .execute(&pool)
        .await
        .expect("stale route should be inserted");

        let payload = RouteRegistration {
            routes: vec![
                route(
                    "/staff/transaction-first",
                    "transaction_group",
                    "transaction_workspace",
                    "staff",
                ),
                route(
                    "/staff/transaction-invalid",
                    "transaction_group",
                    "transaction_workspace",
                    "invalid-user-type",
                ),
            ],
            environment: Some("test".to_string()),
        };

        assert!(sync_routes(&pool, &payload).await.is_err());

        let first_route_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM menu_items WHERE code = 'staff-transaction-first'",
        )
        .fetch_one(&pool)
        .await
        .expect("first route count should load");
        assert_eq!(first_route_count, 0);

        let workspace_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM menu_workspaces WHERE code = 'transaction_workspace'",
        )
        .fetch_one(&pool)
        .await
        .expect("workspace count should load");
        assert_eq!(workspace_count, 0);

        let stale_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM menu_items WHERE code = 'rollback-stale-route'",
        )
        .fetch_one(&pool)
        .await
        .expect("stale route count should load");
        assert_eq!(stale_count, 1);
    }

    #[tokio::test]
    async fn synchronization_rejects_empty_and_duplicate_desired_states() {
        let pool = route_sync_test_pool("route_sync_validation").await;

        let empty = RouteRegistration {
            routes: Vec::new(),
            environment: Some("test".to_string()),
        };
        assert!(sync_routes(&pool, &empty).await.is_err());

        let duplicate = RouteRegistration {
            routes: vec![
                route(
                    "/staff/duplicate-route",
                    "academic_foundation",
                    "academic",
                    "staff",
                ),
                route(
                    "/staff/duplicate-route",
                    "academic_foundation",
                    "academic",
                    "staff",
                ),
            ],
            environment: Some("test".to_string()),
        };
        assert!(sync_routes(&pool, &duplicate).await.is_err());
    }
}
