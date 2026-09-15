use school_migrations::sync_permissions;
use school_permissions::registry::ALL_PERMISSIONS;
use sqlx::PgPool;

async fn fixture(name: &str) -> PgPool {
    let pool = school_test_db::create_named_test_pool(name).await;
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
    let active: bool =
        sqlx::query_scalar("SELECT is_active FROM permissions WHERE code = 'retired.read.school'")
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
    let active: bool =
        sqlx::query_scalar("SELECT is_active FROM permissions WHERE code = 'retired.read.school'")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!active);
}
