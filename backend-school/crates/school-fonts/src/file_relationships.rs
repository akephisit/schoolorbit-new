//! Persistence of temporary upload relationships owned by the school-font domain.

use school_errors::AppError;
use sqlx::PgPool;
use uuid::Uuid;

pub async fn record_upload(
    pool: &PgPool,
    file_id: Uuid,
    uploaded_by: Uuid,
) -> Result<(), AppError> {
    sqlx::query(
        "INSERT INTO school_font_file_uploads (file_id, purpose_code, uploaded_by)
         VALUES ($1, 'school_font', $2)",
    )
    .bind(file_id)
    .bind(uploaded_by)
    .execute(pool)
    .await?;
    Ok(())
}
