use std::future::Future;
use std::sync::Arc;
use std::sync::atomic::{AtomicU8, Ordering};

use async_trait::async_trait;
use school_authorization::ActorContext;
use school_errors::AppError;
use school_tenancy::TenantPool;
use sqlx::PgPool;
use uuid::Uuid;

use super::{CommitPermit, MutationScope, TimetableCollaborationRuntime};
use crate::models::collaboration::DraftMutationIdentity;
use crate::models::timetable_block::TimetableBlock;
use crate::ports::StaffDirectoryPort;

#[async_trait]
pub trait MutationAuthorizationPort: Send + Sync {
    /// The application revalidates the central session and its accepted authorization scope.
    async fn validate_admission(&self, accepted_actor: &ActorContext) -> Result<(), AppError>;
}

/// A creation operation cannot authorize any mutation of an already-existing draft.
pub struct CreationPermit { actor_id: Uuid, phase: AtomicU8 }

impl CreationPermit {
    pub fn actor_id(&self) -> Uuid { self.actor_id }
    pub async fn commit(&self, transaction: school_tenancy::ObservedTransaction) -> Result<(), AppError> {
        self.phase.store(1, Ordering::Release); transaction.commit().await?; self.phase.store(2, Ordering::Release); Ok(())
    }

    /// The lifecycle provider creates a fresh destination in its already-atomic multi-module
    /// transaction. Recheck the canonical term boundary and absence of an active destination.
    pub async fn term_preparation(
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>, actor_id: Uuid,
        context: &school_academic_core::ports::TermPreparationContext,
    ) -> Result<Self, AppError> {
        school_academic_core::services::lifecycle_guard::require_term_write_exclusive(tx, context.target_year_id, context.target_term_id).await?;
        let occupied: bool = school_database::DbSession::new(&mut **tx).scalar(
            "academic_timetable.collaboration.term_preparation_creation_permit",
            "SELECT EXISTS(SELECT 1 FROM academic_timetable_versions WHERE academic_term_id=$1 AND status<>'cancelled')",
        ).bind(context.target_term_id).fetch_one().await?;
        if occupied { return Err(AppError::Conflict("Target term already has a timetable; creation cannot replace it".into())); }
        Ok(Self { actor_id, phase: AtomicU8::new(0) })
    }
}

pub enum VersionMutationPermit { Existing(Arc<CommitPermit>), Creation(Arc<CreationPermit>) }
impl VersionMutationPermit {
    pub fn actor_id(&self) -> Uuid { match self { Self::Existing(permit) => permit.actor_id, Self::Creation(permit) => permit.actor_id() } }
    pub fn require_existing(&self, id: Uuid) -> Result<(), AppError> { match self {
        Self::Existing(permit) => permit.require_version(id),
        Self::Creation(_) => Err(AppError::Conflict("A draft appeared concurrently; reload before resuming it".into())),
    } }
    pub async fn commit(&self, transaction: school_tenancy::ObservedTransaction) -> Result<(), AppError> { match self {
        Self::Existing(permit) => permit.commit(transaction).await,
        Self::Creation(permit) => permit.commit(transaction).await,
    } }
}

#[derive(Clone)]
pub struct TimetableMutationContext {
    pub runtime: Arc<TimetableCollaborationRuntime>,
    pub actor: ActorContext,
    pub tenant_id: Uuid,
    pub session_id: Uuid,
    pub identity: Option<DraftMutationIdentity>,
    pub staff_directory: &'static dyn StaffDirectoryPort,
    pub authorization: Arc<dyn MutationAuthorizationPort>,
    /// Retained by the accepted task across every checkout, commit and publication.
    pub pool_lease: TenantPool,
}

impl TimetableMutationContext {
    #[cfg(any(test, feature = "test-support"))]
    pub fn for_tests(pool: &PgPool, actor_id: Uuid) -> Self {
        Self { runtime: Arc::new(TimetableCollaborationRuntime::new()), actor: ActorContext { user_id: actor_id, permissions: vec!["*".into()] }, tenant_id: Uuid::new_v4(), session_id: Uuid::new_v4(), identity: None,
            staff_directory: &TEST_DIRECTORY, authorization: Arc::new(TestAuthorization), pool_lease: pool.clone().into() }
    }

