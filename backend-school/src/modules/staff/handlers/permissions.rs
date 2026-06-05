use crate::error::AppError;
use crate::modules::staff::services::permission_service;
use crate::permissions::registry::codes;
use crate::utils::request_context::actor_tenant_context;
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
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
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
    let context = actor_tenant_context(&state, &headers).await?;
    let pool = context.tenant.pool;
    let actor = context.actor;
    actor.require_permission(codes::SETTINGS_READ)?;

    let grouped = permission_service::list_permissions_by_module(&pool).await?;

    Ok((
        StatusCode::OK,
        Json(json!({ "success": true, "data": grouped })),
    )
        .into_response())
}
