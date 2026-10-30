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
