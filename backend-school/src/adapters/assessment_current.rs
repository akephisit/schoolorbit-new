use async_trait::async_trait;
use school_academic_delivery::ports::AssessmentCurrentPort;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) struct ApplicationAssessmentCurrent;
pub(crate) static ASSESSMENT_CURRENT: ApplicationAssessmentCurrent = ApplicationAssessmentCurrent;

#[async_trait]
impl AssessmentCurrentPort for ApplicationAssessmentCurrent {
    async fn fill_missing_coordinators(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        actor_user_id: Uuid,
        offering_ids: &[Uuid],
    ) -> Result<(), AppError> {
        school_academic_assessment::assessment::services::fill_missing_coordinators(
            &super::academic_result_locks::RESULT_LOCKS,
            transaction,
            actor_user_id,
            offering_ids,
        )
        .await
    }
}
