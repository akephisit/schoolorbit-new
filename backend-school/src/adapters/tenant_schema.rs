use std::{future::Future, pin::Pin};

use sqlx::PgConnection;

/// Binds runtime pool admission to the migration owner's immutable history check
/// on the connection SQLx is initializing, before that connection enters the pool.
pub fn verify_tenant_schema(
    connection: &mut PgConnection,
) -> Pin<Box<dyn Future<Output = Result<i64, String>> + Send + '_>> {
    Box::pin(school_migrations::verify_tenant_schema(connection))
}
