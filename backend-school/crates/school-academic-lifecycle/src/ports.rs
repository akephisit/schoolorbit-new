use std::collections::HashMap;

use async_trait::async_trait;
use chrono::NaiveDate;
use school_academic_core::ports::{
    PendingTermWork, TermPreparationContext, TermPreparationMappingKind,
    TermPreparationModuleOutcome,
};
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[async_trait]
pub trait ExternalLifecyclePort: Send + Sync {
    async fn pending_exam_work(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        academic_year_id: Uuid,
        academic_term_id: Uuid,
    ) -> Result<Vec<PendingTermWork>, AppError>;

    async fn pending_supervision_work(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        academic_year_id: Uuid,
        academic_term_id: Uuid,
    ) -> Result<Vec<PendingTermWork>, AppError>;

    async fn apply_exam_preparation(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        actor_user_id: Uuid,
        context: &TermPreparationContext,
        entity_mappings: &HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
        date_mappings: &HashMap<NaiveDate, NaiveDate>,
    ) -> Result<TermPreparationModuleOutcome, AppError>;

    async fn apply_supervision_preparation(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        actor_user_id: Uuid,
        context: &TermPreparationContext,
        entity_mappings: &HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
        date_mappings: &HashMap<NaiveDate, NaiveDate>,
    ) -> Result<TermPreparationModuleOutcome, AppError>;
}
