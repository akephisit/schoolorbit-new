use crate::error::AppError;
use crate::modules::staff::models::UpdateDepartmentPermissionsRequest;
use crate::modules::staff::services::department_permission_service;
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::{IntoResponse, Json},
};
use serde_json::json;
use uuid::Uuid;

// GET /api/departments/{id}/permissions
pub async fn get_department_permissions(
    State(state): State<AppState>,
    Path(department_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = resolve_tenant_pool(&state, &headers).await?;

    let permission_ids =
        department_permission_service::list_department_permission_ids(&pool, department_id).await?;

    Ok(Json(json!({ "success": true, "data": permission_ids })))
}

// PUT /api/departments/{id}/permissions
pub async fn update_department_permissions(
    State(state): State<AppState>,
    Path(department_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<UpdateDepartmentPermissionsRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = resolve_tenant_pool(&state, &headers).await?;

    department_permission_service::replace_department_permissions(
        &pool,
        department_id,
        payload.permission_ids,
    )
    .await?;

    // Department permissions changed — all members of this department have stale cache
    state.permission_cache.clear_all();
    state.notify_all_permissions_changed();

    Ok(Json(
        json!({ "success": true, "data": {}, "message": "Update department permissions successfully" }),
    ))
}
