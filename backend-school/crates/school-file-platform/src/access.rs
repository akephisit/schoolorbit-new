#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FilePolicyAction {
    Create,
    Read,
    Delete,
}

use crate::repository::PlatformFile;
use school_authorization::ActorContext;
use school_errors::AppError;
use sqlx::PgPool;
use uuid::Uuid;

/// Domain authorization is supplied by composition; storage never depends on its consumers.
#[async_trait::async_trait]
pub trait FileAuthorizationPort: Send + Sync {
    async fn authorize_create(
        &self,
        pool: &PgPool,
        actor: &ActorContext,
        purpose: crate::platform_types::FilePurpose,
        resource_id: Option<Uuid>,
    ) -> Result<Uuid, AppError>;
    async fn authorize_existing(
        &self,
        pool: &PgPool,
        actor: &ActorContext,
        file: &PlatformFile,
        action: FilePolicyAction,
        resource_id: Option<Uuid>,
    ) -> Result<(), AppError>;
    async fn authorize_portal_application(
        &self,
        pool: &PgPool,
        authenticated_application_id: Uuid,
        requested_application_id: Uuid,
        file_id: Option<Uuid>,
    ) -> Result<(), AppError>;
}
