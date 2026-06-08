use sqlx::postgres::PgPoolOptions;
use std::{env, error::Error, time::Duration};

#[path = "../permissions/registry.rs"]
pub mod permission_registry;

pub mod permissions {
    pub use crate::permission_registry as registry;
}

#[path = "../utils/permission_sync.rs"]
pub mod permission_sync;

pub mod utils {
    pub use crate::permission_sync;
}

#[path = "../db/migration.rs"]
pub mod migration;

type MigrationResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::main]
async fn main() -> MigrationResult<()> {
    dotenvy::dotenv().ok();

    let database_url = env::var("MIGRATION_SCHEMA_DATABASE_URL")
        .or_else(|_| env::var("DATABASE_URL"))
        .map_err(|_| "Set MIGRATION_SCHEMA_DATABASE_URL before running schema migration")?;
    let schema = env::var("MIGRATION_SCHEMA_NAME")
        .map_err(|_| "Set MIGRATION_SCHEMA_NAME before running schema migration")?;

    validate_schema_name(&schema)?;

    let search_path_sql = format!(r#"SET search_path TO "{schema}", public"#);
    let pool = PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(30))
        .after_connect(move |connection, _metadata| {
            let search_path_sql = search_path_sql.clone();
            Box::pin(async move {
                sqlx::query(&search_path_sql).execute(connection).await?;
                Ok(())
            })
        })
        .connect(&database_url)
        .await?;

    migration::run_tenant_migrations(&pool).await?;
    pool.close().await;

    Ok(())
}

fn validate_schema_name(schema: &str) -> MigrationResult<()> {
    if schema.is_empty() {
        return Err("MIGRATION_SCHEMA_NAME must not be empty".into());
    }

    if schema == "public" {
        return Err("Refusing to run schema migration against public".into());
    }

    if !schema
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    {
        return Err(
            "MIGRATION_SCHEMA_NAME must contain only ASCII letters, numbers, and underscores"
                .into(),
        );
    }

    Ok(())
}