    pub async fn execute_creation<T, F, Fut>(&self, pool: &PgPool, operation: F) -> Result<T, AppError>
    where T: Send + 'static, F: FnOnce(PgPool, Arc<CreationPermit>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, AppError>> + Send + 'static {
        if self.identity.is_some() { return Err(AppError::Conflict("A source lease cannot authorize creation of another draft".into())); }
        let context = self.clone(); let pool = pool.clone(); let admission = context.runtime.admit_write()?;
        tokio::spawn(async move {
            let _admission = admission; let _tenant_pool_lease = context.pool_lease;
            context.authorization.validate_admission(&context.actor).await?;
            let permit = Arc::new(CreationPermit { actor_id: context.actor.user_id, phase: AtomicU8::new(0) });
            let result = tokio::time::timeout(std::time::Duration::from_secs(60), operation(pool, permit.clone())).await;
            match result {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(error)) => { if permit.phase.load(Ordering::Acquire) != 0 { context.runtime.invalidate_term(context.tenant_id, None); } Err(error) }
                Err(_) => { if permit.phase.load(Ordering::Acquire) != 0 { context.runtime.invalidate_term(context.tenant_id, None); } Err(AppError::ServiceUnavailable("Timetable creation exceeded its admission deadline".into())) }
            }
        }).await.map_err(|_| AppError::InternalServerError("Accepted timetable creation unavailable".into()))?
    }

    pub async fn execute<T, F, Fut>(&self, pool: &PgPool, version_id: Uuid, scope: MutationScope, operation: F) -> Result<T, AppError>
    where T: Send + 'static, F: FnOnce(PgPool, Arc<CommitPermit>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(T, Vec<TimetableBlock>), AppError>> + Send + 'static {
        self.execute_inner(pool, version_id, scope, false, operation).await
    }

    pub async fn execute_lifecycle<T, F, Fut>(&self, pool: &PgPool, version_id: Uuid, operation: F) -> Result<T, AppError>
    where T: Send + 'static, F: FnOnce(PgPool, Arc<CommitPermit>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, AppError>> + Send + 'static {
        self.execute_inner(pool, version_id, MutationScope::WholeDraft, true, move |pool, permit| async move { Ok((operation(pool, permit).await?, Vec::new())) }).await
    }

    async fn execute_inner<T, F, Fut>(&self, pool: &PgPool, version_id: Uuid, scope: MutationScope, invalidate: bool, operation: F) -> Result<T, AppError>
    where T: Send + 'static, F: FnOnce(PgPool, Arc<CommitPermit>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(T, Vec<TimetableBlock>), AppError>> + Send + 'static {
        let context = self.clone();
        let pool = pool.clone();
        let admission = context.runtime.admit_write()?;
        // Once accepted, caller cancellation only drops the JoinHandle. This bounded task retains
        // admission, the tenant lease, per-draft gate and pinned permit through commit/publication.
        tokio::spawn(async move {
            let _admission = admission;
            let _tenant_pool_lease = context.pool_lease;
            let room = context.runtime.load_room(context.tenant_id, &pool, version_id, context.staff_directory).await?;
            let _gate = room.mutation_gate().await;
            context.authorization.validate_admission(&context.actor).await?;
            let permit = Arc::new(room.begin_commit(context.actor.user_id, context.session_id, context.identity.as_ref(), scope)?);
            if invalidate { permit.disable_content_revision(); }
            let mutation_id = context.identity.as_ref().map(|identity| identity.mutation_id).unwrap_or_else(Uuid::new_v4);
            let connection_id = context.identity.as_ref().map(|identity| identity.connection_id);
            let outcome = tokio::time::timeout(std::time::Duration::from_secs(60), operation(pool, permit.clone())).await;
            let (value, blocks) = match outcome {
                Ok(Ok(outcome)) => outcome,
                Ok(Err(error)) => { if permit.possibly_committed() { room.invalidate(); } return Err(error); }
                Err(_) => { if permit.possibly_committed() { room.invalidate(); } return Err(AppError::ServiceUnavailable("Timetable operation exceeded its admission deadline".into())); }
            };
            if invalidate { room.invalidate(); }
            else if !blocks.is_empty() && room.publish_blocks(blocks, permit.version_snapshot()?, connection_id, mutation_id).is_err() {
                // A committed business result stays successful; clients receive reconciliation.
                room.invalidate();
            }
            drop(permit);
            Ok(value)
        }).await.map_err(|_| AppError::InternalServerError("Accepted timetable operation unavailable".into()))?
    }
}

#[cfg(any(test, feature = "test-support"))]
struct TestAuthorization;
#[cfg(any(test, feature = "test-support"))]
#[async_trait]
impl MutationAuthorizationPort for TestAuthorization { async fn validate_admission(&self, _accepted: &ActorContext) -> Result<(), AppError> { Ok(()) } }

#[cfg(any(test, feature = "test-support"))]
struct TestDirectory;
#[cfg(any(test, feature = "test-support"))]
static TEST_DIRECTORY: TestDirectory = TestDirectory;
#[cfg(any(test, feature = "test-support"))]
#[async_trait]
impl StaffDirectoryPort for TestDirectory {
    async fn collaboration_portraits(&self, _connection: &mut sqlx::PgConnection, _viewer: &ActorContext, _ids: &[Uuid]) -> Result<Vec<(Uuid, Uuid)>, AppError> { Ok(Vec::new()) }
    async fn subject_groups_for_users(&self, _connection: &mut sqlx::PgConnection, _ids: &[Uuid]) -> Result<Vec<crate::ports::StaffSubjectGroupAssignment>, AppError> { Ok(Vec::new()) }
}
