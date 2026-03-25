use crate::db::school_mapping::get_school_database_url;
use crate::error::AppError;
use crate::middleware::permission::check_permission;
use crate::permissions::registry::codes;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::utils::jwt::JwtService;
use crate::AppState;

use axum::{
    extract::{Path, Query, State},
    http::HeaderMap,
    response::IntoResponse,
    Json,
};
use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

#[derive(Serialize)]
pub struct DeptMemberItem {
    pub user_id: Uuid,
    pub department_id: Uuid,
    pub department_name: String,
    pub name: String,
    pub title: String,
    pub position: String,
    pub is_primary: bool,
    pub responsibilities: Option<String>,
    pub started_at: NaiveDate,
}

#[derive(Deserialize)]
pub struct ListMembersQuery {
    pub include_children: Option<bool>,
}

#[derive(Deserialize)]
pub struct AddMemberRequest {
    pub user_id: Uuid,
    pub position: String,
    pub is_primary: Option<bool>,
    pub responsibilities: Option<String>,
}

#[derive(Deserialize)]
pub struct UpdateMemberRequest {
    pub position: String,
    pub is_primary: Option<bool>,
    pub responsibilities: Option<String>,
    pub new_department_id: Option<Uuid>, // ย้ายไปฝ่ายอื่น
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

// GET /api/departments/{id}/members?include_children=true
pub async fn list_members(
    State(state): State<AppState>,
    Path(department_id): Path<Uuid>,
    Query(query): Query<ListMembersQuery>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    // list members requires only authentication (not roles.read.all)
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok())
        .and_then(|h| h.strip_prefix("Bearer ").map(|s| s.to_string()))
        .or_else(|| {
            headers.get(axum::http::header::COOKIE)
                .and_then(|h| h.to_str().ok())
                .and_then(|c| JwtService::extract_token_from_cookie(Some(c)))
        })
        .ok_or(AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string()))?;
    JwtService::verify_token(&token)
        .map_err(|_| AppError::AuthError("Token ไม่ถูกต้อง".to_string()))?;

    let include_children = query.include_children.unwrap_or(false);

    let rows = if include_children {
        sqlx::query(
            r#"
            SELECT
                dm.user_id,
                dm.department_id,
                d.name AS department_name,
                CONCAT(u.title, u.first_name, ' ', u.last_name) AS name,
                COALESCE(u.title, '') AS title,
                dm.position,
                dm.is_primary_department AS is_primary,
                dm.responsibilities,
                dm.started_at
            FROM department_members dm
            JOIN users u ON u.id = dm.user_id
            JOIN departments d ON d.id = dm.department_id
            WHERE (dm.department_id = $1 OR d.parent_department_id = $1)
              AND (dm.ended_at IS NULL OR dm.ended_at > CURRENT_DATE)
            ORDER BY
                CASE dm.position WHEN 'head' THEN 1 ELSE 2 END,
                d.display_order,
                u.first_name
            "#,
        )
        .bind(department_id)
        .fetch_all(&pool)
        .await?
    } else {
        sqlx::query(
            r#"
            SELECT
                dm.user_id,
                dm.department_id,
                d.name AS department_name,
                CONCAT(u.title, u.first_name, ' ', u.last_name) AS name,
                COALESCE(u.title, '') AS title,
                dm.position,
                dm.is_primary_department AS is_primary,
                dm.responsibilities,
                dm.started_at
            FROM department_members dm
            JOIN users u ON u.id = dm.user_id
            JOIN departments d ON d.id = dm.department_id
            WHERE dm.department_id = $1
              AND (dm.ended_at IS NULL OR dm.ended_at > CURRENT_DATE)
            ORDER BY
                CASE dm.position WHEN 'head' THEN 1 ELSE 2 END,
                u.first_name
            "#,
        )
        .bind(department_id)
        .fetch_all(&pool)
        .await?
    };

    let members: Vec<DeptMemberItem> = rows
        .into_iter()
        .map(|r| DeptMemberItem {
            user_id: r.get("user_id"),
            department_id: r.get("department_id"),
            department_name: r.get("department_name"),
            name: r.get::<Option<String>, _>("name").unwrap_or_default(),
            title: r.get("title"),
            position: r.get("position"),
            is_primary: r.get("is_primary"),
            responsibilities: r.get("responsibilities"),
            started_at: r.get("started_at"),
        })
        .collect();

    Ok(Json(json!({ "success": true, "data": members })).into_response())
}

