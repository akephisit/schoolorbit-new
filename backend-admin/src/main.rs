mod db;
mod models;
mod services;
mod handlers;

use dotenv::dotenv;
use ohkami::prelude::*;
use std::env;

#[tokio::main]
async fn main() {
    // Load environment variables
    dotenv().ok();

    // Initialize logger
    env::set_var("RUST_LOG", "info");

    println!("🚀 Starting SchoolOrbit Backend Admin Service...");

    // Get database URL from environment
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| {
            println!("⚠️  DATABASE_URL not set, using example URL");
            "postgresql://user:password@host/db".to_string()
        });

    // Initialize database pool
    match db::init_admin_pool(&database_url).await {
        Ok(pool) => {
            println!("✅ Connected to Neon PostgreSQL");

            // Run migrations
            match sqlx::migrate!("./migrations").run(&pool).await {
                Ok(_) => println!("✅ Database migrations completed"),
                Err(e) => {
                    eprintln!("❌ Migration error: {}", e);
                    eprintln!("   Continuing anyway...");
                }
            }
        }
        Err(e) => {
            eprintln!("❌ Database connection failed: {}", e);
            eprintln!("   Server will start but database features will not work");
        }
    }

    println!("✅ Services initialized");

    // Create server with basic routes
    let app = Ohkami::new((
        // Health check
        "/health".GET(handlers::health::health_check),

        // Simple info endpoint
        "/".GET(|| async {
            serde_json::json!({
                "service": "SchoolOrbit Backend Admin",
                "version": "0.1.0",
                "status": "running"
            }).to_string()
        }),
    ));

    println!("🌐 Server starting on http://0.0.0.0:8080");
    println!("\nAvailable endpoints:");
    println!("  GET  /           - API info");
    println!("  GET  /health     - Health check");
    println!("\n📝 Next steps:");
    println!("  1. Set DATABASE_URL in .env file");
    println!("  2. Run migrations: sqlx migrate run");
    println!("  3. Create first admin user");

    app.howl("0.0.0.0:8080").await
}
