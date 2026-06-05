use crate::error::AppError;
use sqlx::{PgPool, Row};
use uuid::Uuid;

pub async fn list_department_permission_ids(
    pool: &PgPool,
    department_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let rows = sqlx::query(
        r#"
        SELECT permission_id FROM department_permissions
        WHERE department_id = $1
        "#,
    )
    .bind(department_id)
    .fetch_all(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to list department permissions: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงสิทธิ์ของฝ่าย".to_string())
    })?;

    Ok(rows
        .into_iter()
        .map(|row| row.get("permission_id"))
        .collect())
}

pub async fn replace_department_permissions(
    pool: &PgPool,
    department_id: Uuid,
    permission_ids: Vec<Uuid>,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await.map_err(|e| {
        tracing::error!("Failed to start department permission transaction: {}", e);
        AppError::InternalServerError("ไม่สามารถเริ่มต้น Transaction ได้".to_string())
    })?;

    sqlx::query("DELETE FROM department_permissions WHERE department_id = $1")
        .bind(department_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("Failed to clear department permissions: {}", e);
            AppError::InternalServerError("ไม่สามารถลบสิทธิ์เดิมของฝ่ายได้".to_string())
        })?;

    for permission_id in permission_ids {
        sqlx::query(
            "INSERT INTO department_permissions (department_id, permission_id) VALUES ($1, $2)",
        )
        .bind(department_id)
        .bind(permission_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| {
            tracing::error!("Failed to insert department permission: {}", e);
            AppError::InternalServerError("ไม่สามารถกำหนดสิทธิ์ของฝ่ายได้".to_string())
        })?;
    }

    tx.commit().await.map_err(|e| {
        tracing::error!("Failed to commit department permission transaction: {}", e);
        AppError::InternalServerError("ไม่สามารถบันทึกข้อมูลได้".to_string())
    })
}
