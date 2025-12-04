use dotenv::dotenv;
use sqlx::PgPool;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&database_url).await?;

    println!("Dropping and recreating tables with correct timestamp types...");

    // Drop admin_users table
    sqlx::query("DROP TABLE IF EXISTS admin_users CASCADE")
        .execute(&pool)
        .await?;

    // Create admin_users table
    sqlx::query(
        r#"
        CREATE TABLE admin_users (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            national_id VARCHAR(13) NOT NULL UNIQUE,
            password_hash VARCHAR(255) NOT NULL,
            name VARCHAR(255) NOT NULL,
            role VARCHAR(50) NOT NULL DEFAULT 'admin',
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#
    )
    .execute(&pool)
    .await?;

    // Create index
    sqlx::query("CREATE INDEX idx_admin_users_national_id ON admin_users(national_id)")
        .execute(&pool)
        .await?;

    // Drop schools table
    sqlx::query("DROP TABLE IF EXISTS schools CASCADE")
        .execute(&pool)
        .await?;

    // Create schools table
    sqlx::query(
        r#"
        CREATE TABLE schools (
            id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            name VARCHAR(255) NOT NULL,
            subdomain VARCHAR(100) NOT NULL UNIQUE,
            db_name VARCHAR(100) NOT NULL,
            db_connection_string TEXT,
            status VARCHAR(50) NOT NULL DEFAULT 'active',
            config JSONB DEFAULT '{}',
            created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
            updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#
    )
    .execute(&pool)
    .await?;

    // Create indexes
    sqlx::query("CREATE INDEX idx_schools_subdomain ON schools(subdomain)")
        .execute(&pool)
        .await?;

    sqlx::query("CREATE INDEX idx_schools_status ON schools(status)")
        .execute(&pool)
        .await?;

    println!("✅ Database tables fixed!");

    Ok(())
}
