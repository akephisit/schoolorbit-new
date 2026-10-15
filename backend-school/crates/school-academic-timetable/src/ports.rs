use async_trait::async_trait;
use school_errors::AppError;
use sqlx::PgPool;
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
    /// Read only the already-authorized, bounded users selected by the workspace.
    async fn subject_groups_for_users(
        &self,
        pool: &PgPool,
        user_ids: &[Uuid],
    ) -> Result<Vec<StaffSubjectGroupAssignment>, AppError>;
}
