use async_trait::async_trait;
use chrono::NaiveDate;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

/// Atomic timetable consequences required by Delivery mutations.
#[async_trait]
pub trait TimetableMutationPort: Send + Sync {
    async fn include_offering_target(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        timetable_version_id: Uuid,
        learning_offering_id: Uuid,
    ) -> Result<(), AppError>;

    async fn retry_group_sync(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        learning_group_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), AppError>;

    async fn clone_change_set_draft(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        actor_id: Uuid,
        source_version_id: Uuid,
        source_row_version: i64,
        effective_from: NaiveDate,
        change_set_id: Uuid,
    ) -> Result<Uuid, AppError>;
}
