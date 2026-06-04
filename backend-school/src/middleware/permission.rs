use crate::db::permission_cache::PermissionCache;
use crate::permissions::registry::codes;
use axum::{
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ActorContext {
    pub user_id: Uuid,
    pub permissions: Vec<String>,
}

impl ActorContext {
    pub fn has_permission(&self, required_permission: &str) -> bool {
        permission_matches(&self.permissions, required_permission)
    }

    pub fn has_any_permission(&self, required_permissions: &[&str]) -> bool {
        required_permissions
            .iter()
            .any(|permission| self.has_permission(permission))
    }

    pub fn require_permission(&self, required_permission: &str) -> Result<(), Response> {
        if self.has_permission(required_permission) {
            Ok(())
        } else {
            Err(forbidden_response(format!(
                "ไม่มีสิทธิ์ {}",
                required_permission
            )))
        }
    }

    pub fn require_any_permission(&self, required_permissions: &[&str]) -> Result<(), Response> {
        if self.has_any_permission(required_permissions) {
            Ok(())
        } else {
            Err(forbidden_response(format!(
                "ไม่มีสิทธิ์ {}",
                required_permissions.join(" หรือ ")
            )))
        }
    }
}

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
    let actor = get_actor_context(headers, pool, cache).await?;
    actor.require_permission(required_permission)?;
    Ok(actor.user_id)
}

fn extract_actor_user_id(headers: &HeaderMap) -> Result<Uuid, Response> {
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

    Ok(user_id)
}

pub async fn get_cached_user_permissions(
    user_id: Uuid,
    pool: &sqlx::PgPool,
    cache: &PermissionCache,
) -> Result<Vec<String>, sqlx::Error> {
    if let Some(permissions) = cache.get(&user_id) {
        return Ok(permissions);
    }

    let permissions = fetch_user_permissions(user_id, pool).await?;
    cache.set(user_id, permissions.clone());
    Ok(permissions)
}

pub async fn check_any_permission(
    headers: &HeaderMap,
    pool: &sqlx::PgPool,
    required_permissions: &[&str],
    cache: &PermissionCache,
) -> Result<Uuid, Response> {
    let actor = get_actor_context(headers, pool, cache).await?;
    actor.require_any_permission(required_permissions)?;
    Ok(actor.user_id)
}

pub fn permission_matches(permissions: &[String], required_permission: &str) -> bool {
    permissions
        .iter()
        .any(|permission| permission == codes::WILDCARD || permission == required_permission)
}

fn forbidden_response(error: String) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({
            "success": false,
            "error": error
        })),
    )
        .into_response()
}

/// Fetch user's effective permissions from DB (position-aware + delegations).
/// This is the single source of truth used by actor context and permission checks.
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
pub async fn get_actor_context(
    headers: &HeaderMap,
    pool: &sqlx::PgPool,
    cache: &PermissionCache,
) -> Result<ActorContext, Response> {
    let user_id = extract_actor_user_id(headers)?;
    let permissions = get_cached_user_permissions(user_id, pool, cache)
        .await
        .map_err(|_| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "ไม่สามารถตรวจสอบสิทธิ์ได้" })),
            )
                .into_response()
        })?;

    Ok(ActorContext {
        user_id,
        permissions,
    })
}
