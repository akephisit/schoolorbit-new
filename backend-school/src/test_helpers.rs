use sqlx::{PgPool, postgres::PgPoolOptions};
use std::env;

/// Create a test database pool
pub async fn create_test_pool() -> PgPool {
    dotenv::dotenv().ok();
    
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
    
    sqlx::query("DELETE FROM department_members")
        .execute(pool)
        .await
        .ok();
    
    sqlx::query("DELETE FROM teaching_assignments")
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
    
    sqlx::query("DELETE FROM departments WHERE name LIKE '%test%'")
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
        INSERT INTO users (email, password_hash, first_name, last_name, user_type, is_active)
        VALUES ($1, $2, 'Test', 'User', 'staff', true)
        RETURNING id
        "#
    )
    .bind(email)
    .bind(password_hash)
    .fetch_one(pool)
    .await?;
    
    Ok(user_id)
}

/// Create a test role
pub async fn create_test_role(
    pool: &PgPool,
    name: &str,
) -> Result<uuid::Uuid, sqlx::Error> {
    let role_id: uuid::Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO roles (name, description)
        VALUES ($1, $2)
        RETURNING id
        "#
    )
    .bind(name)
    .bind("Test role")
    .fetch_one(pool)
    .await?;
    
    Ok(role_id)
}

/// Create a test department
pub async fn create_test_department(
    pool: &PgPool,
    name: &str,
) -> Result<uuid::Uuid, sqlx::Error> {
    let dept_id: uuid::Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO departments (name, description)
        VALUES ($1, $2)
        RETURNING id
        "#
    )
    .bind(name)
    .bind("Test department")
    .fetch_one(pool)
    .await?;
    
    Ok(dept_id)
}

/// Assign role to user
pub async fn assign_role_to_user(
    pool: &PgPool,
    user_id: uuid::Uuid,
    role_id: uuid::Uuid,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO user_roles (user_id, role_id, is_primary, started_at)
        VALUES ($1, $2, true, NOW())
        "#
    )
    .bind(user_id)
    .bind(role_id)
    .execute(pool)
    .await?;
    
    Ok(())
}
