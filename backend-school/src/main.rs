mod handlers;
mod middleware;
mod models;
mod utils;

use axum::{
    middleware as axum_middleware,
    routing::{get, post},
    Router,
    Json,
};
use dotenv::dotenv;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::env;
use tower_cookies::CookieManagerLayer;

#[tokio::main]
async fn main() {
    dotenv().ok();

    println!("🚀 Starting SchoolOrbit Backend School Service...");

    // Get environment variables
    let port = env::var("PORT").unwrap_or_else(|_| "8081".to_string());
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let database_url = env::var("DATABASE_URL")
        .expect("DATABASE_URL must be set");

    // Verify internal secret is set
    env::var("INTERNAL_API_SECRET")
        .expect("INTERNAL_API_SECRET must be set for internal API authentication");

    // Initialize database pool
    println!("📦 Connecting to database...");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .connect(&database_url)
        .await
        .expect("Failed to connect to database");

    println!("✅ Database connected");
    println!("✅ Services initialized");

    // Build application
    let app = Router::new()
        // Public routes
        .route("/", get(root_handler))
        .route("/health", get(health_check))
        
        // Auth routes (public)
        .route("/api/auth/login", post(handlers::auth::login))
        .route("/api/auth/logout", post(handlers::auth::logout))
        
        // Protected auth routes
        .route("/api/auth/me", get(handlers::auth::me)
            .layer(axum_middleware::from_fn(middleware::auth::auth_middleware)))
        
        // Internal routes (protected by internal auth middleware)
        .route(
            "/internal/provision",
            post(handlers::provision::provision_tenant)
                .layer(axum_middleware::from_fn(
                    middleware::internal_auth::validate_internal_secret,
                )),
        )
        // Add cookie middleware
        .layer(CookieManagerLayer::new())
        // Add state
        .with_state(pool);

    let addr = format!("{}:{}", host, port);
    println!("🌐 Server starting on http://{}", addr);
    println!("\nAvailable endpoints:");
    println!("  GET  /                    - API info");
    println!("  GET  /health              - Health check");
    println!("  POST /api/auth/login      - Login");
    println!("  POST /api/auth/logout     - Logout");
    println!("  GET  /api/auth/me         - Get current user (protected)");
    println!("  POST /internal/provision  - Provision tenant database (internal only)");

    // Run server
    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .expect(&format!("Failed to bind to {}", addr));

    axum::serve(listener, app)
        .await
        .expect("Server failed");
}

// Handler functions
async fn root_handler() -> Json<serde_json::Value> {
    Json(json!({
        "service": "SchoolOrbit Backend School",
        "version": "0.1.0",
        "status": "running"
    }))
}

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}
