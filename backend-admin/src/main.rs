use backend_admin::{handlers, middleware};
use axum::{
    routing::{get, post},
    Router,
};
use dotenv::dotenv;
use sqlx::postgres::PgPoolOptions;
use std::env;
use tower_cookies::CookieManagerLayer;

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
    handlers::school::init_pool(pool.clone());

    println!("✅ Services initialized");
    println!("🔐 CORS handling delegated to nginx reverse proxy");

    // Build application 
    let app = Router::new()
        // Public routes
        .route("/", get(|| async {
            serde_json::json!({
                "service": "SchoolOrbit Backend Admin",
                "version": "0.1.0",
                "status": "running"
            }).to_string()
        }))
        .route("/health", get(handlers::health::health_check))
        .route("/api/v1/auth/login", post(handlers::auth::login_handler))
        .route("/api/v1/auth/logout", post(handlers::auth::logout_handler))
        .route("/api/v1/auth/me", get(handlers::auth::me_handler))
        // Protected routes (require authentication)
        .nest("/api/v1/schools", Router::new()
            .route("/", post(handlers::school::create_school))
            .route("/", get(handlers::school::list_schools))
            .route("/:id", get(handlers::school::get_school))
            .route("/:id", axum::routing::put(handlers::school::update_school))
            .route("/:id", axum::routing::delete(handlers::school::delete_school))
            // Deployment endpoints
            .route("/:id/deploy", post(handlers::school::deploy_school))
            .route("/deploy/bulk", post(handlers::school::bulk_deploy_schools))
            .route("/:id/deployments", get(handlers::school::get_deployment_history))
            .layer(axum::middleware::from_fn(middleware::auth::require_auth))
        )
        // Global layers
        .layer(CookieManagerLayer::new());

    println!("🌐 Server starting on http://0.0.0.0:8080");
    println!("\nAvailable endpoints:");
    println!("  GET  /              - API info");
    println!("  GET  /health        - Health check");
    println!("  POST /api/v1/auth/login - Login with national ID");
    println!("  POST /api/v1/auth/logout - Logout");
    println!("  GET  /api/v1/auth/me - Get current user");
    println!("\n  School Management:");
    println!("  POST   /api/v1/schools       - Create school");
    println!("  GET    /api/v1/schools       - List schools (paginated)");
    println!("  GET    /api/v1/schools/{{id}}  - Get school by ID");
    println!("  PUT    /api/v1/schools/{{id}}  - Update school");
    println!("  DELETE /api/v1/schools/{{id}}  - Delete school");
    println!("\n📝 Test credentials:");
    println!("  National ID: 1234567890123");
    println!("  Password: test123");
    println!("  EIEEI");
    println!("  EIEEI");
    println!("  EIEEI");

    // Run server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("Failed to bind to port 8080");

    axum::serve(listener, app)
        .await
        .expect("Server failed");
}
