use crate::middleware::permission::load_actor_context;
use crate::modules::staff::services::permission_service;
use crate::permissions::registry::codes;
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

// ===================================================================
// List All Permissions
// ===================================================================

pub async fn list_permissions(State(state): State<AppState>, headers: HeaderMap) -> Response {
    let pool = match resolve_tenant_pool(&state, &headers).await {
        Ok(pool) => pool,
        Err(error) => return error.into_response(),
    };

    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    if let Err(response) = actor.require_permission(codes::SETTINGS_READ) {
        return response;
    }

    let permissions = match permission_service::list_permissions(&pool).await {
        Ok(permissions) => permissions,
        Err(error) => return error.into_response(),
    };

    (
        StatusCode::OK,
        Json(json!({ "success": true, "data": permissions })),
    )
        .into_response()
}

// ===================================================================
// List Permissions Grouped by Module
// ===================================================================

pub async fn list_permissions_by_module(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Response {
    let pool = match resolve_tenant_pool(&state, &headers).await {
        Ok(pool) => pool,
        Err(error) => return error.into_response(),
    };

    let actor = match load_actor_context(&headers, &pool, &state.permission_cache).await {
        Ok(actor) => actor,
        Err(response) => return response,
    };
    if let Err(response) = actor.require_permission(codes::SETTINGS_READ) {
        return response;
    }

    let grouped = match permission_service::list_permissions_by_module(&pool).await {
        Ok(grouped) => grouped,
        Err(error) => return error.into_response(),
    };

    (
        StatusCode::OK,
        Json(json!({ "success": true, "data": grouped })),
    )
        .into_response()
}
