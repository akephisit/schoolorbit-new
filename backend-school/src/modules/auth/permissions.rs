use crate::models::staff::Role;
use crate::modules::auth::models::User;
use sqlx::PgPool;

#[async_trait::async_trait]
pub trait UserPermissions {
    /// Check if user has a specific role
    async fn has_role(&self, pool: &PgPool, role_code: &str) -> Result<bool, sqlx::Error>;
    
    /// Check if user has a specific permission
    async fn has_permission(&self, pool: &PgPool, permission: &str) -> Result<bool, sqlx::Error>;
    
    /// Get all roles assigned to user
    async fn get_roles(&self, pool: &PgPool) -> Result<Vec<Role>, sqlx::Error>;
    
    /// Get all permissions for user (from all their roles)
    async fn get_permissions(&self, pool: &PgPool) -> Result<Vec<String>, sqlx::Error>;
}

#[async_trait::async_trait]
impl UserPermissions for User {
    async fn has_role(&self, pool: &PgPool, role_code: &str) -> Result<bool, sqlx::Error> {
        let result: Option<bool> = sqlx::query_scalar(
            "SELECT EXISTS(
                SELECT 1 FROM user_roles ur
                JOIN roles r ON ur.role_id = r.id
                WHERE ur.user_id = $1 
                  AND r.code = $2
                  AND ur.ended_at IS NULL
                  AND r.is_active = true
            )"
        )
        .bind(self.id)
        .bind(role_code)
        .fetch_one(pool)
        .await?;
        
        Ok(result.unwrap_or(false))
    }
    
    async fn has_permission(&self, pool: &PgPool, permission: &str) -> Result<bool, sqlx::Error> {
        // Admin wildcard permission
        if self.user_type == "admin" {
            return Ok(true);
        }
        
        let permissions = self.get_permissions(pool).await?;
        
        // Check for wildcard permission
        if permissions.contains(&"*".to_string()) {
            return Ok(true);
        }
        
        // Check for specific permission
        Ok(permissions.contains(&permission.to_string()))
    }
    
    async fn get_roles(&self, pool: &PgPool) -> Result<Vec<Role>, sqlx::Error> {
        sqlx::query_as::<_, Role>(
            "SELECT r.* FROM roles r
             JOIN user_roles ur ON ur.role_id = r.id
             WHERE ur.user_id = $1 
               AND ur.ended_at IS NULL
               AND r.is_active = true
             ORDER BY ur.is_primary DESC, r.level DESC"
        )
        .bind(self.id)
        .fetch_all(pool)
        .await
    }
    
    
    async fn get_permissions(&self, pool: &PgPool) -> Result<Vec<String>, sqlx::Error> {
        // Get all permissions from user's roles (normalized schema)
        let permissions: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT p.code
             FROM user_roles ur
             JOIN role_permissions rp ON ur.role_id = rp.role_id
             JOIN permissions p ON rp.permission_id = p.id
             WHERE ur.user_id = $1 
               AND ur.ended_at IS NULL
             ORDER BY p.code"
        )
        .bind(self.id)
        .fetch_all(pool)
        .await?;
        
        Ok(permissions)
    }
}
