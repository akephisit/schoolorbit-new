use async_trait::async_trait;
use school_academic_assessment::ports::ResultLockPort;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) struct ApplicationResultLocks;

pub(crate) static RESULT_LOCKS: ApplicationResultLocks = ApplicationResultLocks;

#[async_trait]
impl ResultLockPort for ApplicationResultLocks {
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
