use crate::error::AppError;
use crate::modules::staff::models::UpdateDepartmentPermissionsRequest;
use crate::utils::tenant::resolve_tenant_pool;
use crate::AppState;

use axum::{
    extract::{Path, State},
    http::HeaderMap,
    response::{IntoResponse, Json},
};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

// GET /api/departments/{id}/permissions
pub async fn get_department_permissions(
    State(state): State<AppState>,
    Path(department_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = resolve_tenant_pool(&state, &headers).await?;

    let rows = sqlx::query(
        r#"
        SELECT permission_id FROM department_permissions
        WHERE department_id = $1
        "#,
    )
    .bind(department_id)
    .fetch_all(&pool)
    .await?;

    let permission_ids: Vec<Uuid> = rows
        .into_iter()
        .map(|row| row.get("permission_id"))
        .collect();

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

    // Transaction
    let mut tx = pool.begin().await?;

    // 1. Delete old mappings
    sqlx::query("DELETE FROM department_permissions WHERE department_id = $1")
        .bind(department_id)
        .execute(&mut *tx)
        .await?;

    // 2. Insert new mappings
    for permission_id in payload.permission_ids {
        sqlx::query(
            "INSERT INTO department_permissions (department_id, permission_id) VALUES ($1, $2)",
        )
        .bind(department_id)
        .bind(permission_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    // Department permissions changed — all members of this department have stale cache
    state.permission_cache.clear_all();
    state.notify_all_permissions_changed();

    Ok(Json(
        json!({ "success": true, "data": {}, "message": "Update department permissions successfully" }),
    ))
}
