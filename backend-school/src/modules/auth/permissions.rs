use crate::modules::auth::models::User;
use sqlx::PgPool;

#[async_trait::async_trait]
pub trait UserPermissions {
    /// Check if user has a specific permission
    async fn has_permission(&self, pool: &PgPool, permission: &str) -> Result<bool, sqlx::Error>;

    /// Get all permissions for user (from all their roles)
    async fn get_permissions(&self, pool: &PgPool) -> Result<Vec<String>, sqlx::Error>;
}

#[async_trait::async_trait]
impl UserPermissions for User {
    async fn has_permission(&self, pool: &PgPool, permission: &str) -> Result<bool, sqlx::Error> {
        // Admin wildcard permission

        let permissions = self.get_permissions(pool).await?;

        // Check for wildcard permission
        if permissions.contains(&"*".to_string()) {
            return Ok(true);
        }

        // Check for specific permission
        Ok(permissions.contains(&permission.to_string()))
    }
    async fn get_permissions(&self, pool: &PgPool) -> Result<Vec<String>, sqlx::Error> {
        // Get permissions from both Roles and Departments (Consolidated)
        let permissions: Vec<String> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT code FROM (
                -- 1. Role Permissions
                SELECT p.code
                FROM user_roles ur
                JOIN role_permissions rp ON ur.role_id = rp.role_id
                JOIN permissions p ON rp.permission_id = p.id
                WHERE ur.user_id = $1 
                  AND ur.ended_at IS NULL

                UNION

                -- 2. Department Permissions
                SELECT p.code
                FROM department_members dm
                JOIN department_permissions dp ON dm.department_id = dp.department_id
                JOIN permissions p ON dp.permission_id = p.id
                WHERE dm.user_id = $1 
                  AND (dm.ended_at IS NULL OR dm.ended_at > CURRENT_DATE)
            ) AS combined_permissions
            ORDER BY code
            "#,
        )
        .bind(self.id)
        .fetch_all(pool)
        .await?;

        Ok(permissions)
    }
}
