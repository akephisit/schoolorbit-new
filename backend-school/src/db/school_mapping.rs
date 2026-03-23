use super::admin_client::AdminClient;

/// Get database URL for a school subdomain.
/// Calls backend-admin's internal API — backend-school no longer queries admin DB directly.
pub async fn get_school_database_url(
    client: &AdminClient,
    subdomain: &str,
) -> Result<String, String> {
    client.get_db_url(subdomain).await
}
