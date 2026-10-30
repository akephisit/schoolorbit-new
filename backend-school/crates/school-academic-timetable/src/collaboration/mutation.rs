use std::future::Future;
use std::sync::Arc;

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

    pub async fn execute<T, F, Fut>(&self, pool: &PgPool, version_id: Uuid, scope: MutationScope, operation: F) -> Result<T, AppError>
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
            let mutation_id = context.identity.as_ref().map(|identity| identity.mutation_id).unwrap_or_else(Uuid::new_v4);
            let connection_id = context.identity.as_ref().map(|identity| identity.connection_id);
            let outcome = tokio::time::timeout(std::time::Duration::from_secs(60), operation(pool, permit.clone())).await;
            let (value, blocks) = match outcome {
                Ok(Ok(outcome)) => outcome,
                Ok(Err(error)) => { if permit.possibly_committed() { room.invalidate(); } return Err(error); }
                Err(_) => { if permit.possibly_committed() { room.invalidate(); } return Err(AppError::ServiceUnavailable("Timetable operation exceeded its admission deadline".into())); }
            };
            if room.publish_blocks(blocks, connection_id, mutation_id).is_err() {
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
