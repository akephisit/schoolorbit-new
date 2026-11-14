use async_trait::async_trait;
use school_errors::AppError;
use sqlx::PgConnection;
use uuid::Uuid;

/// Current subject-group affiliations needed by the timetable workspace.
pub struct StaffSubjectGroupAssignment {
    pub user_id: Uuid,
    pub subject_group_id: Uuid,
    pub name: String,
    pub display_order: Option<i32>,
}

/// The application binds Staff's authoritative directory without coupling Timetable to Staff.
#[async_trait]
pub trait StaffDirectoryPort: Send + Sync {
    /// Current portrait references already authorized by Staff's profile scope for this viewer.
    /// The bounded target set comes from authenticated draft membership, never client identity.
    async fn collaboration_portraits(
        &self,
        connection: &mut PgConnection,
        viewer: &school_authorization::ActorContext,
        user_ids: &[Uuid],
    ) -> Result<Vec<(Uuid, Uuid)>, AppError>;

    /// Read only the already-authorized, bounded users selected by the workspace.
    async fn subject_groups_for_users(
        &self,
        connection: &mut PgConnection,
        user_ids: &[Uuid],
    ) -> Result<Vec<StaffSubjectGroupAssignment>, AppError>;
}

/// The application binds Supervision's exact saved-source detachment without
/// introducing a Timetable -> Supervision dependency cycle.
#[async_trait]
pub trait LessonSourceDetachmentPort: Send + Sync {
    async fn detach_timetable_sources_in_transaction(
        &self,
        transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        block_group_ids: &[Uuid],
    ) -> Result<(), AppError>;
}

#[cfg(any(test, feature = "test-support"))]
pub struct NoopLessonSourceDetachment;
#[cfg(any(test, feature = "test-support"))]
pub static NOOP_LESSON_SOURCE_DETACHMENT: NoopLessonSourceDetachment = NoopLessonSourceDetachment;
#[cfg(any(test, feature = "test-support"))]
#[async_trait]
impl LessonSourceDetachmentPort for NoopLessonSourceDetachment {
    async fn detach_timetable_sources_in_transaction(
        &self,
        _transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        _block_group_ids: &[Uuid],
    ) -> Result<(), AppError> {
        Ok(())
    }
}
