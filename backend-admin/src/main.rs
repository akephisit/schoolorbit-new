use backend_admin::{build_app, db::init_admin_pool, AppState};
use dotenv::dotenv;
use std::env;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    info!("starting SchoolOrbit Backend Admin Service");

    // Database setup
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = init_admin_pool(&database_url)
        .await
        .expect("Failed to connect to database");

    info!("connected to admin PostgreSQL database");

    // Run migrations
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .expect("Failed to run migrations");

    info!("database migrations completed");

    info!("services initialized");
    info!("CORS handling delegated to nginx reverse proxy");

    // Build application
    let app = build_app(AppState::new(pool));

    info!(
        address = "http://0.0.0.0:8080",
        "server starting with admin and school management endpoints"
    );

    // Run server
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080")
        .await
        .expect("Failed to bind to port 8080");

    axum::serve(listener, app).await.expect("Server failed");
}
