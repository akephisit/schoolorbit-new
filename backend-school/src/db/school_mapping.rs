use sqlx::PgPool;

/// Get database URL for a school subdomain
/// This queries the backend-admin database for school information
/// Accepts schools with 'active' or 'provisioning' status to support route registration during deployment
pub async fn get_school_database_url(
    admin_pool: &PgPool,
    subdomain: &str,
) -> Result<String, String> {
    let result = sqlx::query_scalar::<_, String>(
        "SELECT db_connection_string FROM schools 
         WHERE subdomain = $1 AND status IN ('active', 'provisioning')"
    )
    .bind(subdomain)
    .fetch_optional(admin_pool)
    .await
    .map_err(|e| format!("Failed to query school mapping: {}", e))?;

    result.ok_or_else(|| format!("School '{}' not found or inactive", subdomain))
}
