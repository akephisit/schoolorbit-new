use crate::middleware::permission::load_actor_context;
use crate::modules::staff::models::Permission;
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
use std::collections::HashMap;

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

    let permissions = match sqlx::query_as::<_, Permission>(
        "SELECT * FROM permissions ORDER BY module, action, code",
    )
    .fetch_all(&pool)
    .await
    {
        Ok(perms) => perms,
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "เกิดข้อผิดพลาดในการดึงข้อมูล" })),
            )
                .into_response();
        }
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

    let permissions = match sqlx::query_as::<_, Permission>(
        "SELECT * FROM permissions ORDER BY module, action, code",
    )
    .fetch_all(&pool)
    .await
    {
        Ok(perms) => perms,
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "เกิดข้อผิดพลาดในการดึงข้อมูล" })),
            )
                .into_response();
        }
    };

    // Group permissions by module
    let mut grouped: HashMap<String, Vec<Permission>> = HashMap::new();
    for perm in permissions {
        grouped
            .entry(perm.module.clone())
            .or_insert_with(Vec::new)
            .push(perm);
    }

    (
        StatusCode::OK,
        Json(json!({ "success": true, "data": grouped })),
    )
        .into_response()
}
