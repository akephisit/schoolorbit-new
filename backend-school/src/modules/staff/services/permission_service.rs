use crate::error::AppError;
use crate::modules::staff::models::Permission;
use sqlx::PgPool;
use std::collections::HashMap;

pub async fn list_permissions(pool: &PgPool) -> Result<Vec<Permission>, AppError> {
    sqlx::query_as::<_, Permission>("SELECT * FROM permissions ORDER BY module, action, code")
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to list permissions: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาดในการดึงข้อมูล".to_string())
        })
}

pub async fn list_permissions_by_module(
    pool: &PgPool,
) -> Result<HashMap<String, Vec<Permission>>, AppError> {
    let permissions = list_permissions(pool).await?;
    let mut grouped: HashMap<String, Vec<Permission>> = HashMap::new();

    for permission in permissions {
        grouped
            .entry(permission.module.clone())
            .or_default()
            .push(permission);
    }

    Ok(grouped)
}
