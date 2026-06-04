use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::modules::staff::handlers::delegations::DelegationItem;

#[derive(Serialize, sqlx::FromRow)]
pub struct DelegatablePermission {
    pub id: Uuid,
    pub code: String,
    pub name: String,
}

pub async fn list_delegatable_permissions(
    pool: &PgPool,
    department_id: Uuid,
) -> Result<Vec<DelegatablePermission>, AppError> {
    sqlx::query_as::<_, DelegatablePermission>(
        "SELECT p.id, p.code, p.name
         FROM department_permissions dp
         JOIN permissions p ON p.id = dp.permission_id
         WHERE dp.department_id = $1
         ORDER BY p.module, p.code",
    )
    .bind(department_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        eprintln!("Failed to list delegatable permissions: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงสิทธิ์ที่มอบหมายได้".to_string())
    })
}

pub async fn list_delegations(
    pool: &PgPool,
    department_id: Uuid,
) -> Result<Vec<DelegationItem>, AppError> {
    let rows = sqlx::query(
        r#"SELECT pd.id, pd.from_user_id,
                  (SELECT CONCAT(u_from.title, u_from.first_name, ' ', u_from.last_name)
                   FROM users u_from WHERE u_from.id = pd.from_user_id) AS from_user_name,
                  pd.to_user_id,
                  (SELECT CONCAT(u_to.title, u_to.first_name, ' ', u_to.last_name)
                   FROM users u_to WHERE u_to.id = pd.to_user_id) AS to_user_name,
                  pd.permission_id, p.code AS permission_code, p.name AS permission_name,
                  pd.reason, pd.started_at, pd.expires_at
           FROM permission_delegations pd
           JOIN permissions p ON p.id = pd.permission_id
           WHERE pd.department_id = $1
             AND pd.revoked_at IS NULL
             AND (pd.expires_at IS NULL OR pd.expires_at > NOW())
           ORDER BY pd.started_at DESC"#,
    )
    .bind(department_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| DelegationItem {
            id: row.get("id"),
            from_user_id: row.get("from_user_id"),
            from_user_name: row
                .get::<Option<String>, _>("from_user_name")
                .unwrap_or_default(),
            to_user_id: row.get("to_user_id"),
            to_user_name: row
                .get::<Option<String>, _>("to_user_name")
                .unwrap_or_default(),
            permission_id: row.get("permission_id"),
            permission_code: row.get("permission_code"),
            permission_name: row.get("permission_name"),
            reason: row.get("reason"),
            started_at: row.get("started_at"),
            expires_at: row.get("expires_at"),
        })
        .collect())
}

pub async fn is_department_head(
    pool: &PgPool,
    user_id: Uuid,
    department_id: Uuid,
) -> Result<bool, AppError> {
    let r: bool = sqlx::query_scalar(
        "SELECT EXISTS(
             SELECT 1 FROM department_members
             WHERE user_id = $1 AND department_id = $2 AND position = 'head'
               AND (ended_at IS NULL OR ended_at > CURRENT_DATE)
         )",
    )
    .bind(user_id)
    .bind(department_id)
    .fetch_one(pool)
    .await?;
    Ok(r)
}

pub async fn is_department_member(
    pool: &PgPool,
    user_id: Uuid,
    department_id: Uuid,
) -> Result<bool, AppError> {
    let r: bool = sqlx::query_scalar(
        "SELECT EXISTS(
             SELECT 1 FROM department_members
             WHERE user_id = $1 AND department_id = $2
               AND (ended_at IS NULL OR ended_at > CURRENT_DATE)
         )",
    )
    .bind(user_id)
    .bind(department_id)
    .fetch_one(pool)
    .await?;
    Ok(r)
}

pub async fn create_delegation(
    pool: &PgPool,
    from_user_id: Uuid,
    to_user_id: Uuid,
    permission_id: Uuid,
    department_id: Uuid,
    reason: Option<String>,
    expires_at: Option<DateTime<Utc>>,
) -> Result<Uuid, AppError> {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO permission_delegations
            (from_user_id, to_user_id, permission_id, department_id, reason, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(from_user_id)
    .bind(to_user_id)
    .bind(permission_id)
    .bind(department_id)
    .bind(reason)
    .bind(expires_at)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

pub async fn get_delegation_users(
    pool: &PgPool,
    id: Uuid,
) -> Result<Option<(Uuid, Uuid)>, AppError> {
    let row = sqlx::query(
        "SELECT from_user_id, to_user_id FROM permission_delegations WHERE id = $1 AND revoked_at IS NULL"
    )
    .bind(id).fetch_optional(pool).await?;

    Ok(row.map(|r| {
        (
            r.get::<Uuid, _>("from_user_id"),
            r.get::<Uuid, _>("to_user_id"),
        )
    }))
}

pub async fn revoke_delegation(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("UPDATE permission_delegations SET revoked_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
