use std::collections::HashMap;

use async_trait::async_trait;
use chrono::NaiveDate;
use school_academic_core::ports::{
    PendingTermWork, TermPreparationContext, TermPreparationMappingKind,
    TermPreparationModuleOutcome,
};
use school_academic_lifecycle::ports::{ExternalLifecyclePort, RegistrationOutcome};
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(crate) struct ApplicationLifecycleProviders;

pub(crate) static LIFECYCLE_PROVIDERS: ApplicationLifecycleProviders =
    ApplicationLifecycleProviders;

#[async_trait]
impl ExternalLifecyclePort for ApplicationLifecycleProviders {
    async fn require_reopen_allowed(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        academic_year_id: Uuid,
        academic_term_id: Option<Uuid>,
    ) -> Result<(), AppError> {
        school_attendance::services::retention::require_reopen_allowed(
            transaction,
            academic_year_id,
            academic_term_id,
        )
        .await
    }

    async fn apply_registration_outcomes(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        actor_user_id: Uuid,
        outcomes: &[(Uuid, RegistrationOutcome)],
    ) -> Result<(), AppError> {
        let outcomes: Vec<_> = outcomes
            .iter()
            .map(|(id, outcome)| {
                (
                    *id,
                    match outcome {
                        RegistrationOutcome::Graduated => {
                            school_students::models::StudentRegistrationStatus::Graduated
                        }
                        RegistrationOutcome::TransferredOut => {
                            school_students::models::StudentRegistrationStatus::TransferredOut
                        }
                    },
                )
            })
            .collect();
        school_students::registration_outcomes::apply_in_transaction(
            transaction,
            actor_user_id,
            &outcomes,
        )
        .await
    }

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
            .await?;
        school_behavior::services::cleanup::require_drained(transaction).await
    }

    async fn pending_exam_work(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        academic_year_id: Uuid,
        academic_term_id: Uuid,
    ) -> Result<Vec<PendingTermWork>, AppError> {
        school_academic_assessment::exam_schedule::services::pending_term_work(
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
        school_academic_assessment::exam_schedule::services::apply_term_preparation(
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
