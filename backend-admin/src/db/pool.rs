use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;

pub async fn init_admin_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(5)
        .acquire_timeout(Duration::from_secs(30))  // Increased for Neon serverless cold starts
        .idle_timeout(Duration::from_secs(300))    // Close idle connections after 5 minutes
        .connect(database_url)
        .await
}
