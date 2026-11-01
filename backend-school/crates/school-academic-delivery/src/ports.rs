use async_trait::async_trait;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// The application supplies the owner of unpinned lesson/day consumption. Publication
/// callers retain the term lock and lend their existing connection through this check.
#[async_trait]
pub trait PublicationHistoryPort: Send + Sync {
    async fn has_consumed_history(
        &self,
        connection: &mut sqlx::PgConnection,
        academic_term_id: Uuid,
        effective_from: chrono::NaiveDate,
    ) -> Result<bool, AppError>;
}

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
