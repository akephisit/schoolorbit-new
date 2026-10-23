mod startup;

use backend_admin::{build_app, db::init_admin_pool, AppState};
use dotenvy::dotenv;
use startup::{required_environment, StartupError};
use std::env;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<(), StartupError> {
    dotenv().ok();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .try_init()
        .map_err(|_| StartupError::logging())?;

    let outcome = run().await;
    if let Err(error) = &outcome {
        error.log();
    }
    outcome
}

async fn run() -> Result<(), StartupError> {
    info!("starting SchoolOrbit Backend Admin Service");

    if let Some(mode) = env::args_os().nth(1) {
        match mode.to_str() {
            Some("--migrate-admin") => {
                let database_url = required_environment("ADMIN_MIGRATION_DATABASE_URL")?;
                let pool = init_admin_pool(&database_url).await.map_err(|error| {
                    StartupError::database(
                        "migration_connection",
                        "ADMIN_MIGRATION_DATABASE_URL",
                        error,
                    )
                })?;
                let outcome = backend_admin::db::migrate_admin(&pool)
                    .await
                    .map_err(|error| StartupError::migration(&error));
                pool.close().await;
                return outcome;
            }
            Some(_) => return Err(StartupError::operation("unsupported_operation")),
            None => return Err(StartupError::operation("invalid_unicode")),
        }
    }

    {
        let credentials_key = required_environment("DATABASE_CREDENTIALS_KEY")?;
        backend_admin::database_credentials::DatabaseCredentials::from_key(&credentials_key)
            .map_err(|_| StartupError::credentials())?;
        let provisioning_url = required_environment("PG_PROVISIONING_DATABASE_URL")?;
        let tenant_url = required_environment("PG_TENANT_DATABASE_URL")?;
        backend_admin::clients::postgres_client::PostgresClient::from_config(
            &provisioning_url,
            &tenant_url,
        )
        .map_err(|error| StartupError::provisioning(&error))?;
    }

    // Runtime never applies migrations.
    let database_url = required_environment("DATABASE_URL")?;

    let pool = init_admin_pool(&database_url)
        .await
        .map_err(|error| StartupError::database("database_connection", "DATABASE_URL", error))?;

    info!("connected to admin PostgreSQL database");

    if let Err(error) = backend_admin::db::verify_admin_schema(&pool).await {
        pool.close().await;
        return Err(StartupError::schema(&error));
    }

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
        .map_err(|error| StartupError::io("listener_bind", error))?;

    axum::serve(listener, app)
        .await
        .map_err(|error| StartupError::io("http_server", error))
}
