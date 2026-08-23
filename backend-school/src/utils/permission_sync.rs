/// Permission sync utility - Auto-sync permission registry to database
use crate::permissions::registry::ALL_PERMISSIONS;
use sqlx::PgPool;

/// Sync all permissions from registry to database
/// This is called after migrations complete to ensure DB is up-to-date
pub async fn sync_permissions(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Step 1: Collect all permission codes from registry
    let registry_codes: Vec<&str> = ALL_PERMISSIONS.iter().map(|p| p.code).collect();

    // Step 2: Deactivate permissions not in registry. Cutover evidence and its
    // grants remain queryable for reconciliation, but can no longer authorize.
    // Build the NOT IN clause dynamically
    if !registry_codes.is_empty() {
        let placeholders: Vec<String> = (1..=registry_codes.len())
            .map(|i| format!("${}", i))
            .collect();

        let deactivate_query = format!(
            "UPDATE permissions
             SET is_active = false, updated_at = NOW()
             WHERE is_active = true AND code NOT IN ({})",
            placeholders.join(", ")
        );

        let mut query = sqlx::query(&deactivate_query);
        for code in &registry_codes {
            query = query.bind(code);
        }

        let result = query.execute(pool).await?;
        if result.rows_affected() > 0 {
            tracing::info!(
                "Deactivated {} permissions absent from the canonical registry",
                result.rows_affected()
            );
        }
    }

    // Step 3: Upsert permissions from registry
    for perm in ALL_PERMISSIONS {
        sqlx::query(
            r#"
            INSERT INTO permissions (code, name, module, action, scope, description, is_active)
            VALUES ($1, $2, $3, $4, $5, $6, true)
            ON CONFLICT (code) DO UPDATE 
            SET 
                name = EXCLUDED.name,
                module = EXCLUDED.module,
                action = EXCLUDED.action,
                scope = EXCLUDED.scope,
                description = EXCLUDED.description,
                is_active = true,
                updated_at = NOW()
            "#,
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
