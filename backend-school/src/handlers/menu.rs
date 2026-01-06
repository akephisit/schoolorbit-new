use crate::db::school_mapping::get_school_database_url;
use crate::models::menu::*;
use crate::models::auth::User;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;

use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};

use serde_json::json;
use std::collections::HashMap;
use uuid::Uuid;

/// Get user's menu items based on permissions
pub async fn get_user_menu(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let subdomain = match extract_subdomain_from_request(&headers) {
        Ok(s) => s,
        Err(response) => return response,
    };

    let db_url = match get_school_database_url(&state.admin_pool, &subdomain).await {
        Ok(url) => url,
        Err(e) => {
            eprintln!("❌ Failed to get school database: {}", e);
            return (
                StatusCode::NOT_FOUND,
                Json(json!({
                    "success": false,
                    "error": "ไม่พบโรงเรียน"
                })),
            )
                .into_response();
        }
    };

    let pool = match state.pool_manager.get_pool(&db_url, &subdomain).await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("❌ Failed to get database pool: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถเชื่อมต่อฐานข้อมูลได้"
                })),
            )
                .into_response();
        }
    };

    // Get authenticated user
    // Try to extract token from Authorization header first
    let auth_header = headers
        .get(header::AUTHORIZATION)
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
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .and_then(|cookie| crate::utils::jwt::JwtService::extract_token_from_cookie(Some(cookie)));

    // Use Authorization header first, then cookie
    let token = match token_from_header.or(token_from_cookie) {
        Some(t) => t,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "error": "กรุณาเข้าสู่ระบบ"
                })),
            ).into_response();
        }
    };
    
    let claims = match crate::utils::jwt::JwtService::verify_token(&token) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "error": "Token ไม่ถูกต้อง"
                })),
            ).into_response();
        }
    };
    
    let user: User = match sqlx::query_as("SELECT * FROM users WHERE id = $1")
        .bind(Uuid::parse_str(&claims.sub).unwrap())
        .fetch_one(&pool)
        .await
    {
        Ok(u) => u,
        Err(e) => {
            eprintln!("❌ Failed to get user: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถดึงข้อมูลผู้ใช้ได้"
                })),
            ).into_response();
        }
    };

    // Get user permissions
    let user_permissions = match get_user_permissions(&user.id, &pool).await {
        Ok(perms) => perms,
        Err(e) => {
            eprintln!("❌ Failed to get user permissions: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถดึงข้อมูล permissions ได้"
                })),
            ).into_response();
        }
    };

    // Query menu items with groups - filter by user_type
    let user_type = user.user_type.as_deref().unwrap_or("staff");
    let menu_rows: Vec<(Uuid, String, String, String, Option<String>, Option<String>, String, String, Option<String>, i32, i32)> = 
        match sqlx::query_as(
            r#"
            SELECT 
                mi.id,
                mi.code,
                mi.name,
                mi.path,
                mi.icon,
                mi.required_permission,
                mg.code as group_code,
                mg.name as group_name,
                mg.icon as group_icon,
                mg.display_order as group_order,
                mi.display_order
            FROM menu_items mi
            JOIN menu_groups mg ON mi.group_id = mg.id
            WHERE mi.is_active = true 
              AND mg.is_active = true
              AND (mi.user_type = $1 OR mi.user_type = 'all')
            ORDER BY mg.display_order, mi.display_order
            "#
        )
        .bind(user_type)
        .fetch_all(&pool)
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            eprintln!("❌ Failed to fetch menu items: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถดึงข้อมูลเมนูได้"
                })),
            ).into_response();
        }
    };

    // Group and filter menu items
    let groups = group_and_filter_menu(menu_rows, &user_permissions);

    (
        StatusCode::OK,
        Json(UserMenuResponse {
            success: true,
            groups,
        }),
    )
        .into_response()
}

/// Check if user has ANY permission in the specified module
/// Example: user_has_module_permission(&["staff.update.all"], "staff") -> true
/// Also checks for wildcard permission (*)
fn user_has_module_permission(user_permissions: &[String], module: &str) -> bool {
    // Check for wildcard permission first
    if user_permissions.contains(&"*".to_string()) {
        return true;
    }
    
    // Check for module-specific permissions
    let prefix = format!("{}.", module);
    user_permissions.iter().any(|perm| perm.starts_with(&prefix))
}

/// Group menu items by group and filter by permissions
fn group_and_filter_menu(
    rows: Vec<(Uuid, String, String, String, Option<String>, Option<String>, String, String, Option<String>, i32, i32)>,
    user_permissions: &[String],
) -> Vec<MenuGroupResponse> {
    // Intermediate struct to hold order information
    struct GroupWithOrder {
        order: i32,
        code: String,
        name: String,
        icon: Option<String>,
        items: Vec<(i32, MenuItemResponse)>, // (item_order, item)
    }
    
    let mut groups_map: HashMap<String, GroupWithOrder> = HashMap::new();

    for (id, code, name, path, icon, required_permission, group_code, group_name, group_icon, group_order, item_order) in rows {
        // Check permission - module-based matching
        if let Some(module) = &required_permission {
            if !user_has_module_permission(user_permissions, module) {
                continue; // Skip if user doesn't have any permission in this module
            }
        }

        // Get or create group
        let group = groups_map
            .entry(group_code.clone())
            .or_insert_with(|| GroupWithOrder {
                order: group_order,
                code: group_code.clone(),
                name: group_name.clone(),
                icon: group_icon.clone(),
                items: vec![],
            });

        // Add item to group with its order
        group.items.push((item_order, MenuItemResponse {
            id,
            code,
            name,
            path,
            icon,
        }));
    }

    // Convert to vector and sort
    let mut groups: Vec<GroupWithOrder> = groups_map
        .into_values()
        .filter(|g| !g.items.is_empty())
        .collect();

    // Sort groups by display_order
    groups.sort_by_key(|g| g.order);

    // Sort items within each group and convert to final format
    groups
        .into_iter()
        .map(|mut g| {
            // Sort items by display_order
            g.items.sort_by_key(|(order, _)| *order);
            
            MenuGroupResponse {
                code: g.code,
                name: g.name,
                icon: g.icon,
                items: g.items.into_iter().map(|(_, item)| item).collect(),
            }
        })
        .collect()
}

/// Get user's permissions from roles
async fn get_user_permissions(
    user_id: &Uuid,
    pool: &sqlx::PgPool,
) -> Result<Vec<String>, sqlx::Error> {
    let permissions: Vec<String> = sqlx::query_scalar(
        r#"
        SELECT DISTINCT unnest(r.permissions) as permission
        FROM user_roles ur
        JOIN roles r ON ur.role_id = r.id
        WHERE ur.user_id = $1
          AND ur.ended_at IS NULL
          AND r.is_active = true
        "#
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(permissions)
}
