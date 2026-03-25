use crate::db::permission_cache::PermissionCache;
use axum::{
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use uuid::Uuid;

/// Shared permission check — returns user_id (Uuid) on success.
///
/// Cache hit (within 30-min TTL): 0 DB trips
///   JWT verify → cache lookup → return user_id immediately
///
/// Cache miss / expired: 1 DB trip
///   permissions-only query (no user JOIN) → cache → check
pub async fn check_permission(
    headers: &HeaderMap,
    pool: &sqlx::PgPool,
    required_permission: &str,
    cache: &PermissionCache,
) -> Result<Uuid, Response> {
    // Extract token
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let token_from_header = auth_header.and_then(|h| {
        if h.starts_with("Bearer ") {
            Some(h[7..].to_string())
        } else {
            None
        }
    });

    let token_from_cookie = headers
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .and_then(|cookie| crate::utils::jwt::JwtService::extract_token_from_cookie(Some(cookie)));

    let token = match token_from_header.or(token_from_cookie) {
        Some(t) => t,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "success": false, "error": "กรุณาเข้าสู่ระบบ" })),
            )
                .into_response());
        }
    };

    let claims = match crate::utils::jwt::JwtService::verify_token(&token) {
        Ok(c) => c,
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "success": false, "error": "Token ไม่ถูกต้อง" })),
            )
                .into_response());
        }
    };

    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "success": false, "error": "Token ไม่ถูกต้อง" })),
            )
                .into_response());
        }
    };

    // ── Cache hit: 0 DB trips ────────────────────────────────────────
    if let Some(permissions) = cache.get(&user_id) {
        return check_permission_result(user_id, &permissions, required_permission);
    }

    // ── Cache miss: permissions-only query (no user JOIN) ────────────
    let permissions = fetch_user_permissions(user_id, pool).await
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": "ไม่สามารถตรวจสอบสิทธิ์ได้" })),
        ).into_response())?;

    cache.set(user_id, permissions.clone());

    check_permission_result(user_id, &permissions, required_permission)
}

fn check_permission_result(
    user_id: Uuid,
    permissions: &[String],
    required_permission: &str,
) -> Result<Uuid, Response> {
    let has_perm = permissions.contains(&"*".to_string())
        || permissions.contains(&required_permission.to_string());

    if has_perm {
        Ok(user_id)
    } else {
        Err((
            StatusCode::FORBIDDEN,
            Json(json!({
                "success": false,
                "error": format!("ไม่มีสิทธิ์ {}", required_permission)
            })),
        )
            .into_response())
    }
}

/// Fetch user's effective permissions from DB (position-aware + delegations).
/// This is the single source of truth used by both check_permission and get_user_with_permissions.
async fn fetch_user_permissions(
    user_id: Uuid,
    pool: &sqlx::PgPool,
) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar(
        r#"
        SELECT DISTINCT code FROM (
            -- 1. Role-based permissions
            SELECT p.code
            FROM user_roles ur
            JOIN role_permissions rp ON ur.role_id = rp.role_id
            JOIN permissions p ON rp.permission_id = p.id
            WHERE ur.user_id = $1 AND ur.ended_at IS NULL

            UNION

            -- 2. Department permissions (position-aware)
            --    dp.position IS NULL  → applies to all positions
            --    dp.position = dm.position → applies to that specific position only
            SELECT p.code
            FROM department_members dm
            JOIN department_permissions dp ON dm.department_id = dp.department_id
            JOIN permissions p ON dp.permission_id = p.id
            WHERE dm.user_id = $1
              AND (dm.ended_at IS NULL OR dm.ended_at > CURRENT_DATE)
              AND (dp.position IS NULL OR dp.position = dm.position)

            UNION

            -- 3. Delegated permissions (from head → this user)
            SELECT p.code
            FROM permission_delegations pd
            JOIN permissions p ON pd.permission_id = p.id
            WHERE pd.to_user_id = $1
              AND pd.revoked_at IS NULL
              AND (pd.expires_at IS NULL OR pd.expires_at > NOW())

            UNION

            -- 4. Parent-head inheritance: head of a parent dept
            --    automatically inherits permissions of all child depts
            SELECT p.code
            FROM department_members dm
            JOIN departments child ON child.parent_department_id = dm.department_id
            JOIN department_permissions dp ON dp.department_id = child.id
            JOIN permissions p ON dp.permission_id = p.id
            WHERE dm.user_id = $1
              AND dm.position = 'head'
              AND (dm.ended_at IS NULL OR dm.ended_at > CURRENT_DATE)
              AND (dp.position IS NULL OR dp.position = 'head')
        ) AS perms
        ORDER BY code
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await
}

/// Verify JWT and return (user_id, permissions) without checking a specific permission.
/// Use this when a handler needs to check multiple permissions or determine scope.
/// Returns Err(401 Response) on auth failure only.
pub async fn get_user_with_permissions(
    headers: &HeaderMap,
    pool: &sqlx::PgPool,
    cache: &PermissionCache,
) -> Result<(Uuid, Vec<String>), Response> {
    let auth_header = headers
        .get(header::AUTHORIZATION)
        .and_then(|h| h.to_str().ok());

    let token_from_header = auth_header.and_then(|h| {
        if h.starts_with("Bearer ") {
            Some(h[7..].to_string())
        } else {
            None
        }
    });

    let token_from_cookie = headers
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .and_then(|cookie| crate::utils::jwt::JwtService::extract_token_from_cookie(Some(cookie)));

    let token = match token_from_header.or(token_from_cookie) {
        Some(t) => t,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "success": false, "error": "กรุณาเข้าสู่ระบบ" })),
            ).into_response());
        }
    };

    let claims = match crate::utils::jwt::JwtService::verify_token(&token) {
        Ok(c) => c,
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "success": false, "error": "Token ไม่ถูกต้อง" })),
            ).into_response());
        }
    };

    let user_id = match Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "success": false, "error": "Token ไม่ถูกต้อง" })),
            ).into_response());
        }
    };

    if let Some(permissions) = cache.get(&user_id) {
        return Ok((user_id, permissions));
    }

    let permissions = fetch_user_permissions(user_id, pool).await
        .map_err(|_| (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "success": false, "error": "ไม่สามารถตรวจสอบสิทธิ์ได้" })),
        ).into_response())?;

    cache.set(user_id, permissions.clone());
    Ok((user_id, permissions))
}
