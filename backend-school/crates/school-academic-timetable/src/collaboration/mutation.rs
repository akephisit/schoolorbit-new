use std::future::Future;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use school_authorization::ActorContext;
use school_errors::AppError;
use school_tenancy::TenantPool;
use sqlx::PgPool;
use uuid::Uuid;

use super::{CommitPermit, MutationScope, TimetableCollaborationRuntime, TimetableRoom};
use crate::models::collaboration::TimetableMutationIdentity;
use crate::models::timetable_block::TimetableBlock;
use crate::ports::StaffDirectoryPort;

struct OperationOutcome<T> {
    value: T,
    blocks: Vec<TimetableBlock>,
    lifecycle_changed: bool,
}

/// Cache publication is after the business COMMIT. Its failure can only request
/// reconciliation; it must never turn an accepted result into a rejected write.
pub(super) fn publish_committed_blocks(
    room: &TimetableRoom,
    blocks: Vec<TimetableBlock>,
    version: Result<Option<crate::models::timetable::Timetable>, AppError>,
    connection_id: Option<Uuid>,
    mutation_id: Uuid,
) {
    if version
        .and_then(|version| room.publish_blocks(blocks, version, connection_id, mutation_id))
        .is_err()
    {
        room.invalidate();
    }
}

#[async_trait]
pub trait MutationAuthorizationPort: Send + Sync {
    /// The application revalidates the central session and its accepted authorization scope.
    async fn validate_admission(&self, accepted_actor: &ActorContext) -> Result<(), AppError>;
}

/// A creation operation cannot authorize any mutation of an already-existing current timetable.
pub struct CreationPermit {
    actor_id: Uuid,
    phase: AtomicU8,
}

impl CreationPermit {
    pub fn actor_id(&self) -> Uuid {
        self.actor_id
    }
    pub async fn commit(
        &self,
        transaction: school_tenancy::ObservedTransaction,
    ) -> Result<(), AppError> {
        self.phase.store(1, Ordering::Release);
        transaction.commit().await?;
        self.phase.store(2, Ordering::Release);
        Ok(())
    }
}

pub enum TimetableMutationPermit {
    Existing(Arc<CommitPermit>),
    Creation(Arc<CreationPermit>),
}
impl TimetableMutationPermit {
    pub fn actor_id(&self) -> Uuid {
        match self {
            Self::Existing(permit) => permit.actor_id,
            Self::Creation(permit) => permit.actor_id(),
        }
    }
    pub fn require_existing(&self, id: Uuid) -> Result<(), AppError> {
        match self {
            Self::Existing(permit) => permit.require_timetable(id),
            Self::Creation(_) => Err(AppError::Conflict(
                "A draft appeared concurrently; reload before resuming it".into(),
            )),
        }
    }
    pub async fn commit(
        &self,
        transaction: school_tenancy::ObservedTransaction,
    ) -> Result<(), AppError> {
        match self {
            Self::Existing(permit) => permit.commit(transaction).await,
            Self::Creation(permit) => permit.commit(transaction).await,
        }
    }
}

#[derive(Clone)]
pub struct TimetableMutationContext {
    pub runtime: Arc<TimetableCollaborationRuntime>,
    pub actor: ActorContext,
    pub tenant_id: Uuid,
    pub session_id: Uuid,
    pub identity: Option<TimetableMutationIdentity>,
    pub staff_directory: &'static dyn StaffDirectoryPort,
    pub lesson_source_detachment: &'static dyn crate::ports::LessonSourceDetachmentPort,
    pub authorization: Arc<dyn MutationAuthorizationPort>,
    /// Retained by the accepted task across every checkout, commit and publication.
    pub pool_lease: TenantPool,
}

impl TimetableMutationContext {
    #[cfg(any(test, feature = "test-support"))]
    pub fn for_tests(pool: &PgPool, actor_id: Uuid) -> Self {
        Self {
            runtime: Arc::new(TimetableCollaborationRuntime::new()),
            actor: ActorContext {
                user_id: actor_id,
                permissions: vec!["*".into()],
            },
            tenant_id: Uuid::new_v4(),
            session_id: Uuid::new_v4(),
            identity: None,
            staff_directory: &TEST_DIRECTORY,
            lesson_source_detachment: &crate::ports::NOOP_LESSON_SOURCE_DETACHMENT,
            authorization: Arc::new(TestAuthorization),
            pool_lease: pool.clone().into(),
        }
    }

