use crate::models::provision::{ProvisionRequest, ProvisionResponse};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use sqlx::postgres::PgPoolOptions;

/// Handler for provisioning a new school tenant database
/// 
/// This endpoint:
/// 1. Connects to the provided database URL
/// 2. Runs all migrations
/// 3. Creates initial tenant data
/// 4. Returns success/failure
pub async fn provision_tenant(
    Json(payload): Json<ProvisionRequest>,
) -> Response {
    println!("📦 Provisioning tenant for school: {}", payload.school_id);
    println!("   Subdomain: {}", payload.subdomain);

    // Connect to the tenant database
    let pool = match PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(std::time::Duration::from_secs(30))
        .connect(&payload.db_connection_string)
        .await
    {
        Ok(pool) => pool,
        Err(e) => {
            eprintln!("❌ Failed to connect to tenant database: {}", e);
            let error = serde_json::json!({
                "success": false,
                "error": format!("Database connection failed: {}", e)
            });
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response();
        }
    };

    println!("✅ Connected to tenant database");

    // Run migrations
    match sqlx::migrate!("./migrations")
        .run(&pool)
        .await
    {
        Ok(_) => {
            println!("✅ Migrations completed successfully");
        }
        Err(e) => {
            eprintln!("❌ Migration failed: {}", e);
            let error = serde_json::json!({
                "success": false,
                "error": format!("Migration failed: {}", e)
            });
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(error)).into_response();
        }
    }

    // Optional: Create initial tenant data here
    // For example, create a default admin user for the school
    // This can be done later based on requirements

    println!("🎉 Tenant provisioning completed for school: {}", payload.school_id);

    let response = ProvisionResponse {
        success: true,
        message: "Tenant database provisioned successfully".to_string(),
        school_id: payload.school_id,
    };

    (StatusCode::OK, Json(response)).into_response()
}
