use std::{future::Future, pin::Pin};

use sqlx::PgConnection;

/// Binds runtime pool admission to the migration owner's immutable history check
/// on the connection SQLx is initializing, before that connection enters the pool.
pub fn verify_tenant_schema(
    connection: &mut PgConnection,
) -> Pin<Box<dyn Future<Output = Result<i64, String>> + Send + '_>> {
    Box::pin(school_migrations::verify_tenant_schema(connection))
}

#[cfg(test)]
mod tests {
    #[tokio::test]
    async fn canonical_adapter_reports_release_version_and_rejects_corrupt_history() {
        let fixture = school_test_db::create_named_test_pool("schema_adapter_canonical").await;
        school_test_db::run_test_migrations(&fixture).await;
        let verifier: school_tenancy::RuntimeSchemaVerifier = super::verify_tenant_schema;
        let mut connection = fixture.acquire().await.unwrap();
        assert_eq!(
            verifier(&mut connection).await.unwrap(),
            school_migrations::latest_migration_version()
        );
        sqlx::query("UPDATE _sqlx_migrations SET checksum = decode('00', 'hex') WHERE version = (SELECT max(version) FROM _sqlx_migrations)")
            .execute(&mut *connection).await.unwrap();
        assert_eq!(
            verifier(&mut connection).await.unwrap_err(),
            "Tenant schema is not ready for this release"
        );
    }
}
