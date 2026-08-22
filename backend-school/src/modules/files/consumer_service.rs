use std::collections::HashSet;

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

use super::{
    platform_service::{FilePlatform, FilePlatformError},
    platform_types::FilePurpose,
    repository::SqlFileRepository,
};

pub async fn record_certificate_template_upload(
    pool: &PgPool,
    file_id: Uuid,
    template_id: Uuid,
    purpose: FilePurpose,
    uploaded_by: Uuid,
) -> Result<(), AppError> {
    if !matches!(
        purpose,
        FilePurpose::CertificateTemplateBackground | FilePurpose::CertificateTemplateImage
    ) {
        return Err(AppError::BadRequest(
            "purpose ไม่ใช่ไฟล์ของแม่แบบเกียรติบัตร".to_string(),
        ));
    }
    let mut transaction = pool.begin().await?;
    let campaign_status = sqlx::query_scalar::<_, String>(
        "SELECT campaign.status
         FROM certificate_templates AS template
         JOIN certificate_campaigns AS campaign ON campaign.id = template.campaign_id
         WHERE template.id = $1
         FOR UPDATE OF campaign",
    )
    .bind(template_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบแม่แบบเกียรติบัตร".to_string()))?;
    if campaign_status == "purging" {
        return Err(AppError::Conflict(
            "certificate_campaign_purging".to_string(),
        ));
    }

    sqlx::query(
        "INSERT INTO certificate_template_file_uploads
            (file_id, template_id, purpose_code, uploaded_by)
         VALUES ($1, $2, $3, $4)",
    )
    .bind(file_id)
    .bind(template_id)
    .bind(purpose.code())
    .bind(uploaded_by)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

pub async fn record_school_font_upload(
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

pub async fn record_certificate_school_font_upload(
    pool: &PgPool,
    file_id: Uuid,
    template_id: Uuid,
    uploaded_by: Uuid,
) -> Result<(), AppError> {
    let mut transaction = pool.begin().await?;
    let campaign_status = sqlx::query_scalar::<_, String>(
        "SELECT campaign.status
         FROM certificate_templates AS template
         JOIN certificate_campaigns AS campaign ON campaign.id = template.campaign_id
         WHERE template.id = $1
         FOR UPDATE OF campaign",
    )
    .bind(template_id)
    .fetch_optional(&mut *transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบแม่แบบเกียรติบัตร".to_string()))?;
    if campaign_status == "purging" {
        return Err(AppError::Conflict(
            "certificate_campaign_purging".to_string(),
        ));
    }

    sqlx::query(
        "INSERT INTO certificate_school_font_file_uploads
            (file_id, purpose_code, template_id, uploaded_by)
         VALUES ($1, 'school_font', $2, $3)",
    )
    .bind(file_id)
    .bind(template_id)
    .bind(uploaded_by)
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

pub fn map_platform_error(error: FilePlatformError) -> AppError {
    match error {
        FilePlatformError::InspectionRejected => {
            AppError::BadRequest("ชนิดหรือโครงสร้างไฟล์ไม่ถูกต้อง".to_string())
        }
        FilePlatformError::MalwareDetected => {
            AppError::BadRequest("ไฟล์ไม่ผ่านการตรวจสอบความปลอดภัย".to_string())
        }
        FilePlatformError::NotFound => AppError::NotFound("ไม่พบไฟล์".to_string()),
        FilePlatformError::NotReady => AppError::Conflict("ไฟล์ยังไม่พร้อมใช้งาน".to_string()),
        FilePlatformError::VisibilityMismatch => {
            AppError::Forbidden("ไม่อนุญาตให้ส่งไฟล์ด้วยช่องทางนี้".to_string())
        }
        FilePlatformError::ScannerUnavailable
        | FilePlatformError::StorageUnavailable
        | FilePlatformError::RequiredDerivativeUnavailable => {
            AppError::ServiceUnavailable(error.log_safe_code().to_string())
        }
        FilePlatformError::MetadataUnavailable => {
            AppError::InternalServerError(error.log_safe_code().to_string())
        }
    }
}

/// Requests lifecycle deletion after a domain relationship transaction commits.
/// Provider failures remain durable retry work; repository failures are surfaced.
pub async fn request_deletions(
    platform: &FilePlatform,
    pool: &PgPool,
    file_ids: impl IntoIterator<Item = Uuid>,
) -> Result<(), AppError> {
    let repository = SqlFileRepository::new(pool.clone());
    let mut unique_ids = HashSet::new();
    let mut first_error = None;
    for file_id in file_ids {
        if unique_ids.insert(file_id) {
            if let Err(error) = platform
                .request_delete(&repository, file_id)
                .await
                .map_err(map_platform_error)
            {
                first_error.get_or_insert(error);
            }
        }
    }
    match first_error {
        Some(error) => Err(error),
        None => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_helpers::{create_named_test_pool, create_test_user, run_test_migrations};

    async fn insert_file(pool: &PgPool, actor_id: Uuid) -> Uuid {
        sqlx::query_scalar(
            "INSERT INTO files (
                display_filename, purpose_code, visibility, lifecycle_status,
                retention_class, inspection_metadata, created_by
             ) VALUES (
                'school-font.ttf', 'school_font', 'private', 'ready',
                'temporary', '{\"kind\":\"font\"}'::jsonb, $1
             )
             RETURNING id",
        )
        .bind(actor_id)
        .fetch_one(pool)
        .await
        .expect("school-font file fixture should insert")
    }

    async fn insert_template(pool: &PgPool, actor_id: Uuid) -> Uuid {
        let academic_year_id: Uuid = sqlx::query_scalar(
            "INSERT INTO academic_years (year, name, start_date, end_date)
             VALUES (2998, 'School font upload test', '2998-01-01', '2998-12-31')
             RETURNING id",
        )
        .fetch_one(pool)
        .await
        .expect("academic-year fixture should insert");
        let campaign_id: Uuid = sqlx::query_scalar(
            "INSERT INTO certificate_campaigns (
                academic_year_id, name, event_date, status, created_by
             ) VALUES ($1, 'School font upload test', '2998-06-01', 'active', $2)
             RETURNING id",
        )
        .bind(academic_year_id)
        .bind(actor_id)
        .fetch_one(pool)
        .await
        .expect("campaign fixture should insert");
        sqlx::query_scalar(
            "INSERT INTO certificate_templates (campaign_id, name, normalized_name)
             VALUES ($1, 'School font upload test', 'school-font-upload-test')
             RETURNING id",
        )
        .bind(campaign_id)
        .fetch_one(pool)
        .await
        .expect("template fixture should insert")
    }

    #[tokio::test]
    async fn central_school_font_upload_records_only_the_typed_staging_relation() {
        let pool = create_named_test_pool("central_school_font_upload_relation").await;
        run_test_migrations(&pool).await;
        let actor_id = create_test_user(
            &pool,
            "central-school-font-upload@example.test",
            "test-password",
        )
        .await
        .expect("actor fixture should insert");
        let file_id = insert_file(&pool, actor_id).await;

        record_school_font_upload(&pool, file_id, actor_id)
            .await
            .expect("central staging relation should insert");

        let row: (String, Uuid) = sqlx::query_as(
            "SELECT purpose_code, uploaded_by
             FROM school_font_file_uploads
             WHERE file_id = $1",
        )
        .bind(file_id)
        .fetch_one(&pool)
        .await
        .expect("central staging relation should be queryable");
        assert_eq!(row, ("school_font".to_string(), actor_id));
        let certificate_row_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM certificate_school_font_file_uploads WHERE file_id = $1",
        )
        .bind(file_id)
        .fetch_one(&pool)
        .await
        .expect("certificate staging relation should be queryable");
        assert_eq!(certificate_row_count, 0);
    }

    #[tokio::test]
    async fn certificate_school_font_upload_records_the_exact_template_relation() {
        let pool = create_named_test_pool("certificate_school_font_upload_relation").await;
        run_test_migrations(&pool).await;
        let actor_id = create_test_user(
            &pool,
            "certificate-school-font-upload@example.test",
            "test-password",
        )
        .await
        .expect("actor fixture should insert");
        let template_id = insert_template(&pool, actor_id).await;
        let file_id = insert_file(&pool, actor_id).await;

        record_certificate_school_font_upload(&pool, file_id, template_id, actor_id)
            .await
            .expect("certificate staging relation should insert");

        let row: (String, Uuid, Uuid) = sqlx::query_as(
            "SELECT purpose_code, template_id, uploaded_by
             FROM certificate_school_font_file_uploads
             WHERE file_id = $1",
        )
        .bind(file_id)
        .fetch_one(&pool)
        .await
        .expect("certificate staging relation should be queryable");
        assert_eq!(row, ("school_font".to_string(), template_id, actor_id));
        let central_row_count: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM school_font_file_uploads WHERE file_id = $1")
                .bind(file_id)
                .fetch_one(&pool)
                .await
                .expect("central staging relation should be queryable");
        assert_eq!(central_row_count, 0);
    }
}
