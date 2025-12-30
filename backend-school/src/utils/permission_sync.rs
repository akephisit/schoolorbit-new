/// Permission sync utility - Auto-sync permission registry to database
use crate::permissions::registry::ALL_PERMISSIONS;
use sqlx::PgPool;

/// Sync all permissions from registry to database
/// This is called after migrations complete to ensure DB is up-to-date
pub async fn sync_permissions(pool: &PgPool) -> Result<(), sqlx::Error> {
    for perm in ALL_PERMISSIONS {
        sqlx::query(
            r#"
            INSERT INTO permissions (code, module, action, scope, description)
            VALUES ($1, $2, $3, $4, $5)
            ON CONFLICT (code) DO UPDATE 
            SET 
                module = EXCLUDED.module,
                action = EXCLUDED.action,
                scope = EXCLUDED.scope,
                description = EXCLUDED.description,
                updated_at = NOW()
            "#
        )
        .bind(perm.code)
        .bind(perm.module)
        .bind(perm.action)
        .bind(perm.scope)
        .bind(perm.description)
        .execute(pool)
        .await?;
    }
    
    Ok(())
}
