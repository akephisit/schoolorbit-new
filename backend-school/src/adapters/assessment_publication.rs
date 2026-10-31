use async_trait::async_trait;
use school_academic_delivery::ports::AssessmentPublicationPort;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) struct ApplicationAssessmentPublication;
pub(crate) static ASSESSMENT_PUBLICATION: ApplicationAssessmentPublication =
    ApplicationAssessmentPublication;

#[async_trait]
impl AssessmentPublicationPort for ApplicationAssessmentPublication {
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
