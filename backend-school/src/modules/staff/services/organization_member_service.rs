use crate::error::AppError;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::modules::staff::handlers::organization_members::OrganizationMemberItem;

fn organization_member_display_name(name: Option<String>) -> String {
    name.unwrap_or_default()
}

pub async fn list_members(
    pool: &PgPool,
    organization_unit_id: Uuid,
    include_children: bool,
) -> Result<Vec<OrganizationMemberItem>, AppError> {
    let rows = if include_children {
        sqlx::query(
            r#"SELECT om.user_id, om.organization_unit_id, ou.name AS organization_unit_name,
                      CONCAT(u.title, u.first_name, ' ', u.last_name) AS name,
                      COALESCE(u.title, '') AS title, om.position_code, om.position_title,
                      om.is_primary, om.responsibilities, om.started_at
               FROM organization_members om
               JOIN users u ON u.id = om.user_id
               JOIN organization_units ou ON ou.id = om.organization_unit_id
               WHERE (om.organization_unit_id = $1 OR ou.parent_unit_id = $1)
                 AND (om.ended_at IS NULL OR om.ended_at > CURRENT_DATE)
               ORDER BY CASE om.position_code WHEN 'head' THEN 1 ELSE 2 END, ou.display_order, u.first_name"#
        ).bind(organization_unit_id).fetch_all(pool).await?
    } else {
        sqlx::query(
            r#"SELECT om.user_id, om.organization_unit_id, ou.name AS organization_unit_name,
                      CONCAT(u.title, u.first_name, ' ', u.last_name) AS name,
                      COALESCE(u.title, '') AS title, om.position_code, om.position_title,
                      om.is_primary, om.responsibilities, om.started_at
               FROM organization_members om
               JOIN users u ON u.id = om.user_id
               JOIN organization_units ou ON ou.id = om.organization_unit_id
               WHERE om.organization_unit_id = $1
                 AND (om.ended_at IS NULL OR om.ended_at > CURRENT_DATE)
               ORDER BY CASE om.position_code WHEN 'head' THEN 1 ELSE 2 END, u.first_name"#,
        )
        .bind(organization_unit_id)
        .fetch_all(pool)
        .await?
    };

    Ok(rows
        .into_iter()
        .map(|r| OrganizationMemberItem {
            user_id: r.get("user_id"),
            organization_unit_id: r.get("organization_unit_id"),
            organization_unit_name: r.get("organization_unit_name"),
            name: organization_member_display_name(r.get("name")),
            title: r.get("title"),
            position_code: r.get("position_code"),
            position_title: r.get("position_title"),
            is_primary: r.get("is_primary"),
            responsibilities: r.get("responsibilities"),
            started_at: r.get("started_at"),
        })
        .collect())
}

pub async fn already_member(
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

pub async fn add_member(
    pool: &PgPool,
    user_id: Uuid,
    organization_unit_id: Uuid,
    position_code: &str,
    position_title: Option<String>,
    is_primary: bool,
    responsibilities: Option<String>,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO organization_members
            (
                user_id, organization_unit_id, position_code, position_title,
                is_primary, responsibilities, started_at
            )
         VALUES ($1, $2, $3, $4, $5, $6, CURRENT_DATE)",
    )
    .bind(user_id)
    .bind(organization_unit_id)
    .bind(position_code)
    .bind(position_title)
    .bind(is_primary)
    .bind(responsibilities)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_member(
    pool: &PgPool,
    organization_unit_id: Uuid,
    user_id: Uuid,
    position_code: &str,
    position_title: Option<String>,
    is_primary: bool,
    responsibilities: Option<String>,
    new_organization_unit_id: Uuid,
) -> Result<u64, AppError> {
    let result = sqlx::query(
        r#"UPDATE organization_members
           SET position_code = $1,
               position_title = $2,
               is_primary = $3,
               responsibilities = $4,
               organization_unit_id = $5
           WHERE user_id = $6 AND organization_unit_id = $7
             AND (ended_at IS NULL OR ended_at > CURRENT_DATE)"#,
    )
    .bind(position_code)
    .bind(position_title)
    .bind(is_primary)
    .bind(responsibilities)
    .bind(new_organization_unit_id)
    .bind(user_id)
    .bind(organization_unit_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn remove_member(
    pool: &PgPool,
    organization_unit_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE organization_members SET ended_at = CURRENT_DATE
         WHERE user_id = $1 AND organization_unit_id = $2
           AND (ended_at IS NULL OR ended_at > CURRENT_DATE)",
    )
    .bind(user_id)
    .bind(organization_unit_id)
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn organization_member_display_name_defaults_missing_names_to_empty_string() {
        assert_eq!(organization_member_display_name(None), "");
        assert_eq!(
            organization_member_display_name(Some("ครูสมหญิง ใจดี".to_string())),
            "ครูสมหญิง ใจดี"
        );
    }
}
