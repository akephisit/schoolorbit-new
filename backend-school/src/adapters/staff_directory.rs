use async_trait::async_trait;
use school_academic_timetable::ports::{StaffDirectoryPort, StaffSubjectGroupAssignment};
use school_errors::AppError;
use sqlx::PgPool;
use uuid::Uuid;

pub(crate) struct ApplicationStaffDirectory;

pub(crate) static STAFF_DIRECTORY: ApplicationStaffDirectory = ApplicationStaffDirectory;

#[async_trait]
impl StaffDirectoryPort for ApplicationStaffDirectory {
    async fn subject_groups_for_users(
        &self,
        pool: &PgPool,
        user_ids: &[Uuid],
    ) -> Result<Vec<StaffSubjectGroupAssignment>, AppError> {
        Ok(
            school_staff::services::subject_groups_for_users(pool, user_ids)
                .await?
                .into_iter()
                .map(|assignment| StaffSubjectGroupAssignment {
                    user_id: assignment.user_id,
                    subject_group_id: assignment.subject_group_id,
                    name: assignment.name,
                    display_order: assignment.display_order,
                })
                .collect(),
        )
    }
}
