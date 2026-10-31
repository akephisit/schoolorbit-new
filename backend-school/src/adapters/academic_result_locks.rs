use async_trait::async_trait;
use school_academic_assessment::ports::ResultLockPort;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) struct ApplicationResultLocks;

pub(crate) static RESULT_LOCKS: ApplicationResultLocks = ApplicationResultLocks;

#[async_trait]
impl ResultLockPort for ApplicationResultLocks {
    async fn current_obligation_group_ids(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        academic_term_id: Uuid,
        academic_year_id: Uuid,
        date: chrono::NaiveDate,
        candidate_group_ids: &[Uuid],
    ) -> Result<Vec<Uuid>, AppError> {
        school_academic_results::services::current_obligation_group_ids_on_connection(
            transaction,
            academic_term_id,
            academic_year_id,
            date,
            candidate_group_ids,
        )
        .await
    }

    async fn course_offering_is_locked(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        learning_offering_id: Uuid,
        academic_term_id: Uuid,
        academic_year_id: Uuid,
    ) -> Result<bool, AppError> {
        school_academic_results::services::course_offering_is_locked(
            transaction,
            learning_offering_id,
            academic_term_id,
            academic_year_id,
        )
        .await
    }

    async fn require_course_offering_unlocked(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        learning_offering_id: Uuid,
        academic_term_id: Uuid,
        academic_year_id: Uuid,
    ) -> Result<(), AppError> {
        school_academic_results::services::require_course_offering_unlocked(
            transaction,
            learning_offering_id,
            academic_term_id,
            academic_year_id,
        )
        .await
    }
}
