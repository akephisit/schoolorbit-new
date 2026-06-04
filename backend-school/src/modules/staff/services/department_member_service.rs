use crate::error::AppError;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::modules::staff::handlers::department_members::DeptMemberItem;

pub async fn list_members(
    pool: &PgPool,
    department_id: Uuid,
    include_children: bool,
) -> Result<Vec<DeptMemberItem>, AppError> {
    let rows = if include_children {
        sqlx::query(
            r#"SELECT dm.user_id, dm.department_id, d.name AS department_name,
                      CONCAT(u.title, u.first_name, ' ', u.last_name) AS name,
                      COALESCE(u.title, '') AS title, dm.position,
                      dm.is_primary_department AS is_primary,
                      dm.responsibilities, dm.started_at
               FROM department_members dm
               JOIN users u ON u.id = dm.user_id
               JOIN departments d ON d.id = dm.department_id
               WHERE (dm.department_id = $1 OR d.parent_department_id = $1)
                 AND (dm.ended_at IS NULL OR dm.ended_at > CURRENT_DATE)
               ORDER BY CASE dm.position WHEN 'head' THEN 1 ELSE 2 END, d.display_order, u.first_name"#
        ).bind(department_id).fetch_all(pool).await?
    } else {
        sqlx::query(
            r#"SELECT dm.user_id, dm.department_id, d.name AS department_name,
                      CONCAT(u.title, u.first_name, ' ', u.last_name) AS name,
                      COALESCE(u.title, '') AS title, dm.position,
                      dm.is_primary_department AS is_primary,
                      dm.responsibilities, dm.started_at
               FROM department_members dm
               JOIN users u ON u.id = dm.user_id
               JOIN departments d ON d.id = dm.department_id
               WHERE dm.department_id = $1
                 AND (dm.ended_at IS NULL OR dm.ended_at > CURRENT_DATE)
               ORDER BY CASE dm.position WHEN 'head' THEN 1 ELSE 2 END, u.first_name"#,
        )
        .bind(department_id)
        .fetch_all(pool)
        .await?
    };

    Ok(rows
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
        .collect())
}

pub async fn already_member(
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

pub async fn add_member(
    pool: &PgPool,
    user_id: Uuid,
    department_id: Uuid,
    position: &str,
    is_primary: bool,
    responsibilities: Option<String>,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO department_members
            (user_id, department_id, position, is_primary_department, responsibilities, started_at)
         VALUES ($1, $2, $3, $4, $5, CURRENT_DATE)",
    )
    .bind(user_id)
    .bind(department_id)
    .bind(position)
    .bind(is_primary)
    .bind(responsibilities)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_member(
    pool: &PgPool,
    department_id: Uuid,
    user_id: Uuid,
    position: &str,
    is_primary: bool,
    responsibilities: Option<String>,
    new_department_id: Uuid,
) -> Result<u64, AppError> {
    let result = sqlx::query(
        r#"UPDATE department_members
           SET position = $1, is_primary_department = $2,
               responsibilities = $3, department_id = $4
           WHERE user_id = $5 AND department_id = $6
             AND (ended_at IS NULL OR ended_at > CURRENT_DATE)"#,
    )
    .bind(position)
    .bind(is_primary)
    .bind(responsibilities)
    .bind(new_department_id)
    .bind(user_id)
    .bind(department_id)
    .execute(pool)
    .await?;
    Ok(result.rows_affected())
}

pub async fn remove_member(
    pool: &PgPool,
    department_id: Uuid,
    user_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "UPDATE department_members SET ended_at = CURRENT_DATE
         WHERE user_id = $1 AND department_id = $2
           AND (ended_at IS NULL OR ended_at > CURRENT_DATE)",
    )
    .bind(user_id)
    .bind(department_id)
    .execute(pool)
    .await?;
    Ok(())
}
