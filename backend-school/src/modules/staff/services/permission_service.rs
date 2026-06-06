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
    Ok(group_permissions_by_module(permissions))
}

fn group_permissions_by_module(permissions: Vec<Permission>) -> HashMap<String, Vec<Permission>> {
    let mut grouped: HashMap<String, Vec<Permission>> = HashMap::new();

    for permission in permissions {
        grouped
            .entry(permission.module.clone())
            .or_default()
            .push(permission);
    }

    grouped
}

#[cfg(test)]
mod tests {
    use super::*;

    fn permission(code: &str, module: &str) -> Permission {
        Permission {
            id: uuid::Uuid::new_v4(),
            code: code.to_string(),
            name: code.to_string(),
            module: module.to_string(),
            action: "read".to_string(),
            description: None,
            created_at: chrono::Utc::now(),
        }
    }

    #[test]
    fn group_permissions_by_module_groups_each_permission_under_its_module() {
        let grouped = group_permissions_by_module(vec![
            permission("ACADEMIC_READ", "academic"),
            permission("ACADEMIC_WRITE", "academic"),
            permission("STAFF_READ", "staff"),
        ]);

        assert_eq!(grouped["academic"].len(), 2);
        assert_eq!(grouped["staff"].len(), 1);
    }
}
