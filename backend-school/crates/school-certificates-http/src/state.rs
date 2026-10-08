use async_trait::async_trait;
use school_auth::runtime::AuthRuntime;
use school_certificates::verification_limiter::CertificateVerificationLimiter;
use school_errors::AppError;
use school_file_platform::platform_service::FilePlatform;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

#[async_trait]
pub trait CertificateFileDeletionPort: Send + Sync {
    async fn request_deletions(
        &self,
        platform: &FilePlatform,
        pool: &PgPool,
        file_ids: Vec<Uuid>,
    ) -> Result<(), AppError>;
}

#[derive(Clone)]
pub struct CertificateHttpState {
    pub auth_runtime: AuthRuntime,
    pub file_platform: Arc<FilePlatform>,
    pub certificate_verification_limiter: Arc<CertificateVerificationLimiter>,
    pub file_deletions: &'static dyn CertificateFileDeletionPort,
}
