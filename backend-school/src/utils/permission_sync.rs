/// Permission sync utility - Auto-sync permission registry to database
use crate::permissions::registry::ALL_PERMISSIONS;
use sqlx::PgPool;

/// Sync all permissions from registry to database
/// This is called after migrations complete to ensure DB is up-to-date
pub async fn sync_permissions(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Step 1: Collect all permission codes from registry
    let registry_codes: Vec<&str> = ALL_PERMISSIONS
        .iter()
        .map(|p| p.code)
        .collect();
    
    // Step 2: Delete permissions not in registry
    // Build the NOT IN clause dynamically
    if !registry_codes.is_empty() {
        let placeholders: Vec<String> = (1..=registry_codes.len())
            .map(|i| format!("${}", i))
            .collect();
        
        let delete_query = format!(
            "DELETE FROM permissions WHERE code NOT IN ({})",
            placeholders.join(", ")
        );
        
        let mut query = sqlx::query(&delete_query);
        for code in &registry_codes {
            query = query.bind(code);
        }
        
        let result = query.execute(pool).await?;
        if result.rows_affected() > 0 {
            println!("🗑️  Deleted {} old permissions not in registry", result.rows_affected());
        }
    }
    
    // Step 3: Upsert permissions from registry
    for perm in ALL_PERMISSIONS {
        sqlx::query(
            r#"
            INSERT INTO permissions (code, name, module, action, scope, description)
            VALUES ($1, $2, $3, $4, $5, $6)
            ON CONFLICT (code) DO UPDATE 
            SET 
                name = EXCLUDED.name,
                module = EXCLUDED.module,
                action = EXCLUDED.action,
                scope = EXCLUDED.scope,
                description = EXCLUDED.description
            "#
        )
        .bind(perm.code)
        .bind(perm.name)
        .bind(perm.module)
        .bind(perm.action)
        .bind(perm.scope)
        .bind(perm.description)
        .execute(pool)
        .await?;
    }
    
    Ok(())
}
