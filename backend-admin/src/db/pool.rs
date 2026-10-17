use sqlx::ConnectOptions;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;

pub async fn init_admin_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let options = database_url
        .parse::<sqlx::postgres::PgConnectOptions>()?
        .disable_statement_logging();
    let maximum = std::env::var("ADMIN_POOL_MAX_CONNECTIONS").map_or(Ok(5u32), |raw| {
        raw.parse::<u32>().map_err(|_| {
            sqlx::Error::Protocol("ADMIN_POOL_MAX_CONNECTIONS must be an integer".into())
        })
    })?;
    if !(1..=1000).contains(&maximum) {
        return Err(sqlx::Error::Protocol(
            "ADMIN_POOL_MAX_CONNECTIONS outside supported range".into(),
        ));
    }
    PgPoolOptions::new()
        .min_connections(0)
        .acquire_time_level(log::LevelFilter::Info)
        .acquire_slow_threshold(Duration::from_millis(250))
        .max_connections(maximum)
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Duration::from_secs(300)) // Close idle connections after 5 minutes
        .test_before_acquire(true)
        .connect_with(options)
        .await
}

pub async fn verify_admin_schema(pool: &PgPool) -> Result<(), String> {
    let applied: Vec<(i64, bool, Vec<u8>)> =
        sqlx::query_as("SELECT version, success, checksum FROM _sqlx_migrations ORDER BY version")
            .fetch_all(pool)
            .await
            .map_err(|_| "Admin migration history unavailable".to_string())?;
    let migrator = sqlx::migrate!("./migrations");
    if applied.len() != migrator.iter().count()
        || !applied
            .iter()
            .zip(migrator.iter())
            .all(|((version, success, checksum), expected)| {
                *success
                    && *version == expected.version
                    && checksum.as_slice() == expected.checksum.as_ref()
            })
    {
        return Err("Admin schema does not match this release".into());
    }
    Ok(())
}

pub async fn migrate_admin(pool: &PgPool) -> Result<(), String> {
    let migrator = sqlx::migrate!("./migrations");
    migrator
        .run(pool)
        .await
        .map_err(|_| "Admin migration gate failed".to_string())?;
    // Only production's owner/runtime pair needs grants; fixtures use their owned role.
    let owner: String = sqlx::query_scalar("SELECT current_user::text")
        .fetch_one(pool)
        .await
        .map_err(|_| "Admin migration identity unavailable".to_string())?;
    if owner == "schoolorbit_admin_owner" {
        sqlx::raw_sql("REVOKE CREATE ON SCHEMA public FROM PUBLIC;
            GRANT USAGE ON SCHEMA public TO schoolorbit_admin_runtime;
            GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO schoolorbit_admin_runtime;
            GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO schoolorbit_admin_runtime;
            REVOKE INSERT, UPDATE, DELETE ON _sqlx_migrations FROM schoolorbit_admin_runtime;
            ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT SELECT, INSERT, UPDATE, DELETE ON TABLES TO schoolorbit_admin_runtime;
            ALTER DEFAULT PRIVILEGES IN SCHEMA public GRANT USAGE, SELECT ON SEQUENCES TO schoolorbit_admin_runtime")
            .execute(pool).await.map_err(|_| "Admin runtime grants failed".to_string())?;
    }
    Ok(())
}
