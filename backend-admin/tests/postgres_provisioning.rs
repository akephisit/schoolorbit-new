use backend_admin::{
    clients::postgres_client::PostgresClient, database_credentials::random_password,
};
use reqwest::Url;
use sqlx::{postgres::PgPoolOptions, PgPool};
use uuid::Uuid;

#[tokio::test]
async fn provisioning_isolated_roles_retry_and_refuse_unowned_database() {
    let fixture = std::env::var("TEST_DATABASE_URL").expect("owned PostgreSQL fixture is required");
    let admin = PgPool::connect(&fixture).await.unwrap();
    let suffix = Uuid::new_v4().simple().to_string();
    let provisioner = format!("so_test_{suffix}");
    let password = random_password().unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
        "CREATE ROLE {provisioner} LOGIN NOSUPERUSER CREATEDB CREATEROLE PASSWORD '{password}'"
    )))
    .execute(&admin)
    .await
    .unwrap();
    let mut url = Url::parse(&fixture).unwrap();
    url.set_username(&provisioner).unwrap();
    url.set_password(Some(&password)).unwrap();
    let client = PostgresClient::from_config(url.as_str(), &fixture).unwrap();
    let first = Uuid::now_v7();
    let second = Uuid::now_v7();
    let first_db = format!("so_test_{}", first.simple());
    let second_db = format!("so_test_{}", second.simple());
    let migration_password = random_password().unwrap();
    let runtime_password = random_password().unwrap();
    client
        .create_database(first, &first_db, &migration_password, &runtime_password)
        .await
        .unwrap();
    client
        .create_database(first, &first_db, &migration_password, &runtime_password)
        .await
        .unwrap();
    client
        .create_database(
            second,
            &second_db,
            &random_password().unwrap(),
            &random_password().unwrap(),
        )
        .await
        .unwrap();
    let runtime_url = client
        .connection_string(first, &first_db, "runtime", &runtime_password)
        .unwrap();
    let runtime = PgPool::connect(&runtime_url).await.unwrap();
    assert!(
        sqlx::query("CREATE TABLE forbidden_runtime_ddl(id integer)")
            .execute(&runtime)
            .await
            .is_err()
    );
    assert!(sqlx::query("CREATE DATABASE forbidden_runtime_db")
        .execute(&runtime)
        .await
        .is_err());
    assert!(sqlx::query("CREATE ROLE forbidden_runtime_role")
        .execute(&runtime)
        .await
        .is_err());
    let cross = client
        .connection_string(first, &second_db, "runtime", &runtime_password)
        .unwrap();
    assert!(PgPoolOptions::new()
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect(&cross)
        .await
        .is_err());
    assert!(client.delete_database(second, &first_db).await.is_err());
    assert!(client
        .create_database(second, &first_db, &migration_password, &runtime_password)
        .await
        .is_err());
    runtime.close().await;
    client.delete_database(first, &first_db).await.unwrap();
    client.delete_database(first, &first_db).await.unwrap();
    client.delete_database(second, &second_db).await.unwrap();
    sqlx::raw_sql(sqlx::AssertSqlSafe(format!("DROP ROLE {provisioner}")))
        .execute(&admin)
        .await
        .unwrap();
    admin.close().await;
}
