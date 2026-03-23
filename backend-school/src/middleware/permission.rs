use crate::modules::auth::models::User;
use axum::{
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

/// Combined row type: user fields + permission check result in one query
#[derive(sqlx::FromRow)]
struct PermCheckRow {
    #[sqlx(flatten)]
    user: User,
    has_permission: bool,
}

/// Shared permission check function
///
/// Single DB round trip: fetches user + checks permission in one query instead of two.
/// Previously: SELECT user (trip 1) → get_permissions (trip 2) — 2x Neon latency
/// Now: single JOIN query returns user + EXISTS(permission) in one trip
pub async fn check_permission(
    headers: &HeaderMap,
    pool: &sqlx::PgPool,
    required_permission: &str,
) -> Result<User, Response> {
    // Try to extract token from Authorization header first
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

    // Fallback to cookie
    let token_from_cookie = headers
        .get(header::COOKIE)
        .and_then(|h| h.to_str().ok())
        .and_then(|cookie| crate::utils::jwt::JwtService::extract_token_from_cookie(Some(cookie)));

    let token = match token_from_header.or(token_from_cookie) {
        Some(t) => t,
        None => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "error": "กรุณาเข้าสู่ระบบ"
                })),
            )
                .into_response());
        }
    };

    // Verify token
    let claims = match crate::utils::jwt::JwtService::verify_token(&token) {
        Ok(c) => c,
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "error": "Token ไม่ถูกต้อง"
                })),
            )
                .into_response());
        }
    };

    let user_id = match uuid::Uuid::parse_str(&claims.sub) {
        Ok(id) => id,
        Err(_) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "error": "Token ไม่ถูกต้อง"
                })),
            )
                .into_response());
        }
    };

    // Single query: fetch user + check permission in one round trip
    let row = match sqlx::query_as::<_, PermCheckRow>(
        r#"
        SELECT
            u.id, u.username, u.national_id, u.email, u.password_hash,
            u.first_name, u.last_name, u.user_type, u.phone, u.date_of_birth,
            u.address, u.status, u.metadata, u.created_at, u.updated_at,
            u.title, u.nickname, u.emergency_contact, u.line_id, u.gender,
            u.profile_image_url, u.hired_date, u.resigned_date,
            EXISTS(
                SELECT 1 FROM (
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
                ) AS perms
                WHERE perms.code IN ($2, '*')
            ) AS has_permission
        FROM users u
        WHERE u.id = $1
        "#,
    )
    .bind(user_id)
    .bind(required_permission)
    .fetch_optional(pool)
    .await
    {
        Ok(Some(r)) => r,
        Ok(None) => {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(json!({
                    "success": false,
                    "error": "ไม่พบข้อมูลผู้ใช้"
                })),
            )
                .into_response());
        }
        Err(e) => {
            eprintln!("❌ Failed to check permission: {}", e);
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(json!({
                    "success": false,
                    "error": "ไม่สามารถตรวจสอบสิทธิ์ได้"
                })),
            )
                .into_response());
        }
    };

    // Decrypt national_id
    let mut user = row.user;
    if let Some(ref nid) = user.national_id {
        match crate::utils::field_encryption::decrypt(nid) {
            Ok(decrypted) => user.national_id = Some(decrypted),
            Err(e) => {
                eprintln!(
                    "Failed to decrypt national_id for user {}: {}",
                    user.id, e
                );
                user.national_id = None;
            }
        }
    }

    if row.has_permission {
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
