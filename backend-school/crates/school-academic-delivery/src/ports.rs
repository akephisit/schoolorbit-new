use async_trait::async_trait;
use school_errors::AppError;
use sqlx::{Postgres, Transaction};
use std::sync::Arc;
use uuid::Uuid;

/// Captured request identity and a retained tenant pool for an accepted command.
#[derive(Clone)]
pub struct DeliveryDeletionCommandContext {
    pub actor: school_authorization::ActorContext,
    pub tenant_id: Uuid,
    pub session_id: Uuid,
    pub pool_lease: school_tenancy::TenantPool,
    pub authorization: Arc<dyn DeliveryDeletionAuthorizationPort>,
}

#[async_trait]
pub trait DeliveryDeletionAuthorizationPort: Send + Sync {
    async fn validate_admission(
        &self,
        actor: &school_authorization::ActorContext,
    ) -> Result<(), AppError>;
    /// Infallible publication after COMMIT, inside the accepted task.
    fn committed(&self, result: &crate::models::current::DeletedDeliveryResource);
}

#[derive(Debug, Clone, Copy)]
pub struct DeliveryDeletionTarget {
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub learning_offering_id: Uuid,
    pub learning_group_id: Option<Uuid>,
}

/// Owns dependency gates until the actual outer transaction commits.
#[async_trait]
pub trait DeliveryDeletionAdmission: Send + Sync {
    async fn lock_dependencies_before_term(
        &mut self,
        transaction: &mut Transaction<'_, Postgres>,
    ) -> Result<(), AppError>;
    /// Locks Attendance term and Behavior after the canonical term, before digest acceptance.
    async fn lock_dependencies_after_term(
        &mut self,
        transaction: &mut Transaction<'_, Postgres>,
        scope: &crate::models::current::DeliveryDeletionScope,
    ) -> Result<(), AppError>;
    async fn delete_dependencies(
        &mut self,
        transaction: &mut Transaction<'_, Postgres>,
        scope: &crate::models::current::DeliveryDeletionScope,
    ) -> Result<(), AppError>;
    async fn commit(
        self: Box<Self>,
        transaction: school_tenancy::ObservedTransaction,
        changed: bool,
    ) -> Result<(), AppError>;
}

/// Assessment initializes only missing coordinators during a current Delivery save.
#[async_trait]
pub trait AssessmentCurrentPort: Send + Sync {
    async fn fill_missing_coordinators(
        &self,
        transaction: &mut Transaction<'_, Postgres>,
        actor_user_id: Uuid,
        offering_ids: &[Uuid],
    ) -> Result<(), AppError>;
}

/// The application composes dependency owners using the caller's transaction.
/// Preview evidence must cover all owned dependent rows, including content changes
/// which leave counts unchanged. Deletion must retain unrelated resources.
#[async_trait]
pub trait DeliveryDeletionPort: Send + Sync {
    async fn preview(
        &self,
        connection: &mut sqlx::PgConnection,
        scope: &crate::models::current::DeliveryDeletionScope,
    ) -> Result<crate::models::current::DeliveryDeletionDependencies, AppError>;
    async fn acquire(
        &self,
        context: &DeliveryDeletionCommandContext,
        target: DeliveryDeletionTarget,
    ) -> Result<Box<dyn DeliveryDeletionAdmission>, AppError>;
}
