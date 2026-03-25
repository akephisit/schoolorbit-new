use crate::db::school_mapping::get_school_database_url;
use crate::error::AppError;
use crate::middleware::permission::get_user_with_permissions;
use crate::permissions::registry::codes;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

#[derive(Serialize)]
pub struct DelegationItem {
    pub id: Uuid,
    pub from_user_id: Uuid,
    pub from_user_name: String,
    pub to_user_id: Uuid,
    pub to_user_name: String,
    pub permission_id: Uuid,
    pub permission_code: String,
    pub permission_name: String,
    pub reason: Option<String>,
    pub started_at: DateTime<Utc>,
    pub expires_at: Option<DateTime<Utc>>,
}

#[derive(Deserialize)]
pub struct CreateDelegationRequest {
    pub to_user_id: Uuid,
    pub permission_id: Uuid,
    pub reason: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    let subdomain = extract_subdomain_from_request(headers)
        .map_err(|_| AppError::BadRequest("Missing subdomain".to_string()))?;
    let db_url = get_school_database_url(&state.admin_client, &subdomain)
        .await
        .map_err(|_| AppError::NotFound("ไม่พบโรงเรียน".to_string()))?;
    state
        .pool_manager
        .get_pool(&db_url, &subdomain)
        .await
        .map_err(|_| AppError::InternalServerError("Database connection error".to_string()))
}

// GET /api/departments/{id}/delegatable-permissions
// Returns full permission objects for permissions assigned to this dept — for use in delegation form
pub async fn list_delegatable_permissions(
    State(state): State<AppState>,
    Path(department_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let (_, permissions) =
        match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
            Ok(r) => r,
            Err(resp) => return Ok(resp),
        };
    let has_access = permissions.contains(&"*".to_string())
        || permissions.contains(&codes::DEPT_WORK_APPROVE.to_string());
    if !has_access {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "ไม่มีสิทธิ์" })),
        )
            .into_response());
    }

    let rows = sqlx::query(
        r#"
        SELECT p.id, p.code, p.name
        FROM department_permissions dp
        JOIN permissions p ON p.id = dp.permission_id
        WHERE dp.department_id = $1
        ORDER BY p.module, p.code
        "#,
    )
    .bind(department_id)
    .fetch_all(&pool)
    .await?;

    let perms: Vec<serde_json::Value> = rows
        .into_iter()
        .map(|r| {
            json!({
                "id": r.get::<Uuid, _>("id"),
                "code": r.get::<String, _>("code"),
                "name": r.get::<String, _>("name"),
            })
        })
        .collect();

    Ok(Json(json!({ "success": true, "data": perms })).into_response())
}

// GET /api/departments/{id}/delegations
pub async fn list_delegations(
    State(state): State<AppState>,
    Path(department_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    // Auth: must have dept_work.approve.department
    let (_, permissions) =
        match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
            Ok(r) => r,
            Err(resp) => return Ok(resp),
        };
    let has_access = permissions.contains(&"*".to_string())
        || permissions.contains(&codes::DEPT_WORK_APPROVE.to_string());
    if !has_access {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "ไม่มีสิทธิ์มอบหมายงานในกลุ่มนี้" })),
        )
            .into_response());
    }

    let rows = sqlx::query(
        r#"
        SELECT
            pd.id,
            pd.from_user_id,
            (SELECT CONCAT(u_from.title, u_from.first_name, ' ', u_from.last_name)
             FROM users u_from WHERE u_from.id = pd.from_user_id) AS from_user_name,
            pd.to_user_id,
            (SELECT CONCAT(u_to.title, u_to.first_name, ' ', u_to.last_name)
             FROM users u_to WHERE u_to.id = pd.to_user_id) AS to_user_name,
            pd.permission_id,
            p.code AS permission_code,
            p.name AS permission_name,
            pd.reason,
            pd.started_at,
            pd.expires_at
        FROM permission_delegations pd
        JOIN permissions p ON p.id = pd.permission_id
        WHERE pd.department_id = $1
          AND pd.revoked_at IS NULL
          AND (pd.expires_at IS NULL OR pd.expires_at > NOW())
        ORDER BY pd.started_at DESC
        "#,
    )
    .bind(department_id)
    .fetch_all(&pool)
    .await?;

    let delegations: Vec<DelegationItem> = rows
        .into_iter()
        .map(|row| DelegationItem {
            id: row.get("id"),
            from_user_id: row.get("from_user_id"),
            from_user_name: row.get::<Option<String>, _>("from_user_name").unwrap_or_default(),
            to_user_id: row.get("to_user_id"),
            to_user_name: row.get::<Option<String>, _>("to_user_name").unwrap_or_default(),
            permission_id: row.get("permission_id"),
            permission_code: row.get("permission_code"),
            permission_name: row.get("permission_name"),
            reason: row.get("reason"),
            started_at: row.get("started_at"),
            expires_at: row.get("expires_at"),
        })
        .collect();

    Ok(Json(json!({ "success": true, "data": delegations })).into_response())
}

