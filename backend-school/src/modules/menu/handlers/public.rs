use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Json},
};
use std::collections::HashMap;
use uuid::Uuid;

use crate::error::AppError;
use crate::middleware::permission::get_user_with_permissions;
use crate::modules::menu::models::*;
use crate::modules::menu::services::public_menu_service::{self, MenuRow};
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;

pub async fn get_user_menu(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = resolve_tenant_pool(&state, &headers).await?;

    let token_from_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer ").map(|t| t.to_string()));
    let token_from_cookie = headers
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .and_then(|cookie| crate::utils::jwt::JwtService::extract_token_from_cookie(Some(cookie)));

    let token = token_from_header
        .or(token_from_cookie)
        .ok_or(AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string()))?;
    let claims = crate::utils::jwt::JwtService::verify_token(&token)
        .map_err(|_| AppError::AuthError("Token ไม่ถูกต้อง".to_string()))?;

    let user_id = Uuid::parse_str(&claims.sub)
        .map_err(|_| AppError::AuthError("Invalid user ID in token".to_string()))?;
    let user = public_menu_service::get_user(&pool, user_id).await?;

    let user_permissions =
        match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
            Ok((_, perms)) => perms,
            Err(_) => {
                return Err(AppError::AuthError(
                    "ไม่สามารถดึงข้อมูล permissions ได้".to_string(),
                ))
            }
        };

    let rows = public_menu_service::fetch_menu_items(&pool, &user.user_type).await?;
    let groups = group_and_filter_menu(rows, &user_permissions);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "data": { "groups": groups }
        })),
    ))
}

fn user_has_module_permission(user_permissions: &[String], module: &str) -> bool {
    if user_permissions.contains(&"*".to_string()) {
        return true;
    }
    if user_permissions.contains(&module.to_string()) {
        return true;
    }
    let prefix = format!("{}.", module);
    user_permissions
        .iter()
        .any(|perm| perm.starts_with(&prefix))
}

fn group_and_filter_menu(
    rows: Vec<MenuRow>,
    user_permissions: &[String],
) -> Vec<MenuGroupResponse> {
    struct GroupWithOrder {
        order: i32,
        code: String,
        name: String,
        icon: Option<String>,
        items: Vec<(i32, MenuItemResponse)>,
    }

    let mut groups_map: HashMap<String, GroupWithOrder> = HashMap::new();

    for (
        id,
        code,
        name,
        path,
        icon,
        required_permission,
        group_code,
        group_name,
        group_icon,
        group_order,
        item_order,
    ) in rows
    {
        if let Some(module) = &required_permission {
            if !user_has_module_permission(user_permissions, module) {
                continue;
            }
        }

        let group = groups_map
            .entry(group_code.clone())
            .or_insert_with(|| GroupWithOrder {
                order: group_order,
                code: group_code.clone(),
                name: group_name.clone(),
                icon: group_icon.clone(),
                items: vec![],
            });

        group.items.push((
            item_order,
            MenuItemResponse {
                id,
                code,
                name,
                path,
                icon,
            },
        ));
    }

    let mut groups: Vec<GroupWithOrder> = groups_map
        .into_values()
        .filter(|g| !g.items.is_empty())
        .collect();
    groups.sort_by_key(|g| g.order);

    groups
        .into_iter()
        .map(|mut g| {
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
