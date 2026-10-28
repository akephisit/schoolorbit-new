use async_trait::async_trait;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use school_errors::AppError;

/// Hard bound for raw group scans before current-obligation filtering.
pub const CURRENT_OBLIGATION_GROUP_SCAN_LIMIT: usize = 10_000;

/// Result-owned lock validation needed inside Assessment write transactions.
#[async_trait]
pub trait ResultLockPort: Send + Sync {
    async fn current_obligation_group_ids(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        academic_term_id: Uuid,
        academic_year_id: Uuid,
        date: chrono::NaiveDate,
        candidate_group_ids: &[Uuid],
    ) -> Result<Vec<Uuid>, AppError>;

    async fn require_course_offering_unlocked(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        learning_offering_id: Uuid,
        academic_term_id: Uuid,
        academic_year_id: Uuid,
    ) -> Result<(), AppError>;
}