// POST /api/departments/{id}/members
pub async fn add_member(
    State(state): State<AppState>,
    Path(department_id): Path<Uuid>,
    headers: HeaderMap,
    Json(body): Json<AddMemberRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(resp) = check_permission(&headers, &pool, codes::ROLES_ASSIGN_ALL, &state.permission_cache).await {
        return Ok(resp);
    }

    let already_member: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1 FROM department_members
            WHERE user_id = $1 AND department_id = $2
              AND (ended_at IS NULL OR ended_at > CURRENT_DATE)
        )
        "#,
    )
    .bind(body.user_id)
    .bind(department_id)
    .fetch_one(&pool)
    .await?;

    if already_member {
        return Ok(Json(json!({ "success": false, "error": "บุคลากรนี้เป็นสมาชิกของกลุ่มนี้อยู่แล้ว" })).into_response());
    }

    sqlx::query(
        r#"
        INSERT INTO department_members
            (user_id, department_id, position, is_primary_department, responsibilities, started_at)
        VALUES ($1, $2, $3, $4, $5, CURRENT_DATE)
        "#,
    )
    .bind(body.user_id)
    .bind(department_id)
    .bind(&body.position)
    .bind(body.is_primary.unwrap_or(false))
    .bind(body.responsibilities)
    .execute(&pool)
    .await?;

    state.permission_cache.invalidate(&body.user_id);

    Ok(Json(json!({ "success": true })).into_response())
}

// PUT /api/departments/{id}/members/{user_id}
pub async fn update_member(
    State(state): State<AppState>,
    Path((department_id, user_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
    Json(body): Json<UpdateMemberRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(resp) = check_permission(&headers, &pool, codes::ROLES_ASSIGN_ALL, &state.permission_cache).await {
        return Ok(resp);
    }

    let target_dept = body.new_department_id.unwrap_or(department_id);

    let updated = sqlx::query(
        r#"
        UPDATE department_members
        SET position = $1,
            is_primary_department = $2,
            responsibilities = $3,
            department_id = $4
        WHERE user_id = $5 AND department_id = $6
          AND (ended_at IS NULL OR ended_at > CURRENT_DATE)
        "#,
    )
    .bind(&body.position)
    .bind(body.is_primary.unwrap_or(false))
    .bind(body.responsibilities)
    .bind(target_dept)
    .bind(user_id)
    .bind(department_id)
    .execute(&pool)
    .await?;

    if updated.rows_affected() == 0 {
        return Ok(Json(json!({ "success": false, "error": "ไม่พบสมาชิกนี้ในกลุ่ม" })).into_response());
    }

    state.permission_cache.invalidate(&user_id);

    Ok(Json(json!({ "success": true })).into_response())
}

// DELETE /api/departments/{id}/members/{user_id}
pub async fn remove_member(
    State(state): State<AppState>,
    Path((department_id, user_id)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;

    if let Err(resp) = check_permission(&headers, &pool, codes::ROLES_ASSIGN_ALL, &state.permission_cache).await {
        return Ok(resp);
    }

    sqlx::query(
        "UPDATE department_members SET ended_at = CURRENT_DATE WHERE user_id = $1 AND department_id = $2 AND (ended_at IS NULL OR ended_at > CURRENT_DATE)"
    )
    .bind(user_id)
    .bind(department_id)
    .execute(&pool)
    .await?;

    state.permission_cache.invalidate(&user_id);

    Ok(Json(json!({ "success": true })).into_response())
}
