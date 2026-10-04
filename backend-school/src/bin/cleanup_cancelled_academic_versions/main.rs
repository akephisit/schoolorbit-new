use school_tenancy::{get_school_database_info, AdminClient, AdminClientConfig};
use sqlx::postgres::PgPoolOptions;
use std::{env, error::Error};
use uuid::Uuid;
mod cleanup;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error + Send + Sync>> {
    // Resolve one explicit reviewed tenant through the canonical registry; never fan out.
    let subdomain = env::var("ACADEMIC_CLEANUP_SUBDOMAIN")?;
    let year = Uuid::parse_str(&env::var("ACADEMIC_CLEANUP_ACADEMIC_YEAR_ID")?)?;
    let term = Uuid::parse_str(&env::var("ACADEMIC_CLEANUP_ACADEMIC_TERM_ID")?)?;
    let actor_id = Uuid::parse_str(&env::var("ACADEMIC_CLEANUP_ACTOR_ID")?)?;
    let client = AdminClient::new(
        env::var("BACKEND_ADMIN_URL")?,
        env::var("INTERNAL_API_SECRET_BACKEND_SCHOOL")
            .or_else(|_| env::var("INTERNAL_API_SECRET"))?,
        AdminClientConfig::from_env()?,
    );
    let school = get_school_database_info(&client, &subdomain)
        .await
        .map_err(|_| "Cannot resolve the authorized school through backend-admin")?;
    let expected_tenant = Uuid::parse_str(&env::var("ACADEMIC_CLEANUP_TENANT_ID")?)?;
    if expected_tenant != school.tenant_id {
        return Err("Resolved tenant differs from the reviewed tenant identity".into());
    }
    let pool = PgPoolOptions::new()
        .max_connections(2)
        .connect(&school.database_url)
        .await
        .map_err(|_| "Cannot connect to authorized school")?;
    let actor = school_authorization::load_actor_context(
        actor_id,
        &subdomain,
        &pool,
        &school_authorization::PermissionCache::new(),
    )
    .await?;
    let hash = env::var("ACADEMIC_CLEANUP_PREVIEW_HASH").ok();
    let report = cleanup::cleanup(&pool, &actor, year, term, hash.as_deref()).await?;
    println!("{}", serde_json::to_string(&report)?);
    pool.close().await;
    Ok(())
}
