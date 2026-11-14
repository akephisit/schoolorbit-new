use std::{collections::HashMap, sync::Arc};

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

pub(crate) struct ApplicationLifecycleProviders {
    pub collaboration: Arc<school_academic_timetable::collaboration::TimetableCollaborationRuntime>,
}
struct PreparationAuthorization(
    Arc<dyn school_academic_lifecycle::ports::PreparationAuthorizationPort>,
);
#[async_trait]
impl school_academic_timetable::collaboration::MutationAuthorizationPort
    for PreparationAuthorization
{
    async fn validate_admission(
        &self,
        actor: &school_authorization::ActorContext,
    ) -> Result<(), AppError> {
        self.0.validate_admission(actor).await
    }
}
struct ApplicationPreparationAdmission {
    guard: Option<school_academic_timetable::collaboration::TimetablePreparationGuard>,
}
#[async_trait]
impl school_academic_lifecycle::ports::PreparationAdmission for ApplicationPreparationAdmission {
    async fn apply_timetable(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        context: &TermPreparationContext,
        mappings: &HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
    ) -> Result<TermPreparationModuleOutcome, AppError> {
        self.guard
            .as_ref()
            .ok_or_else(|| AppError::Conflict("Timetable preparation was not admitted".into()))?
            .apply_in_transaction(tx, context, mappings)
            .await
    }
    async fn commit(
        self: Box<Self>,
        tx: school_tenancy::ObservedTransaction,
        changed: bool,
    ) -> Result<(), AppError> {
        if let Some(guard) = &self.guard {
            guard.commit(tx, changed).await
        } else {
            tx.commit().await?;
            Ok(())
        }
    }
}

#[async_trait]
impl ExternalLifecyclePort for ApplicationLifecycleProviders {
    async fn admit_preparation(
        &self,
        context: &school_academic_lifecycle::ports::PreparationCommandContext,
        year: Uuid,
        term: Uuid,
        include_timetable: bool,
    ) -> Result<Box<dyn school_academic_lifecycle::ports::PreparationAdmission>, AppError> {
        let guard = if include_timetable {
            let mutation = school_academic_timetable::collaboration::TimetableMutationContext {
                runtime: self.collaboration.clone(),
                actor: context.actor.clone(),
                tenant_id: context.tenant_id,
                session_id: context.session_id,
                identity: None,
                staff_directory: &super::staff_directory::STAFF_DIRECTORY,
                lesson_source_detachment: &super::lesson_sources::LESSON_SOURCE_DETACHMENT,
                authorization: Arc::new(PreparationAuthorization(context.authorization.clone())),
                pool_lease: context.pool_lease.clone(),
            };
            Some(
                mutation
                    .admit_term_preparation(&context.pool_lease, year, term)
                    .await?,
            )
        } else {
            None
        };
        Ok(Box::new(ApplicationPreparationAdmission { guard }))
    }

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
