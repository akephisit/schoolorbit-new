use async_trait::async_trait;
use chrono::NaiveDate;
use school_academic_delivery::ports::PublicationHistoryPort;
use school_errors::AppError;
use sqlx::PgConnection;
use uuid::Uuid;

pub(crate) struct ApplicationPublicationHistory;
pub(crate) static PUBLICATION_HISTORY: ApplicationPublicationHistory =
    ApplicationPublicationHistory;

#[async_trait]
impl PublicationHistoryPort for ApplicationPublicationHistory {
    async fn has_consumed_history(
        &self,
        connection: &mut PgConnection,
        term: Uuid,
        date: NaiveDate,
    ) -> Result<bool, AppError> {
        school_attendance::services::publication::has_consumed_lesson_history_on_connection(
            connection, term, date,
        )
        .await
    }
}
