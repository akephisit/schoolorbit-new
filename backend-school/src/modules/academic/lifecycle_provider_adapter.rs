use std::collections::HashMap;

use async_trait::async_trait;
use chrono::NaiveDate;
use school_academic_core::ports::{
    PendingTermWork, TermPreparationContext, TermPreparationMappingKind,
    TermPreparationModuleOutcome,
};
use school_academic_lifecycle::ports::ExternalLifecyclePort;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) struct ApplicationLifecycleProviders;

pub(crate) static LIFECYCLE_PROVIDERS: ApplicationLifecycleProviders =
    ApplicationLifecycleProviders;

#[async_trait]
impl ExternalLifecyclePort for ApplicationLifecycleProviders {
    async fn require_attendance_delivery_complete(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        academic_term_id: Uuid,
        closed_on: NaiveDate,
    ) -> Result<(), AppError> {
        school_attendance::services::finalization::require_finalized_through(
            transaction,
            academic_term_id,
            closed_on,
        )
        .await?;
        school_attendance::services::score_changes::require_drained(transaction, academic_term_id)
            .await
    }

    async fn pending_exam_work(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        academic_year_id: Uuid,
        academic_term_id: Uuid,
    ) -> Result<Vec<PendingTermWork>, AppError> {
        crate::modules::academic::services::exam_schedule_service::pending_term_work(
            transaction,
            academic_year_id,
            academic_term_id,
        )
        .await
    }

    async fn pending_supervision_work(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        academic_year_id: Uuid,
        academic_term_id: Uuid,
    ) -> Result<Vec<PendingTermWork>, AppError> {
        school_supervision::services::pending_term_work(
            transaction,
            academic_year_id,
            academic_term_id,
        )
        .await
    }

    async fn apply_exam_preparation(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        actor_user_id: Uuid,
        context: &TermPreparationContext,
        entity_mappings: &HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
        date_mappings: &HashMap<NaiveDate, NaiveDate>,
    ) -> Result<TermPreparationModuleOutcome, AppError> {
        crate::modules::academic::services::exam_schedule_service::apply_term_preparation(
            transaction,
            actor_user_id,
            context,
            entity_mappings,
            date_mappings,
        )
        .await
    }

    async fn apply_supervision_preparation(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        actor_user_id: Uuid,
        context: &TermPreparationContext,
        entity_mappings: &HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
        date_mappings: &HashMap<NaiveDate, NaiveDate>,
    ) -> Result<TermPreparationModuleOutcome, AppError> {
        school_supervision::services::apply_term_preparation(
            transaction,
            actor_user_id,
            context,
            entity_mappings,
            date_mappings,
        )
        .await
    }
}
