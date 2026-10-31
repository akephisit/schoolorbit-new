use async_trait::async_trait;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// Assessment-owned initialization required before a Delivery publication commits.
#[async_trait]
pub trait AssessmentPublicationPort: Send + Sync {
    async fn fill_missing_coordinators(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        actor_user_id: Uuid,
        offering_ids: &[Uuid],
    ) -> Result<(), AppError>;
}
