use async_trait::async_trait;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;
pub(crate) struct ApplicationLessonSourceDetachment;
pub(crate) static LESSON_SOURCE_DETACHMENT: ApplicationLessonSourceDetachment =
    ApplicationLessonSourceDetachment;
#[async_trait]
impl school_academic_timetable::ports::LessonSourceDetachmentPort
    for ApplicationLessonSourceDetachment
{
    async fn detach_timetable_sources_in_transaction(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        ids: &[Uuid],
    ) -> Result<(), AppError> {
        school_supervision::services::current_source::detach_timetable_sources_in_transaction(
            tx, ids,
        )
        .await?;
        Ok(())
    }
}
