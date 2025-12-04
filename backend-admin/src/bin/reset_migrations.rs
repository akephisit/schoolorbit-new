use dotenv::dotenv;
use sqlx::PgPool;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&database_url).await?;

    println!("Clearing migrations table...");

    // Drop migrations table
    sqlx::query("DROP TABLE IF EXISTS _sqlx_migrations")
        .execute(&pool)
        .await?;

    println!("✅ Migrations table cleared!");
    println!("   Now restart the server to run migrations again.");

    Ok(())
}
