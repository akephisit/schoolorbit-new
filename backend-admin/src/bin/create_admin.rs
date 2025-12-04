use dotenv::dotenv;
use sqlx::PgPool;
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let database_url = env::var("DATABASE_URL")?;
    let pool = PgPool::connect(&database_url).await?;

    let national_id = "1234567890123";
    let password = "test123";
    let name = "Test Admin";

    // Hash password using shared library
    let password_hash = shared::auth::hash_password(password)?;

    // Insert admin user
    sqlx::query!(
        r#"
        INSERT INTO admin_users (national_id, password_hash, name, role)
        VALUES ($1, $2, $3, 'super_admin')
        ON CONFLICT (national_id) DO UPDATE SET password_hash = $2
        "#,
        national_id,
        password_hash,
        name
    )
    .execute(&pool)
    .await?;

    println!("✅ Test admin user created:");
    println!("   National ID: {}", national_id);
    println!("   Password: {}", password);
    println!("   Name: {}", name);

    Ok(())
}
