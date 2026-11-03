//! Narrow owner projection for domain retention manifests; physical deletion stays in Files.
#[cfg(test)]
#[path = "retention_tests.rs"]
mod tests;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::PgConnection;
use uuid::Uuid;

#[derive(Debug, Default, sqlx::FromRow)]
pub struct RetentionDeletionProgress {
    pub pending: i64,
    pub failed: i64,
}

pub async fn require_retention_allowed(
    connection: &mut PgConnection,
    files: &[Uuid],
) -> Result<(), AppError> {
    let classes: Vec<String> = DbSession::new(connection)
        .scalar(
            "files.retention.require_allowed",
            "SELECT retention_class FROM files WHERE id=ANY($1) ORDER BY id FOR UPDATE",
        )
        .bind(files)
        .fetch_all()
        .await?;
    if classes.iter().any(|class| class == "legal_hold") {
        return Err(AppError::Conflict(
            "หลักฐานอยู่ระหว่างระงับการลบ จึงยังเก็บเฉพาะสรุปไม่ได้".into(),
        ));
    }
    Ok(())
}

/// A missing manifest file is unresolved, never evidence of physical deletion.
pub async fn deletion_progress(
    connection: &mut PgConnection,
    files: &[Uuid],
) -> Result<RetentionDeletionProgress, AppError> {
    Ok(DbSession::new(connection).query_as("files.retention.deletion_progress", r#"
        SELECT count(*) FILTER(WHERE f.id IS NULL OR f.lifecycle_status<>'deleted'
            OR EXISTS(SELECT 1 FROM file_versions v WHERE v.file_id=f.id AND v.storage_status<>'deleted')
            OR EXISTS(SELECT 1 FROM file_derivatives d WHERE d.file_id=f.id AND d.storage_status<>'deleted'))::bigint AS pending,
            count(*) FILTER(WHERE EXISTS(SELECT 1 FROM file_operations o WHERE o.file_id=f.id
                AND o.operation_type='delete_object' AND o.status='failed'
                AND (EXISTS(SELECT 1 FROM file_versions v WHERE v.id=o.file_version_id AND v.storage_status<>'deleted')
                  OR EXISTS(SELECT 1 FROM file_derivatives d WHERE d.id=o.file_derivative_id AND d.storage_status<>'deleted'))
                AND NOT EXISTS(SELECT 1 FROM file_operations replacement WHERE replacement.file_id=o.file_id
                  AND replacement.operation_type='delete_object' AND replacement.id<>o.id AND replacement.created_at>=o.created_at
                  AND replacement.file_version_id IS NOT DISTINCT FROM o.file_version_id
                  AND replacement.file_derivative_id IS NOT DISTINCT FROM o.file_derivative_id
                  AND replacement.status IN ('pending','leased','retryable_failure','succeeded'))))::bigint AS failed
        FROM (SELECT DISTINCT unnest($1::uuid[]) AS id) manifest
        LEFT JOIN files f ON f.id=manifest.id
    "#).bind(files).fetch_one().await?)
}
