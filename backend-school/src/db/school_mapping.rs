use super::admin_client::{AdminClient, SchoolDatabaseInfo};

/// Get database metadata for a school subdomain.
/// Calls backend-admin's internal API — backend-school no longer queries admin DB directly.
pub async fn get_school_database_info(
    client: &AdminClient,
    subdomain: &str,
) -> Result<SchoolDatabaseInfo, String> {
    client.get_school_database_info(subdomain).await
}
