use crate::error::AppError;
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::modules::staff::handlers::organization_delegations::DelegationItem;

#[derive(Serialize, sqlx::FromRow, ToSchema)]
pub struct DelegatablePermission {
    pub id: Uuid,
    pub code: String,
    pub name: String,
}

fn delegation_display_name(name: Option<String>) -> String {
    name.unwrap_or_default()
}

pub async fn list_delegatable_permissions(
    pool: &PgPool,
    organization_unit_id: Uuid,
) -> Result<Vec<DelegatablePermission>, AppError> {
    sqlx::query_as::<_, DelegatablePermission>(
        "SELECT p.id, p.code, p.name
         FROM organization_permission_grants opg
         JOIN permissions p ON p.id = opg.permission_id
         WHERE opg.organization_unit_id = $1
         GROUP BY p.id, p.code, p.name, p.module
         ORDER BY p.module, p.code",
    )
    .bind(organization_unit_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list delegatable permissions: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงสิทธิ์ที่มอบหมายได้".to_string())
    })
}

pub async fn list_delegations(
    pool: &PgPool,
    organization_unit_id: Uuid,
) -> Result<Vec<DelegationItem>, AppError> {
    let rows = sqlx::query(
        r#"SELECT opd.id, opd.from_user_id,
                  (SELECT CONCAT(u_from.title, u_from.first_name, ' ', u_from.last_name)
                   FROM users u_from WHERE u_from.id = opd.from_user_id) AS from_user_name,
                  opd.to_user_id,
                  (SELECT CONCAT(u_to.title, u_to.first_name, ' ', u_to.last_name)
                   FROM users u_to WHERE u_to.id = opd.to_user_id) AS to_user_name,
                  opd.permission_id, p.code AS permission_code, p.name AS permission_name,
                  opd.reason, opd.started_at, opd.expires_at
           FROM organization_permission_delegations opd
           JOIN permissions p ON p.id = opd.permission_id
           WHERE opd.organization_unit_id = $1
             AND opd.revoked_at IS NULL
             AND (opd.expires_at IS NULL OR opd.expires_at > NOW())
           ORDER BY opd.started_at DESC"#,
    )
    .bind(organization_unit_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| DelegationItem {
            id: row.get("id"),
            from_user_id: row.get("from_user_id"),
            from_user_name: delegation_display_name(row.get("from_user_name")),
            to_user_id: row.get("to_user_id"),
            to_user_name: delegation_display_name(row.get("to_user_name")),
            permission_id: row.get("permission_id"),
            permission_code: row.get("permission_code"),
            permission_name: row.get("permission_name"),
            reason: row.get("reason"),
            started_at: row.get("started_at"),
            expires_at: row.get("expires_at"),
        })
        .collect())
}

pub async fn is_organization_unit_leader(
    pool: &PgPool,
    user_id: Uuid,
    organization_unit_id: Uuid,
) -> Result<bool, AppError> {
    let r: bool = sqlx::query_scalar(
        "SELECT EXISTS(
             SELECT 1 FROM organization_members
             WHERE user_id = $1
               AND organization_unit_id = $2
               AND position_code IN ('director', 'deputy_director', 'head', 'deputy_head')
               AND (ended_at IS NULL OR ended_at > CURRENT_DATE)
         )",
    )
    .bind(user_id)
    .bind(organization_unit_id)
    .fetch_one(pool)
    .await?;
    Ok(r)
}

pub async fn is_organization_member(
    pool: &PgPool,
    user_id: Uuid,
    organization_unit_id: Uuid,
) -> Result<bool, AppError> {
    let r: bool = sqlx::query_scalar(
        "SELECT EXISTS(
             SELECT 1 FROM organization_members
             WHERE user_id = $1 AND organization_unit_id = $2
               AND (ended_at IS NULL OR ended_at > CURRENT_DATE)
         )",
    )
    .bind(user_id)
    .bind(organization_unit_id)
    .fetch_one(pool)
    .await?;
    Ok(r)
}

pub async fn organization_permission_grant_exists(
    pool: &PgPool,
    organization_unit_id: Uuid,
    permission_id: Uuid,
) -> Result<bool, AppError> {
    let r: bool = sqlx::query_scalar(
        "SELECT EXISTS(
             SELECT 1 FROM organization_permission_grants
             WHERE organization_unit_id = $1 AND permission_id = $2
         )",
    )
    .bind(organization_unit_id)
    .bind(permission_id)
    .fetch_one(pool)
    .await?;
    Ok(r)
}

pub async fn create_delegation(
    pool: &PgPool,
    from_user_id: Uuid,
    to_user_id: Uuid,
    permission_id: Uuid,
    organization_unit_id: Uuid,
    reason: Option<String>,
    expires_at: Option<DateTime<Utc>>,
) -> Result<Uuid, AppError> {
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO organization_permission_delegations
            (from_user_id, to_user_id, permission_id, organization_unit_id, reason, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)
         RETURNING id",
    )
    .bind(from_user_id)
    .bind(to_user_id)
    .bind(permission_id)
    .bind(organization_unit_id)
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
        "SELECT from_user_id, to_user_id
         FROM organization_permission_delegations
         WHERE id = $1 AND revoked_at IS NULL",
    )
    .bind(id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(|r| {
        (
            r.get::<Uuid, _>("from_user_id"),
            r.get::<Uuid, _>("to_user_id"),
        )
    }))
}

pub async fn revoke_delegation(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("UPDATE organization_permission_delegations SET revoked_at = NOW() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delegation_display_name_defaults_missing_names_to_empty_string() {
        assert_eq!(delegation_display_name(None), "");
        assert_eq!(
            delegation_display_name(Some("ครูสมชาย ใจดี".to_string())),
            "ครูสมชาย ใจดี"
        );
    }
}