    pub async fn execute_creation<T, F, Fut>(
        &self,
        pool: &PgPool,
        academic_term_id: Uuid,
        operation: F,
    ) -> Result<T, AppError>
    where
        T: Send + 'static,
        F: FnOnce(PgPool, Arc<CreationPermit>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, AppError>> + Send + 'static,
    {
        if self.identity.is_some() {
            return Err(AppError::Conflict(
                "A source lease cannot authorize creation of another current timetable".into(),
            ));
        }
        let context = self.clone();
        let pool = pool.clone();
        let admission = context.runtime.admit_write()?;
        tokio::spawn(async move {
            let _admission = admission;
            let _tenant_pool_lease = context.pool_lease;
            let _identity_gate = context
                .runtime
                .identity_gate(context.tenant_id, academic_term_id)
                .await?;
            context
                .authorization
                .validate_admission(&context.actor)
                .await?;
            let permit = Arc::new(CreationPermit {
                actor_id: context.actor.user_id,
                phase: AtomicU8::new(0),
            });
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(60),
                operation(pool, permit.clone()),
            )
            .await;
            match result {
                Ok(Ok(value)) => Ok(value),
                Ok(Err(error)) => {
                    if permit.phase.load(Ordering::Acquire) != 0 {
                        context.runtime.invalidate_term(context.tenant_id, None);
                    }
                    Err(error)
                }
                Err(_) => {
                    if permit.phase.load(Ordering::Acquire) != 0 {
                        context.runtime.invalidate_term(context.tenant_id, None);
                    }
                    Err(AppError::ServiceUnavailable(
                        "Timetable creation exceeded its admission deadline".into(),
                    ))
                }
            }
        })
        .await
        .map_err(|_| {
            AppError::InternalServerError("Accepted timetable creation unavailable".into())
        })?
    }

