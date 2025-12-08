use axum::{
    extract::Json,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Router,
};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;

mod neon;
use neon::NeonClient;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSchoolDatabaseRequest {
    pub school_name: String,
    pub subdomain: String,
}

#[derive(Debug, Serialize)]
pub struct CreateSchoolDatabaseResponse {
    pub success: bool,
    pub message: String,
    pub database_name: String,
    pub connection_string: String,
    pub tables_created: Vec<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv::dotenv().ok();

    println!("🏫 Starting Backend-School Database Service...");
    println!("   Managing database lifecycle for schools");

    let app = Router::new()
        .route("/", axum::routing::get(|| async { "Backend-School Database Service v1.0" }))
        .route("/health", axum::routing::get(health_check))
        .route("/api/v1/create-school-database", post(create_school_database));

    let port = std::env::var("PORT").unwrap_or_else(|_| "8081".to_string());
    let addr = format!("0.0.0.0:{}", port);

    println!("✅ Service ready on http://{}", addr);
    println!("   POST /api/v1/create-school-database - Complete database setup");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "service": "backend-school-database",
        "status": "healthy",
        "version": "1.0.0"
    }))
}

async fn create_school_database(
    Json(request): Json<CreateSchoolDatabaseRequest>,
) -> Response {
    println!("� Creating database for: {}", request.school_name);
    println!("   Subdomain: {}", request.subdomain);

    let db_name = format!("schoolorbit_{}", request.subdomain.replace('-', "_"));

    match provision_school_database(&db_name).await {
        Ok((connection_string, tables)) => {
            println!("✅ Database provisioned successfully: {}", db_name);
            (
                StatusCode::OK,
                Json(CreateSchoolDatabaseResponse {
                    success: true,
                    message: format!("Database created and initialized for {}", request.school_name),
                    database_name: db_name,
                    connection_string,
                    tables_created: tables,
                }),
            )
                .into_response()
        }
        Err(e) => {
            eprintln!("❌ Database provisioning failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "success": false,
                    "error": e.to_string()
                })),
            )
                .into_response()
        }
    }
}

async fn provision_school_database(db_name: &str) -> Result<(String, Vec<String>), String> {
    // Initialize Neon client
    let neon_client = NeonClient::new()?;

    // Step 1: Create database in Neon
    println!("  📊 Creating database in Neon...");
    let connection_string = neon_client.create_database(db_name).await?;
    println!("  ✅ Database created");

    // Step 2: Connect and run migrations
    println!("  🔧 Running migrations...");
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&connection_string)
        .await
        .map_err(|e| format!("Connection failed: {}", e))?;

    println!("  🔗 Connected to database");

    // Read and execute migrations
    let migration_sql = include_str!("../migrations/20250101000000_initial_schema.sql");
    
    sqlx::raw_sql(migration_sql)
        .execute(&pool)
        .await
        .map_err(|e| format!("Migration failed: {}", e))?;

    println!("  ✅ Migrations completed");

    let tables = vec![
        "admin_users".to_string(),
        "students".to_string(),
        "teachers".to_string(),
        "classes".to_string(),
        "attendance".to_string(),
        "grades".to_string(),
        "announcements".to_string(),
    ];

    pool.close().await;

    Ok((connection_string, tables))
}
