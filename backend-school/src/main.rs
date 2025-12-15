mod db;
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
use db::pool_manager::PoolManager;
use dotenv::dotenv;
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::{env, sync::Arc};
use tower_cookies::CookieManagerLayer;

/// Shared application state
#[derive(Clone)]
pub struct AppState {
    pub admin_pool: sqlx::PgPool,  // Backend-admin database (for school mapping)
    pub pool_manager: Arc<PoolManager>,
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    println!("🚀 Starting SchoolOrbit Backend School Service...");

    // Get environment variables
    let port = env::var("PORT").unwrap_or_else(|_| "8081".to_string());
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    
    // Connect to backend-admin database for school mapping
    let admin_database_url = env::var("ADMIN_DATABASE_URL")
        .expect("ADMIN_DATABASE_URL must be set (backend-admin database for school mapping)");

    // Verify internal secret is set
    env::var("INTERNAL_API_SECRET")
        .expect("INTERNAL_API_SECRET must be set for internal API authentication");

    println!("📦 Connecting to admin database for school mapping...");
    let admin_pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&admin_database_url)
        .await
        .expect("Failed to connect to admin database");

    println!("✅ Admin database connected");

    // Create pool manager for tenant databases
    let pool_manager = Arc::new(PoolManager::new());
    
    // Start cleanup task
    let pool_manager_cleanup = Arc::clone(&pool_manager);
    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
            pool_manager_cleanup.cleanup_expired().await;
        }
    });

    println!("✅ Pool manager initialized");
    println!("ℹ️  Multi-tenant architecture ready");
    println!("ℹ️  Each school has its own database connection pool (cached)");

    // Create shared state
    let state = AppState {
        admin_pool,
        pool_manager,
    };

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
        .with_state(state);

    let addr = format!("{}:{}", host, port);
    println!("🌐 Server starting on http://{}", addr);
    println!("\nAvailable endpoints:");
    println!("  GET  /                    - API info");
    println!("  GET  /health              - Health check");
    println!("  POST /api/auth/login      - Login (requires X-School-Subdomain header)");
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
        "version": "0.2.0",
        "status": "running",
        "architecture": "multi-tenant with dynamic connection pools"
    }))
}

async fn health_check() -> Json<serde_json::Value> {
    Json(json!({
        "status": "healthy",
        "timestamp": chrono::Utc::now().to_rfc3339()
    }))
}
