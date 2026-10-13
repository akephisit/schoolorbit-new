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

    if let Some(mode) = env::args().nth(1) {
        match mode.as_str() {
            "--provision-database" => {
                use std::io::Read;
                #[derive(serde::Deserialize)]
                struct Input {
                    tenant_id: uuid::Uuid,
                    db_name: String,
                    migration_password: String,
                    runtime_password: String,
                }
                let mut bytes = Vec::new();
                std::io::stdin()
                    .take(16_385)
                    .read_to_end(&mut bytes)
                    .expect("Private input unavailable");
                assert!(
                    bytes.len() <= 16_384,
                    "Private operation input is too large"
                );
                let input: Input = serde_json::from_slice(&bytes)
                    .unwrap_or_else(|_| panic!("Invalid private operation input"));
                let client = backend_admin::clients::postgres_client::PostgresClient::new()
                    .expect("Invalid PostgreSQL provisioning configuration");
                client
                    .create_database(
                        input.tenant_id,
                        &input.db_name,
                        &input.migration_password,
                        &input.runtime_password,
                    )
                    .await
                    .expect("Database provisioning failed; reconcile ownership before retry");
                println!("{{\"provisioned\":true}}");
                return;
            }
            "--migrate-admin" | "--migrate-admin-expand" => {
                let database_url = env::var("ADMIN_MIGRATION_DATABASE_URL")
                    .expect("ADMIN_MIGRATION_DATABASE_URL is required for migration jobs");
                let pool = init_admin_pool(&database_url)
                    .await
                    .expect("Admin migration connection failed");
                backend_admin::db::migrate_admin(&pool, mode == "--migrate-admin-expand")
                    .await
                    .expect("Admin migration/reconciliation gate failed");
                pool.close().await;
                return;
            }
            _ => panic!("Unsupported Admin operation"),
        }
    }

    backend_admin::database_credentials::DatabaseCredentials::from_env()
        .expect("Database credentials key is invalid");
    backend_admin::clients::postgres_client::PostgresClient::new()
        .expect("PostgreSQL provisioning configuration is invalid");

    // Runtime never applies migrations.
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = init_admin_pool(&database_url)
        .await
        .expect("Failed to connect to database");

    info!("connected to admin PostgreSQL database");

    backend_admin::db::verify_admin_schema(&pool)
        .await
        .expect("Admin schema is not ready");

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
