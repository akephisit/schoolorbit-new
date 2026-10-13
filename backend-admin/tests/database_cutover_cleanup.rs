use backend_admin::db::{migrate_admin, verify_admin_schema};
use sqlx::PgPool;

#[tokio::test]
async fn cleanup_preserves_school_data_credentials_and_applied_history() {
    let fixture = std::env::var("TEST_DATABASE_URL").expect("owned PostgreSQL fixture required");
    let control = PgPool::connect(&fixture).await.unwrap();
    let database = format!("so_cleanup_{}", uuid::Uuid::new_v4().simple());
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("CREATE DATABASE {database}")))
        .execute(&control)
        .await
        .unwrap();
    let mut url = reqwest::Url::parse(&fixture).unwrap();
    url.set_path(&database);
    let pool = PgPool::connect(url.as_str()).await.unwrap();
    let mut predecessor = sqlx::migrate!("./migrations");
    predecessor.migrations = std::borrow::Cow::Owned(
        predecessor
            .iter()
            .filter(|m| m.version <= 8)
            .cloned()
            .collect(),
    );
    predecessor.run(&pool).await.unwrap();
    let history: Vec<(i64, Vec<u8>)> =
        sqlx::query_as("SELECT version, checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(&pool)
            .await
            .unwrap();
    let tenant = uuid::Uuid::now_v7();
    sqlx::query("INSERT INTO schools(id, name, subdomain, db_name, db_runtime_password, db_migration_password, config, db_credentials_reconciled_at, db_credentials_reconciliation) VALUES($1, 'Fixture', 'fixture', 'fixture', $2, $3, '{\"dns_record_id\":\"preserved\"}', now(), '{\"passed\":true}')")
        .bind(tenant).bind(vec![1u8, 2, 3]).bind(vec![4u8, 5, 6])
        .execute(&pool).await.unwrap();
    let before: serde_json::Value = sqlx::query_scalar(
        "SELECT to_jsonb(s) - 'db_credentials_reconciled_at' - 'db_credentials_reconciliation' FROM schools s WHERE id=$1",
    ).bind(tenant).fetch_one(&pool).await.unwrap();
    migrate_admin(&pool).await.unwrap();
    migrate_admin(&pool).await.unwrap();
    verify_admin_schema(&pool).await.unwrap();
    let after: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(s) FROM schools s WHERE id=$1")
            .bind(tenant)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(before, after);
    let preserved: Vec<(i64, Vec<u8>)> = sqlx::query_as(
        "SELECT version, checksum FROM _sqlx_migrations WHERE version <= 8 ORDER BY version",
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(history, preserved);
    let obsolete: i64 = sqlx::query_scalar("SELECT count(*) FROM information_schema.columns WHERE table_schema='public' AND table_name='schools' AND column_name IN ('db_connection_string','db_credentials_reconciled_at','db_credentials_reconciliation')")
        .fetch_one(&pool).await.unwrap();
    assert_eq!(obsolete, 0);
    pool.close().await;
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP DATABASE {database}")))
        .execute(&control)
        .await
        .unwrap();
    control.close().await;
}
