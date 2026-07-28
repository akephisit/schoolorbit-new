use std::collections::HashSet;

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

use super::{
    platform_service::{FilePlatform, FilePlatformError},
    repository::SqlFileRepository,
};

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
