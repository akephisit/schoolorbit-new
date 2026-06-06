use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
};
use serde::Serialize;
use std::collections::HashMap;

use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::menu::models::*;
use crate::modules::menu::services::public_menu_service::{self, MenuRow};
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

#[derive(Debug, Serialize)]
struct UserMenuData {
    groups: Vec<MenuGroupResponse>,
}

pub async fn get_user_menu(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers)
        .await
        .map_err(|_| AppError::AuthError("ไม่สามารถดึงข้อมูล permissions ได้".to_string()))?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let user = public_menu_service::get_user(&pool, actor.user_id).await?;

    let rows = public_menu_service::fetch_menu_items(&pool, &user.user_type).await?;
    let groups = group_and_filter_menu(rows, &actor);

    Ok((
        StatusCode::OK,
        Json(ApiResponse::ok(UserMenuData { groups })),
    ))
}

fn group_and_filter_menu(rows: Vec<MenuRow>, actor: &ActorContext) -> Vec<MenuGroupResponse> {
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
            if !actor.has_module_permission(module) {
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
