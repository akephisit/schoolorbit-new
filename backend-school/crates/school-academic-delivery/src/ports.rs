use async_trait::async_trait;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// Assessment initializes only missing coordinators during a current Delivery save.
#[async_trait]
pub trait AssessmentCurrentPort: Send + Sync {
    async fn fill_missing_coordinators(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        actor_user_id: Uuid,
        offering_ids: &[Uuid],
    ) -> Result<(), AppError>;
}

/// The application composes dependency owners using the caller's transaction.
/// Preview evidence must cover all owned dependent rows, including content changes
/// which leave counts unchanged. Deletion must retain unrelated resources.
#[async_trait]
pub trait DeliveryDeletionPort: Send + Sync {
    async fn preview(
        &self,
        connection: &mut sqlx::PgConnection,
        scope: &crate::models::current::DeliveryDeletionScope,
    ) -> Result<crate::models::current::DeliveryDeletionDependencies, AppError>;
    async fn delete(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        scope: &crate::models::current::DeliveryDeletionScope,
        actor_user_id: Uuid,
    ) -> Result<(), AppError>;
}
