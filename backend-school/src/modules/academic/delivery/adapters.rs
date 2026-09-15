use async_trait::async_trait;
use chrono::NaiveDate;
use school_academic_delivery::ports::TimetableMutationPort;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) struct ApplicationTimetableMutations;

pub(crate) static TIMETABLE_MUTATIONS: ApplicationTimetableMutations =
    ApplicationTimetableMutations;

#[async_trait]
impl TimetableMutationPort for ApplicationTimetableMutations {
    async fn retry_group_sync(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        learning_group_id: Uuid,
        actor_id: Uuid,
    ) -> Result<(), AppError> {
        school_academic_timetable::services::timetable_block_sync::retry_sync_for_group_in_tx(
            transaction,
            learning_group_id,
            actor_id,
        )
        .await
    }

    async fn clone_change_set_draft(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        actor_id: Uuid,
        source_version_id: Uuid,
        source_row_version: i64,
        effective_from: NaiveDate,
        change_set_id: Uuid,
    ) -> Result<Uuid, AppError> {
        school_academic_timetable::services::timetable_version_service::clone_draft_in_transaction(
            transaction,
            actor_id,
            source_version_id,
            source_row_version,
            effective_from,
            Some(change_set_id),
        )
        .await
    }
}
