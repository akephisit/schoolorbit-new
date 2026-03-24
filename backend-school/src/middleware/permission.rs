use crate::db::permission_cache::PermissionCache;
use crate::modules::auth::models::User;
use axum::{
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Shared permission check function
///
/// Cache hit (within 30-min TTL): 0 DB trips
///   - permissions checked from cache
///   - User returned from cache (password_hash cleared, national_id = None)
///
/// Cache miss / expired: 1 DB trip
///   - combined SQL: user + all permissions in one query
///   - stores sanitized User + permissions in cache
pub async fn check_permission(
    headers: &HeaderMap,
    pool: &sqlx::PgPool,
    required_permission: &str,
    cache: &PermissionCache,
) -> Result<User, Response> {
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

    let user_id = match uuid::Uuid::parse_str(&claims.sub) {
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
    if let Some((user, permissions)) = cache.get(&user_id) {
        let has_perm = permissions.contains(&"*".to_string())
            || permissions.contains(&required_permission.to_string());

        return if has_perm {
            Ok(user)
        } else {
            Err((
                StatusCode::FORBIDDEN,
                Json(json!({
                    "success": false,
                    "error": format!("ไม่มีสิทธิ์ {}", required_permission)
                })),
            )
                .into_response())
        };
    }

    // ── Cache miss: 1 DB trip (user + all permissions) ───────────────
    #[derive(sqlx::FromRow)]
    struct PermRow {
        #[sqlx(flatten)]
        user: User,
        permissions_json: serde_json::Value,
    }

    let row = match sqlx::query_as::<_, PermRow>(
        r#"
        SELECT
            u.id, u.username, u.national_id, u.email, u.password_hash,
            u.first_name, u.last_name, u.user_type, u.phone, u.date_of_birth,
            u.address, u.status, u.metadata, u.created_at, u.updated_at,
            u.title, u.nickname, u.emergency_contact, u.line_id, u.gender,
            u.profile_image_url, u.hired_date, u.resigned_date,
            COALESCE(
                (SELECT jsonb_agg(DISTINCT code) FROM (
                    SELECT p.code
                    FROM user_roles ur
                    JOIN role_permissions rp ON ur.role_id = rp.role_id
                    JOIN permissions p ON rp.permission_id = p.id
                    WHERE ur.user_id = u.id AND ur.ended_at IS NULL

                    UNION

                    SELECT p.code
                    FROM department_members dm
                    JOIN department_permissions dp ON dm.department_id = dp.department_id
                    JOIN permissions p ON dp.permission_id = p.id
                    WHERE dm.user_id = u.id
                      AND (dm.ended_at IS NULL OR dm.ended_at > CURRENT_DATE)
                ) AS perms),
                '[]'::jsonb
            ) AS permissions_json
        FROM users u
        WHERE u.id = $1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({ "success": false, "error": "ไม่พบข้อมูลผู้ใช้" })),
            )
                .into_response());
        }
        Err(e) => {
            eprintln!("❌ Failed to check permission: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({ "success": false, "error": "ไม่สามารถตรวจสอบสิทธิ์ได้" })),
            )
                .into_response());
        }
    };

    // Decrypt national_id for this response only (not stored in cache)
    let mut user = row.user;
    if let Some(ref nid) = user.national_id {
        match crate::utils::field_encryption::decrypt(nid) {
            Ok(dec) => user.national_id = Some(dec),
            Err(_) => user.national_id = None,
        }
    }

    let permissions: Vec<String> = row
        .permissions_json
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    // Cache sanitized user (password_hash cleared, national_id = None) + permissions
    cache.set(user_id, user.clone(), permissions.clone());

    let has_perm = permissions.contains(&"*".to_string())
        || permissions.contains(&required_permission.to_string());

    if has_perm {
        Ok(user)
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
