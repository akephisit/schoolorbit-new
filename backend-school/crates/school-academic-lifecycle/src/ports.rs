use std::{collections::HashMap, sync::Arc};

#[derive(Clone)]
pub struct PreparationCommandContext {
    pub actor: school_authorization::ActorContext,
    pub tenant_id: Uuid,
    pub session_id: Uuid,
    pub pool_lease: school_tenancy::TenantPool,
    pub authorization: Arc<dyn PreparationAuthorizationPort>,
}
#[async_trait]
pub trait PreparationAuthorizationPort: Send + Sync {
    async fn validate_admission(
        &self,
        actor: &school_authorization::ActorContext,
    ) -> Result<(), AppError>;
    fn committed(&self, outcome: &crate::models::TermPreparationOutcome);
}
#[async_trait]
pub trait PreparationAdmission: Send + Sync {
    async fn apply_timetable(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        context: &TermPreparationContext,
        mappings: &HashMap<(TermPreparationMappingKind, Uuid), Uuid>,
    ) -> Result<TermPreparationModuleOutcome, AppError>;
    async fn commit(
        self: Box<Self>,
        tx: school_tenancy::ObservedTransaction,
        changed: bool,
    ) -> Result<(), AppError>;
}

use async_trait::async_trait;
use chrono::NaiveDate;
use school_academic_core::ports::{
    PendingTermWork, TermPreparationContext, TermPreparationMappingKind,
    TermPreparationModuleOutcome,
};
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub enum RegistrationOutcome {
    Graduated,
    TransferredOut,
}

#[async_trait]
pub trait ExternalLifecyclePort: Send + Sync {
    async fn admit_preparation(
        &self,
        context: &PreparationCommandContext,
        year: Uuid,
        term: Uuid,
        include_timetable: bool,
    ) -> Result<Box<dyn PreparationAdmission>, AppError>;

    async fn require_reopen_allowed(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        academic_year_id: Uuid,
        academic_term_id: Option<Uuid>,
    ) -> Result<(), AppError>;

    async fn apply_registration_outcomes(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        actor_user_id: Uuid,
        outcomes: &[(Uuid, RegistrationOutcome)],
    ) -> Result<(), AppError>;

    async fn require_attendance_delivery_complete(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        academic_term_id: Uuid,
        closed_on: NaiveDate,
    ) -> Result<(), AppError>;

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
