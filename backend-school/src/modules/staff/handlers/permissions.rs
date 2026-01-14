use crate::db::school_mapping::get_school_database_url;
use crate::middleware::permission::check_permission;
use crate::modules::staff::models::Permission;
use crate::permissions::registry::codes;
use crate::utils::subdomain::extract_subdomain_from_request;
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

pub async fn list_permissions(
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

    // Check permission
    let _user = match check_permission(&headers, &pool, codes::SETTINGS_READ).await {
        Ok(u) => u,
        Err(response) => return response,
    };

    let permissions = match sqlx::query_as::<_, Permission>(
        "SELECT * FROM permissions ORDER BY module, action, code"
    )
    .fetch_all(&pool)
    .await
    {
        Ok(perms) => perms,
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการดึงข้อมูล"
                })), 
            )
                .into_response();
        }
    };

    (
        StatusCode::OK,
        Json(json!({
            "success": true,
            "data": permissions
        })),
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

    // Check permission
    let _user = match check_permission(&headers, &pool, "settings.read").await {
        Ok(u) => u,
        Err(response) => return response,
    };

    let permissions = match sqlx::query_as::<_, Permission>(
        "SELECT * FROM permissions ORDER BY module, action, code"
    )
    .fetch_all(&pool)
    .await
    {
        Ok(perms) => perms,
        Err(e) => {
            eprintln!("❌ Database error: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "เกิดข้อผิดพลาดในการดึงข้อมูล"
                })),
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
        Json(json!({
            "success": true,
            "data": grouped
        })),
    )
        .into_response()
}
