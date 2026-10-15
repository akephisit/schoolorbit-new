use axum::{
    extract::{Extension, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use serde::Serialize;
#[cfg(feature = "api-docs")]
use utoipa::ToSchema;

use crate::modules::menu::models::*;
use crate::modules::menu::services::public_menu_service;
use crate::utils::request_context::actor_tenant_context_from_session;
use crate::AppState;
use school_auth::session_service::AuthenticatedSession;
#[cfg(feature = "api-docs")]
use school_http::ApiErrorResponse;
use school_http::ApiResponse;
use school_http::HttpError as AppError;

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct UserMenuData {
    groups: Vec<MenuGroupResponse>,
}

#[cfg_attr(feature = "api-docs", utoipa::path(
    get,
    path = "/api/menu/user",
    operation_id = "getUserMenu",
    tag = "menu",
    responses(
        (status = 200, description = "Menu groups visible to the current user", body = ApiResponse<UserMenuData>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse)
    )
))]
pub async fn get_user_menu(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = actor_tenant_context_from_session(&state, &session)
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
