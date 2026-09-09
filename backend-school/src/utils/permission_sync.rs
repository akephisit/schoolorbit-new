/// Permission sync utility - Auto-sync permission registry to database
use crate::permissions::registry::ALL_PERMISSIONS;
use sqlx::{PgPool, Postgres, QueryBuilder};

/// Sync all permissions from registry to database
/// This is called after migrations complete to ensure DB is up-to-date
pub async fn sync_permissions(pool: &PgPool) -> Result<(), sqlx::Error> {
    // Preserve the empty-registry safeguard: never deactivate every permission.
    let registry_codes: Vec<&str> = ALL_PERMISSIONS.iter().map(|p| p.code).collect();
    if registry_codes.is_empty() {
        return Ok(());
    }
    let mut transaction = pool.begin().await?;
    // Keep retired rows and their grants for reconciliation, without authorization.
    let deactivated = sqlx::query(
        "UPDATE permissions SET is_active = false, updated_at = NOW()
         WHERE is_active = true AND NOT (code = ANY($1::text[]))",
    )
    .bind(&registry_codes)
    .execute(&mut *transaction)
    .await?
    .rows_affected();

    let mut query = QueryBuilder::<Postgres>::new(
        "INSERT INTO permissions (code, name, module, action, scope, description, is_active) ",
    );
    query.push_values(ALL_PERMISSIONS, |mut row, permission| {
        row.push_bind(permission.code)
            .push_bind(permission.name)
            .push_bind(permission.module)
            .push_bind(permission.action)
            .push_bind(permission.scope)
            .push_bind(permission.description)
            .push_bind(true);
    });
    query.push(
        " ON CONFLICT (code) DO UPDATE SET
          name = EXCLUDED.name, module = EXCLUDED.module, action = EXCLUDED.action,
          scope = EXCLUDED.scope, description = EXCLUDED.description,
          is_active = true, updated_at = NOW()",
    );
    query.build().execute(&mut *transaction).await?;
    transaction.commit().await?;
    if deactivated > 0 {
        tracing::info!(
            "Deactivated {} permissions absent from the canonical registry",
            deactivated
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    async fn fixture(name: &str) -> PgPool {
        let pool = crate::test_helpers::create_named_test_pool(name).await;
        sqlx::raw_sql(
            r#"CREATE TABLE permissions (
                id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                code text UNIQUE NOT NULL, name text NOT NULL, module text NOT NULL,
                action text NOT NULL, scope text NOT NULL, description text,
                is_active boolean NOT NULL DEFAULT true, updated_at timestamptz DEFAULT now()
            );
            CREATE TABLE retained_grants (permission_id bigint REFERENCES permissions(id));
            CREATE TABLE sync_statements (id integer);
            CREATE FUNCTION count_sync_insert() RETURNS trigger LANGUAGE plpgsql AS $$
            BEGIN INSERT INTO sync_statements VALUES (1); RETURN NULL; END $$;
            CREATE TRIGGER sync_insert AFTER INSERT ON permissions
                FOR EACH STATEMENT EXECUTE FUNCTION count_sync_insert();
            INSERT INTO permissions (code, name, module, action, scope)
                VALUES ('retired.read.school', 'retired', 'retired', 'read', 'school');
            INSERT INTO retained_grants SELECT id FROM permissions;
            TRUNCATE sync_statements;"#,
        )
        .execute(&pool)
        .await
        .unwrap();
        pool
    }

    #[tokio::test]
    async fn batch_sync_preserves_grants_and_reconciles_in_one_insert_statement() {
        let pool = fixture("permission_batch_reconcile").await;
        sync_permissions(&pool).await.unwrap();
        let statements: i64 = sqlx::query_scalar("SELECT count(*) FROM sync_statements")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(statements, 1, "registry must be sent as one batch");
        let retained: bool = sqlx::query_scalar(
            "SELECT NOT p.is_active FROM retained_grants g JOIN permissions p ON p.id = g.permission_id"
        ).fetch_one(&pool).await.unwrap();
        assert!(retained);
        let first = &ALL_PERMISSIONS[0];
        let original_id: i64 = sqlx::query_scalar("SELECT id FROM permissions WHERE code = $1")
            .bind(first.code)
            .fetch_one(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE permissions SET name = 'stale', is_active = false WHERE code = $1")
            .bind(first.code)
            .execute(&pool)
            .await
            .unwrap();
        sync_permissions(&pool).await.unwrap();
        let restored: (i64, String, bool) =
            sqlx::query_as("SELECT id, name, is_active FROM permissions WHERE code = $1")
                .bind(first.code)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(restored, (original_id, first.name.to_string(), true));
        let active: i64 = sqlx::query_scalar("SELECT count(*) FROM permissions WHERE is_active")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(active, ALL_PERMISSIONS.len() as i64);
    }

    #[tokio::test]
    async fn batch_sync_failure_rolls_back_deactivation_and_can_retry() {
        let pool = fixture("permission_batch_rollback").await;
        sqlx::raw_sql(
            r#"CREATE FUNCTION reject_sync_insert() RETURNS trigger LANGUAGE plpgsql AS $$
            BEGIN RAISE EXCEPTION 'injected sync failure'; END $$;
            CREATE TRIGGER reject_sync BEFORE INSERT ON permissions
                FOR EACH STATEMENT EXECUTE FUNCTION reject_sync_insert();"#,
        )
        .execute(&pool)
        .await
        .unwrap();
        assert!(sync_permissions(&pool).await.is_err());
        let active: bool = sqlx::query_scalar(
            "SELECT is_active FROM permissions WHERE code = 'retired.read.school'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(
            active,
            "failed reconciliation must not partially deactivate permissions"
        );
        sqlx::query("DROP TRIGGER reject_sync ON permissions")
            .execute(&pool)
            .await
            .unwrap();
        sync_permissions(&pool).await.unwrap();
        let active: bool = sqlx::query_scalar(
            "SELECT is_active FROM permissions WHERE code = 'retired.read.school'",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!active);
    }
}
