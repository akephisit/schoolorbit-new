use sqlx::{postgres::PgPoolOptions, PgPool};
use std::env;

static MIGRATION_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// Create a test database pool
pub async fn create_test_pool() -> PgPool {
    dotenvy::dotenv().ok();

    let database_url = env::var("TEST_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .expect("TEST_DATABASE_URL or DATABASE_URL must be set for tests");

    PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("Failed to connect to test database")
}

/// Run migrations on test database
pub async fn run_test_migrations(pool: &PgPool) {
    let _guard = MIGRATION_LOCK.lock().await;
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .expect("Failed to run migrations");
}

/// Clean up test data
pub async fn cleanup_test_data(pool: &PgPool) {
    // Delete in reverse order of dependencies
    sqlx::query("DELETE FROM user_roles")
        .execute(pool)
        .await
        .ok();

    sqlx::query("DELETE FROM organization_permission_delegations")
        .execute(pool)
        .await
        .ok();

    sqlx::query("DELETE FROM organization_permission_grants")
        .execute(pool)
        .await
        .ok();

    sqlx::query("DELETE FROM organization_members")
        .execute(pool)
        .await
        .ok();

    sqlx::query("DELETE FROM users WHERE email LIKE '%test%'")
        .execute(pool)
        .await
        .ok();

    sqlx::query("DELETE FROM roles WHERE name LIKE '%test%'")
        .execute(pool)
        .await
        .ok();

    sqlx::query("DELETE FROM organization_units WHERE name LIKE '%test%'")
        .execute(pool)
        .await
        .ok();
}

/// Create a test user
pub async fn create_test_user(
    pool: &PgPool,
    email: &str,
    password: &str,
) -> Result<uuid::Uuid, sqlx::Error> {
    let password_hash = bcrypt::hash(password, 10).unwrap();

    let user_id: uuid::Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO users (username, email, password_hash, first_name, last_name, user_type, status)
        VALUES ($1, $1, $2, 'Test', 'User', 'staff', 'active')
        RETURNING id
        "#,
    )
    .bind(email)
    .bind(password_hash)
    .fetch_one(pool)
    .await?;

    Ok(user_id)
}
