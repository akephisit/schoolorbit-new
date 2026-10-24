//! Persistence of temporary upload relationships owned by the school-font domain.

use school_errors::AppError;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn record_upload(
    pool: &PgPool,
    file_id: Uuid,
    uploaded_by: Uuid,
) -> Result<(), AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "fonts.record_upload").await?;
    record_upload_on_connection(&mut connection, file_id, uploaded_by).await
}

pub async fn record_upload_on_connection(
    connection: &mut sqlx::PgConnection,
    file_id: Uuid,
    uploaded_by: Uuid,
) -> Result<(), AppError> {
    school_database::DbSession::new(&mut *connection)
        .query(
            "fonts.src.file_relationships.record_upload_on_connection",
            "INSERT INTO school_font_file_uploads (file_id, purpose_code, uploaded_by)
         VALUES ($1, 'school_font', $2)",
        )
        .bind(file_id)
        .bind(uploaded_by)
        .execute()
        .await?;
    Ok(())
}