    pub async fn execute<T, F, Fut>(
        &self,
        pool: &PgPool,
        version_id: Uuid,
        scope: MutationScope,
        operation: F,
    ) -> Result<T, AppError>
    where
        T: Send + 'static,
        F: FnOnce(PgPool, Arc<CommitPermit>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<(T, Vec<TimetableBlock>), AppError>> + Send + 'static,
    {
        self.execute_inner(
            pool,
            version_id,
            scope,
            false,
            move |pool, permit| async move {
                let (value, blocks) = operation(pool, permit).await?;
                Ok(OperationOutcome {
                    value,
                    blocks,
                    lifecycle_changed: false,
                })
            },
        )
        .await
    }

    pub async fn execute_lifecycle<T, F, Fut>(
        &self,
        pool: &PgPool,
        version_id: Uuid,
        operation: F,
    ) -> Result<T, AppError>
    where
        T: Send + 'static,
        F: FnOnce(PgPool, Arc<CommitPermit>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, AppError>> + Send + 'static,
    {
        self.execute_inner(
            pool,
            version_id,
            MutationScope::WholeTimetable,
            true,
            move |pool, permit| async move {
                let value = operation(pool, permit).await?;
                Ok(OperationOutcome {
                    value,
                    blocks: Vec::new(),
                    lifecycle_changed: true,
                })
            },
        )
        .await
    }

    async fn execute_inner<T, F, Fut>(
        &self,
        pool: &PgPool,
        version_id: Uuid,
        scope: MutationScope,
        invalidate: bool,
        operation: F,
    ) -> Result<T, AppError>
    where
        T: Send + 'static,
        F: FnOnce(PgPool, Arc<CommitPermit>) -> Fut + Send + 'static,
        Fut: Future<Output = Result<OperationOutcome<T>, AppError>> + Send + 'static,
    {
        let context = self.clone();
        let pool = pool.clone();
        let admission = context.runtime.admit_write()?;
        // Once accepted, caller cancellation only drops the JoinHandle. This bounded task retains
        // admission, the tenant lease, per-timetable gate and pinned permit through commit/publication.
        tokio::spawn(async move {
            let _admission = admission;
            let _tenant_pool_lease = context.pool_lease;
            let room = context
                .runtime
                .load_room(
                    context.tenant_id,
                    &pool,
                    version_id,
                    context.staff_directory,
                )
                .await?;
            let _gate = room.mutation_gate().await;
            context
                .authorization
                .validate_admission(&context.actor)
                .await?;
            let permit = Arc::new(room.begin_commit(
                context.actor.user_id,
                context.session_id,
                context.identity.as_ref(),
                context.lesson_source_detachment,
                scope,
            )?);
            if invalidate {
                permit.disable_content_revision();
            }
            let mutation_id = context
                .identity
                .as_ref()
                .map(|identity| identity.mutation_id)
                .unwrap_or_else(Uuid::new_v4);
            let connection_id = context
                .identity
                .as_ref()
                .map(|identity| identity.connection_id);
            let outcome = tokio::time::timeout(
                std::time::Duration::from_secs(60),
                operation(pool, permit.clone()),
            )
            .await;
            let OperationOutcome {
                value,
                blocks,
                lifecycle_changed,
            } = match outcome {
                Ok(Ok(outcome)) => outcome,
                Ok(Err(error)) => {
                    if permit.possibly_committed() {
                        room.invalidate();
                    }
                    return Err(error);
                }
                Err(_) => {
                    if permit.possibly_committed() {
                        room.invalidate();
                    }
                    return Err(AppError::ServiceUnavailable(
                        "Timetable operation exceeded its admission deadline".into(),
                    ));
                }
            };
            if invalidate && lifecycle_changed {
                room.invalidate();
            } else if !blocks.is_empty() {
                publish_committed_blocks(
                    &room,
                    blocks,
                    permit.timetable_snapshot(),
                    connection_id,
                    mutation_id,
                );
            }
            permit.settle();
            drop(permit);
            Ok(value)
        })
        .await
        .map_err(|_| {
            AppError::InternalServerError("Accepted timetable operation unavailable".into())
        })?
    }
}

#[cfg(any(test, feature = "test-support"))]
struct TestAuthorization;
#[cfg(any(test, feature = "test-support"))]
#[async_trait]
impl MutationAuthorizationPort for TestAuthorization {
    async fn validate_admission(&self, _accepted: &ActorContext) -> Result<(), AppError> {
        Ok(())
    }
}

#[cfg(any(test, feature = "test-support"))]
struct TestDirectory;
#[cfg(any(test, feature = "test-support"))]
static TEST_DIRECTORY: TestDirectory = TestDirectory;
#[cfg(any(test, feature = "test-support"))]
#[async_trait]
impl StaffDirectoryPort for TestDirectory {
    async fn collaboration_portraits(
        &self,
        _connection: &mut sqlx::PgConnection,
        _viewer: &ActorContext,
        _ids: &[Uuid],
    ) -> Result<Vec<(Uuid, Uuid)>, AppError> {
        Ok(Vec::new())
    }
    async fn subject_groups_for_users(
        &self,
        _connection: &mut sqlx::PgConnection,
        _ids: &[Uuid],
    ) -> Result<Vec<crate::ports::StaffSubjectGroupAssignment>, AppError> {
        Ok(Vec::new())
    }
}

/// Owned admission for a current Delivery cascade. The accepted command task must
/// retain this guard from before its transaction until this guard performs COMMIT.
/// School-wide Delivery management authorizes only the exact captured cascade.
pub struct TimetableDeletionGuard {
    context: TimetableMutationContext,
    year: Uuid,
    term: Uuid,
    expected_timetable: Option<Uuid>,
    expected_row_version: Option<i64>,
    permit: Option<Arc<CommitPermit>>,
    _room_gate: Option<tokio::sync::OwnedMutexGuard<()>>,
    _identity_gate: tokio::sync::OwnedMutexGuard<()>,
    _admission: tokio::sync::OwnedSemaphorePermit,
}
impl TimetableMutationContext {
    pub async fn admit_delivery_deletion(
        &self,
        pool: &PgPool,
        year: Uuid,
        term: Uuid,
    ) -> Result<TimetableDeletionGuard, AppError> {
        school_academic_delivery::services::deletion::require_school_management(&self.actor)?;
        if self.identity.is_some() {
            return Err(AppError::Conflict(
                "A placement lease cannot authorize a Delivery cascade".into(),
            ));
        }
        self.admit_current_aggregate(pool, year, term).await
    }
    async fn admit_current_aggregate(
        &self,
        pool: &PgPool,
        year: Uuid,
        term: Uuid,
    ) -> Result<TimetableDeletionGuard, AppError> {
        let admission = self.runtime.admit_write()?;
        let identity_gate = self.runtime.identity_gate(self.tenant_id, term).await?;
        self.authorization.validate_admission(&self.actor).await?;
        let current = crate::services::timetable_service::get_for_term(pool, term).await?;
        let expected_timetable = current.as_ref().map(|current| current.id);
        let expected_row_version = current.as_ref().map(|current| current.row_version);
        let (permit, room_gate) = if let Some(current) = current {
            if current.academic_year_id != year {
                return Err(AppError::Conflict("Timetable context changed".into()));
            }
            let room = self
                .runtime
                .load_room(self.tenant_id, pool, current.id, self.staff_directory)
                .await?;
            let gate = room.mutation_gate().await;
            self.authorization.validate_admission(&self.actor).await?;
            let permit = Arc::new(room.begin_commit(
                self.actor.user_id,
                self.session_id,
                None,
                self.lesson_source_detachment,
                MutationScope::WholeTimetable,
            )?);
            (Some(permit), Some(gate))
        } else {
            (None, None)
        };
        Ok(TimetableDeletionGuard {
            context: self.clone(),
            year,
            term,
            expected_timetable,
            expected_row_version,
            permit,
            _room_gate: room_gate,
            _identity_gate: identity_gate,
            _admission: admission,
        })
    }
}
impl TimetableDeletionGuard {
    pub async fn validate_current_on_connection(
        &self,
        connection: &mut sqlx::PgConnection,
        scope: &school_academic_delivery::models::current::DeliveryDeletionScope,
    ) -> Result<(), AppError> {
        if scope.academic_year_id != self.year || scope.academic_term_id != self.term {
            return Err(AppError::Conflict(
                "Deletion scope differs from accepted timetable admission".into(),
            ));
        }
        let current =
            crate::services::timetable_service::get_for_term_on_connection(connection, self.term)
                .await?;
        if current.as_ref().map(|current| current.id) != self.expected_timetable
            || current
                .as_ref()
                .is_some_and(|current| current.academic_year_id != self.year)
        {
            return Err(AppError::Conflict(
                "Current timetable appeared or changed; refresh deletion impact before retrying"
                    .into(),
            ));
        }
        Ok(())
    }
    pub async fn delete_in_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        scope: &school_academic_delivery::models::current::DeliveryDeletionScope,
    ) -> Result<(), AppError> {
        self.validate_current_on_connection(&mut **tx, scope)
            .await?;
        crate::services::deletion::delete_in_transaction(tx, scope, self.permit.as_deref()).await
    }
    pub async fn commit(
        &self,
        transaction: school_tenancy::ObservedTransaction,
        changed: bool,
    ) -> Result<(), AppError> {
        let result = if let Some(permit) = &self.permit {
            permit.commit(transaction).await
        } else {
            transaction.commit().await.map_err(AppError::from)
        };
        if changed || result.is_err() {
            self.context
                .runtime
                .invalidate_term(self.context.tenant_id, Some(self.term));
        }
        if result.is_ok() {
            if let Some(permit) = &self.permit {
                permit.settle();
            }
        }
        result
    }
}

/// Lifecycle owns authorization and its entire accepted multi-owner command;
/// preparation shares current timetable identity/room exclusion until outer COMMIT.
pub struct TimetablePreparationGuard {
    admission: TimetableDeletionGuard,
}
impl TimetableMutationContext {
    pub async fn admit_term_preparation(
        &self,
        pool: &PgPool,
        year: Uuid,
        term: Uuid,
    ) -> Result<TimetablePreparationGuard, AppError> {
        self.actor.require_permission(
            school_permissions::registry::codes::ACADEMIC_LIFECYCLE_MANAGE_SCHOOL,
        )?;
        if self.identity.is_some() {
            return Err(AppError::Conflict(
                "A placement lease cannot authorize term preparation".into(),
            ));
        }
        Ok(TimetablePreparationGuard {
            admission: self.admit_current_aggregate(pool, year, term).await?,
        })
    }
}
impl TimetablePreparationGuard {
    pub async fn apply_in_transaction(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        context: &school_academic_core::ports::TermPreparationContext,
        mappings: &std::collections::HashMap<
            (
                school_academic_core::ports::TermPreparationMappingKind,
                Uuid,
            ),
            Uuid,
        >,
    ) -> Result<school_academic_core::ports::TermPreparationModuleOutcome, AppError> {
        if context.target_year_id != self.admission.year
            || context.target_term_id != self.admission.term
        {
            return Err(AppError::Conflict(
                "Preparation differs from accepted timetable context".into(),
            ));
        }
        let current = crate::services::timetable_service::get_for_term_on_connection(
            &mut **tx,
            context.target_term_id,
        )
        .await?;
        if current.as_ref().map(|current| current.id) != self.admission.expected_timetable
            || current.as_ref().map(|current| current.row_version)
                != self.admission.expected_row_version
        {
            return Err(AppError::Conflict(
                "Target timetable changed; refresh preparation preview".into(),
            ));
        }
        let permit = match &self.admission.permit {
            Some(permit) => TimetableMutationPermit::Existing(permit.clone()),
            None => TimetableMutationPermit::Creation(Arc::new(CreationPermit {
                actor_id: self.admission.context.actor.user_id,
                phase: AtomicU8::new(0),
            })),
        };
        crate::services::timetable_service::apply_term_preparation(tx, &permit, context, mappings)
            .await
    }
    pub async fn commit(
        &self,
        transaction: school_tenancy::ObservedTransaction,
        changed: bool,
    ) -> Result<(), AppError> {
        self.admission.commit(transaction, changed).await
    }
}