// POST /api/departments/{id}/delegations
pub async fn create_delegation(
    State(state): State<AppState>,
    Path(department_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<CreateDelegationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    // Auth: must have dept_work.approve.department
    let (user_id, permissions) =
        match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
            Ok(r) => r,
            Err(resp) => return Ok(resp),
        };
    let has_access = permissions.contains(&"*".to_string())
        || permissions.contains(&codes::DEPT_WORK_APPROVE.to_string());
    if !has_access {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "ไม่มีสิทธิ์มอบหมายงานในกลุ่มนี้" })),
        )
            .into_response());
    }

    // Validate: requester must be head of this department
    let is_head: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM department_members
            WHERE user_id = $1
              AND department_id = $2
              AND position = 'head'
              AND (ended_at IS NULL OR ended_at > CURRENT_DATE)
        )
        "#,
    )
    .bind(user_id)
    .bind(department_id)
    .fetch_one(&pool)
    .await?;

    if !is_head {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "เฉพาะหัวหน้าหรือรองหัวหน้ากลุ่มเท่านั้นที่สามารถมอบหมายสิทธิ์ได้" })),
        )
            .into_response());
    }

    // Validate: to_user must be a member of the same department
    let is_member: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM department_members
            WHERE user_id = $1
              AND department_id = $2
              AND (ended_at IS NULL OR ended_at > CURRENT_DATE)
        )
        "#,
    )
    .bind(body.to_user_id)
    .bind(department_id)
    .fetch_one(&pool)
    .await?;

    if !is_member {
        return Ok((
            StatusCode::BAD_REQUEST,
            Json(json!({ "success": false, "error": "ผู้รับมอบหมายต้องเป็นสมาชิกของกลุ่มนี้" })),
        )
            .into_response());
    }

    // Insert delegation
    let delegation_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO permission_delegations
            (from_user_id, to_user_id, permission_id, department_id, reason, expires_at)
        VALUES ($1, $2, $3, $4, $5, $6)
        RETURNING id
        "#,
    )
    .bind(user_id)
    .bind(body.to_user_id)
    .bind(body.permission_id)
    .bind(department_id)
    .bind(body.reason)
    .bind(body.expires_at)
    .fetch_one(&pool)
    .await?;

    // Invalidate only the recipient's cache entry
    state.permission_cache.invalidate(&body.to_user_id);

    Ok(Json(json!({ "success": true, "delegation_id": delegation_id })).into_response())
}

// DELETE /api/delegations/{id}
pub async fn revoke_delegation(
    State(state): State<AppState>,
    Path(delegation_id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    let (user_id, permissions) =
        match get_user_with_permissions(&headers, &pool, &state.permission_cache).await {
            Ok(r) => r,
            Err(resp) => return Ok(resp),
        };

    // Fetch the delegation to verify ownership and get to_user_id for cache invalidation
    let row = sqlx::query(
        "SELECT from_user_id, to_user_id FROM permission_delegations WHERE id = $1 AND revoked_at IS NULL",
    )
    .bind(delegation_id)
    .fetch_optional(&pool)
    .await?;

    let row = match row {
        Some(r) => r,
        None => {
            return Ok((
                StatusCode::NOT_FOUND,
                Json(json!({ "success": false, "error": "ไม่พบการมอบหมายสิทธิ์นี้" })),
            )
                .into_response())
        }
    };

    let from_user_id: Uuid = row.get("from_user_id");
    let to_user_id: Uuid = row.get("to_user_id");

    // Only the original delegator or a super admin (*) can revoke
    let can_revoke = user_id == from_user_id || permissions.contains(&"*".to_string());
    if !can_revoke {
        return Ok((
            StatusCode::FORBIDDEN,
            Json(json!({ "success": false, "error": "ไม่มีสิทธิ์ยกเลิกการมอบหมายนี้" })),
        )
            .into_response());
    }

    sqlx::query("UPDATE permission_delegations SET revoked_at = NOW() WHERE id = $1")
        .bind(delegation_id)
        .execute(&pool)
        .await?;

    // Invalidate only the recipient's cache entry
    state.permission_cache.invalidate(&to_user_id);

    Ok(Json(json!({ "success": true })).into_response())
}
