use crate::db::school_mapping::get_school_database_url;
use crate::modules::staff::models::UpdateDepartmentPermissionsRequest;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;
use crate::error::AppError;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
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
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
             eprintln!("❌ Failed to get school database: {}", e);
             AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            eprintln!("❌ Failed to get database pool: {}", e);
             AppError::InternalServerError("Database connection error".to_string())
        })?;

    let rows = sqlx::query(
        r#"
        SELECT permission_id FROM department_permissions
        WHERE department_id = $1
        "#
    )
    .bind(department_id)
    .fetch_all(&pool)
    .await?;

    let permission_ids: Vec<Uuid> = rows
        .into_iter()
        .map(|row| row.get("permission_id"))
        .collect();

    Ok(Json(permission_ids))
}

// PUT /api/departments/{id}/permissions
pub async fn update_department_permissions(
    State(state): State<AppState>,
    Path(department_id): Path<Uuid>,
    headers: HeaderMap,
    Json(payload): Json<UpdateDepartmentPermissionsRequest>,
) -> Result<impl IntoResponse, AppError> {
     let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| AppError::NotFound("ไม่พบโรงเรียน".to_string()))?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| AppError::InternalServerError("Database connection error".to_string()))?;

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
            "INSERT INTO department_permissions (department_id, permission_id) VALUES ($1, $2)"
        )
        .bind(department_id)
        .bind(permission_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;

    Ok(Json(json!({
        "success": true,
        "message": "Update department permissions successfully"
    })))
}
