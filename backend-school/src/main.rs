mod handlers;
mod middleware;
mod models;

use axum::{
    middleware as axum_middleware,
    routing::{get, post},
    Router,
    Json,
};
use dotenv::dotenv;
use serde_json::json;
use std::env;

#[tokio::main]
async fn main() {
    dotenv().ok();

    println!("🚀 Starting SchoolOrbit Backend School Service...");

    // Get environment variables
    let port = env::var("PORT").unwrap_or_else(|_| "8081".to_string());
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());

    // Verify internal secret is set
    env::var("INTERNAL_API_SECRET")
        .expect("INTERNAL_API_SECRET must be set for internal API authentication");

    println!("✅ Services initialized");

    // Build application
    let app = Router::new()
        // Public routes
        .route("/", get(root_handler))
        .route("/health", get(health_check))
        // Internal routes (protected by internal auth middleware)
        .route(
            "/internal/provision",
            post(handlers::provision::provision_tenant)
                .layer(axum_middleware::from_fn(
                    middleware::internal_auth::validate_internal_secret,
                )),
        );

    let addr = format!("{}:{}", host, port);
    println!("🌐 Server starting on http://{}", addr);
    println!("\nAvailable endpoints:");
    println!("  GET  /        - API info");
    println!("  GET  /health  - Health check");
    println!("  POST /internal/provision - Provision tenant database (internal only)");

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

