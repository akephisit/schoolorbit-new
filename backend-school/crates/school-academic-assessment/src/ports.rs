use async_trait::async_trait;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use school_errors::AppError;

/// Result-owned lock validation needed inside Assessment write transactions.
#[async_trait]
pub trait ResultLockPort: Send + Sync {
    async fn require_course_offering_unlocked(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        learning_offering_id: Uuid,
        academic_term_id: Uuid,
        academic_year_id: Uuid,
    ) -> Result<(), AppError>;
}
