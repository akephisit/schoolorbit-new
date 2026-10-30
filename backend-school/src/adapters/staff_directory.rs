use async_trait::async_trait;
use school_academic_timetable::ports::{StaffDirectoryPort, StaffSubjectGroupAssignment};
use school_errors::AppError;
use sqlx::PgConnection;
use uuid::Uuid;

pub(crate) struct ApplicationStaffDirectory;

pub(crate) static STAFF_DIRECTORY: ApplicationStaffDirectory = ApplicationStaffDirectory;

#[async_trait]
impl StaffDirectoryPort for ApplicationStaffDirectory {
    async fn collaboration_portraits(
        &self,
        connection: &mut PgConnection,
        viewer: &school_authorization::ActorContext,
        user_ids: &[Uuid],
    ) -> Result<Vec<(Uuid, Uuid)>, AppError> {
        school_staff::services::collaboration_portraits_on_connection(connection, viewer, user_ids).await
    }

    async fn subject_groups_for_users(
        &self,
        connection: &mut PgConnection,
        user_ids: &[Uuid],
    ) -> Result<Vec<StaffSubjectGroupAssignment>, AppError> {
        Ok(
            school_staff::services::subject_groups_for_users_on_connection(connection, user_ids)
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
