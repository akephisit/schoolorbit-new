use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Json},
};
use serde::Serialize;
use utoipa::ToSchema;

use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::menu::models::*;
use crate::modules::menu::services::public_menu_service;
use crate::utils::request_context::actor_tenant_context;
use crate::AppState;

#[derive(Debug, Serialize, ToSchema)]
pub struct UserMenuData {
    groups: Vec<MenuGroupResponse>,
}

#[utoipa::path(
    get,
    path = "/api/menu/user",
    operation_id = "getUserMenu",
    tag = "menu",
    responses(
        (status = 200, description = "Menu groups visible to the current user", body = ApiResponse<UserMenuData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse)
    )
)]
pub async fn get_user_menu(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context(&state, &headers)
        .await
        .map_err(|_| AppError::AuthError("ไม่สามารถดึงข้อมูล permissions ได้".to_string()))?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    let user_type = public_menu_service::get_user_type(&pool, actor.user_id).await?;

    let rows = public_menu_service::fetch_menu_items(&pool, &user_type).await?;
    let groups = public_menu_service::group_and_filter_menu(rows, &actor);

    Ok((
        StatusCode::OK,
        Json(ApiResponse::ok(UserMenuData { groups })),
    ))
}
