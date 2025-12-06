use axum::{
    routing::{get, post},
    Router,
};
use dotenv::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::env;
use tower_cookies::CookieManagerLayer;

mod db;
mod handlers;
mod middleware;
mod models;
mod services;

#[tokio::main]
async fn main() {
    dotenv().ok();

    println!("🚀 Starting SchoolOrbit Backend Admin Service...");

    // Database setup
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .idle_timeout(Some(std::time::Duration::from_secs(600)))
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    println!("✅ Connected to Neon PostgreSQL");

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    println!("✅ Database migrations completed");

    // Initialize handlers with database pool
    handlers::auth::init_pool(pool.clone());

    println!("✅ Services initialized");
    println!("🔐 CORS handling delegated to nginx reverse proxy");

    // Build application 
    let app = Router::new()
        // API info
        .route("/", get(|| async {
            serde_json::json!({
                "service": "SchoolOrbit Backend Admin",
                "version": "0.1.0",
                "status": "running"
            }).to_string()
        }))
        // Health check
        .route("/health", get(handlers::health::health_check))
        // Auth endpoints
        .route("/api/v1/auth/login", post(handlers::auth::login_handler))
        .route("/api/v1/auth/logout", post(handlers::auth::logout_handler))
        .route("/api/v1/auth/me", get(handlers::auth::me_handler))
        // Layers (order matters: last added = first executed)
        .layer(CookieManagerLayer::new());

    println!("🌐 Server starting on http://0.0.0.0:8080");
    println!("\nAvailable endpoints:");
    println!("  GET  /              - API info");
    println!("  GET  /health        - Health check");
    println!("  POST /api/v1/auth/login - Login with national ID");
    println!("  POST /api/v1/auth/logout - Logout");
    println!("  GET  /api/v1/auth/me - Get current user");
    println!("\n📝 Test credentials:");
    println!("  National ID: 1234567890123");
    println!("  Password: test123");

    // Run server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("Failed to bind to port 8080");

    axum::serve(listener, app)
        .await
        .expect("Server failed");
}
