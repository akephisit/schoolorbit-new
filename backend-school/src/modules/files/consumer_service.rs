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
        FilePurpose::CertificateTemplateBackground
            | FilePurpose::CertificateTemplateImage
            | FilePurpose::CertificateTemplateFont
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
