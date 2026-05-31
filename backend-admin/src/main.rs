use backend_admin::{build_app, AppState};
use dotenv::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::env;

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

    println!("✅ Services initialized");
    println!("🔐 CORS handling delegated to nginx reverse proxy");

    // Build application
    let app = build_app(AppState::new(pool));

    println!("🌐 Server starting on http://0.0.0.0:8080");
    println!("\n✅ Available endpoints:");
    println!("  GET  /                          - API info");
    println!("  GET  /health                    - Health check");
    println!("  POST /api/v1/auth/login         - Login with national ID");
    println!("  POST /api/v1/auth/logout        - Logout");
    println!("  GET  /api/v1/auth/me            - Get current user");
    println!("  Internal APIs (Protected by Internal Secret):");
    println!("  GET  /internal/schools          - List all schools (internal use)\n");
    println!("  School Management (Protected):");
    println!("  /api/v1/schools/*               - CRUD Schools");
    println!("  /api/v1/schools/stream          - Create School with SSE Logs");
    println!("  /api/v1/schools/{{id}}/deploy     - Trigger Deployment");

    // Run server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("Failed to bind to port 8080");

    axum::serve(listener, app).await.expect("Server failed");
}
