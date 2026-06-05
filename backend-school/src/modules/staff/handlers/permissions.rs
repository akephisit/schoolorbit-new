use crate::error::AppError;
use crate::middleware::permission::load_actor_context;
use crate::modules::staff::services::permission_service;
use crate::permissions::registry::codes;
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::json;

// ===================================================================
// List All Permissions
// ===================================================================

pub async fn list_permissions(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = resolve_tenant_pool(&state, &headers).await?;

    let actor = load_actor_context(&headers, &pool, &state.permission_cache).await?;
    actor.require_permission(codes::SETTINGS_READ)?;

    let permissions = permission_service::list_permissions(&pool).await?;

    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": permissions })),
    )
        .into_response())
}

// ===================================================================
// List Permissions Grouped by Module
// ===================================================================

pub async fn list_permissions_by_module(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = resolve_tenant_pool(&state, &headers).await?;

    let actor = load_actor_context(&headers, &pool, &state.permission_cache).await?;
    actor.require_permission(codes::SETTINGS_READ)?;

    let grouped = permission_service::list_permissions_by_module(&pool).await?;

    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": grouped })),
    )
        .into_response())
}
