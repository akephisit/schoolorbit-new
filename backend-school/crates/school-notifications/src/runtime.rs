//! The sole durable notification dispatcher. Registrations hold names, not pools.
use crate::{
    dispatch::{self, DeliveryEligibilityPort},
    events::TenantNotificationEvent,
    publisher::{NotificationService, TenantNotificationPublisher},
};
use futures::{stream, StreamExt};
use school_errors::AppError;
use school_tenancy::{
    background::{TenantJobKind, TenantTurn},
    AdminClient, PoolManager, ResolvedTenantDatabase, TenantPool,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tokio::sync::broadcast;

const DELIVERY_CONCURRENCY: usize = 4;
const DATABASE_TIMEOUT: Duration = Duration::from_secs(5);
const ADMISSION_TIMEOUT: Duration = Duration::from_secs(5);
static WAKE: tokio::sync::Notify = tokio::sync::Notify::const_new();
static HINTS: OnceLock<Mutex<BTreeSet<String>>> = OnceLock::new();
fn hints() -> &'static Mutex<BTreeSet<String>> {
    HINTS.get_or_init(Mutex::default)
}
fn hint_guard() -> std::sync::MutexGuard<'static, BTreeSet<String>> {
    match hints().lock() {
        Ok(guard) => guard,
        Err(poisoned) => {
            tracing::warn!(
                error_code = "notification_hint_recovered",
                "Notification hint state recovered"
            );
            poisoned.into_inner()
        }
    }
}
pub fn wake_after_commit(tenant: &str) {
    hint_guard().insert(tenant.into());
    WAKE.notify_one();
}
#[derive(Clone)]
pub struct NotificationRuntimeState {
    pub admin_client: Arc<AdminClient>,
    pub pool_manager: Arc<PoolManager>,
    pub notification_channel: broadcast::Sender<TenantNotificationEvent>,
    pub eligibility: &'static dyn DeliveryEligibilityPort,
}
#[derive(Default, Debug, PartialEq)]
struct BatchDelivery {
    completed: usize,
    failed: usize,
}
async fn deliver_batch<T, F, Fut, E>(items: Vec<T>, concurrency: usize, deliver: F) -> BatchDelivery
where
    F: Fn(T) -> Fut,
    Fut: std::future::Future<Output = Result<(), E>>,
{
    // Unspawned futures cannot outlive the caller's logical TenantPool lease.
    stream::iter(items)
        .map(deliver)
        .buffer_unordered(concurrency)
        .fold(BatchDelivery::default(), |mut result, item| async move {
            match item {
                Ok(()) => result.completed += 1,
                Err(_) => result.failed += 1,
            }
            result
        })
        .await
}
async fn database<T>(
    future: impl std::future::Future<Output = Result<T, AppError>>,
) -> Result<T, AppError> {
    tokio::time::timeout(DATABASE_TIMEOUT, future)
        .await
        .map_err(|_| AppError::ServiceUnavailable("notification_database_timeout".into()))?
}
/// Unmanaged fixture adapter; shares the same orchestration and fenced finish as production.
pub async fn deliver_tenant_batch(
    pool: &sqlx::PgPool,
    tenant: &str,
    channel: &broadcast::Sender<TenantNotificationEvent>,
    eligibility: &dyn DeliveryEligibilityPort,
) -> Result<dispatch::QueueStatus, AppError> {
    deliver_database_batch(
        &DeliveryDatabase::Unmanaged(pool),
        tenant,
        channel,
        eligibility,
        None,
        DeliveryTransport::Network(std::marker::PhantomData),
    )
    .await
}

#[derive(Clone, Copy)]
enum DeliveryTransport<'a> {
    Network(std::marker::PhantomData<&'a ()>),
    #[cfg(feature = "test-support")]
    Controlled(&'a dyn test_support::ControlledTransport),
}
impl DeliveryTransport<'_> {
    async fn deliver(
        self,
        publisher: &TenantNotificationPublisher<'_>,
        user: uuid::Uuid,
        prepared: crate::publisher::PreparedNotification,
    ) -> Result<crate::publisher::DeliveryReceipt, &'static str> {
        match self {
            Self::Network(_) => {
                NotificationService::deliver_prepared(publisher, user, prepared).await
            }
            #[cfg(feature = "test-support")]
            Self::Controlled(transport) => {
                let result = transport.deliver(prepared.notification.id).await?;
                Ok(crate::publisher::DeliveryReceipt {
                    invalid_devices: result.invalid_devices,
                })
            }
        }
    }
}

#[cfg(feature = "test-support")]
pub mod test_support {
    use super::*;
    pub struct DeliveryResult {
        pub invalid_devices: Vec<(uuid::Uuid, uuid::Uuid)>,
    }
    #[async_trait::async_trait]
    pub trait ControlledTransport: Send + Sync {
        async fn deliver(
            &self,
            notification_id: uuid::Uuid,
        ) -> Result<DeliveryResult, &'static str>;
    }
    pub async fn deliver_managed_batch(
        state: &NotificationRuntimeState,
        tenant: &str,
        transport: &dyn ControlledTransport,
    ) -> Result<dispatch::QueueStatus, AppError> {
        deliver_managed_with_transport(state, tenant, DeliveryTransport::Controlled(transport))
            .await
    }
}

enum DeliveryDatabase<'a> {
    Managed(&'a NotificationRuntimeState),
    Unmanaged(&'a sqlx::PgPool),
}
impl DeliveryDatabase<'_> {
    async fn resolve(&self, tenant: &str) -> Result<Option<ResolvedTenantDatabase>, AppError> {
        match self {
            Self::Unmanaged(_) => Ok(None),
            Self::Managed(state) => state
                .admin_client
                .resolve_database_work(tenant)
                .await
                .map(Some)
                .map_err(|_| {
                    AppError::ServiceUnavailable("notification_tenant_unavailable".into())
                }),
        }
    }
    async fn pool(
        &self,
        resolved: Option<&ResolvedTenantDatabase>,
        expected: Option<uuid::Uuid>,
    ) -> Result<(uuid::Uuid, TenantPool), AppError> {
        match self {
            Self::Unmanaged(pool) => Ok((uuid::Uuid::nil(), (*pool).clone().into())),
            Self::Managed(state) => {
                let resolved = resolved.ok_or_else(|| {
                    AppError::ServiceUnavailable("notification_tenant_unavailable".into())
                })?;
                state
                    .admin_client
                    .pool_for_resolved_work(&state.pool_manager, resolved, expected)
                    .await
                    .map_err(|_| {
                        AppError::ServiceUnavailable("notification_tenant_unavailable".into())
                    })
            }
        }
    }
}

async fn resolved_phase<T, F, Fut>(
    access: &DeliveryDatabase<'_>,
    tenant: &str,
    expected: Option<uuid::Uuid>,
    turn: Option<&TenantTurn<'_>>,
    phase: &'static str,
    work: F,
) -> Result<T, AppError>
where
    F: FnOnce(uuid::Uuid, TenantPool) -> Fut,
    Fut: std::future::Future<Output = Result<T, AppError>>,
{
    let resolved = access.resolve(tenant).await?;
    tracing::debug!(
        operation = "notification_database_phase",
        phase,
        "Tenant database work resolved"
    );
    admitted(turn, || async {
        let (id, pool) = access.pool(resolved.as_ref(), expected).await?;
        work(id, pool).await
    })
    .await
}

async fn admitted<T, F, Fut>(turn: Option<&TenantTurn<'_>>, phase: F) -> Result<T, AppError>
where
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, AppError>>,
{
    match turn {
        Some(turn) => turn.run(phase).await.ok_or_else(|| {
            AppError::ServiceUnavailable("notification_admission_unavailable".into())
        })?,
        None => phase().await,
    }
}

async fn deliver_managed_tenant_batch(
    state: &NotificationRuntimeState,
    tenant: &str,
) -> Result<dispatch::QueueStatus, AppError> {
    deliver_managed_with_transport(
        state,
        tenant,
        DeliveryTransport::Network(std::marker::PhantomData),
    )
    .await
}

async fn deliver_managed_with_transport(
    state: &NotificationRuntimeState,
    tenant: &str,
    transport: DeliveryTransport<'_>,
) -> Result<dispatch::QueueStatus, AppError> {
    let turns = state.pool_manager.background_turns();
    let Some(turn) = turns.begin(tenant, TenantJobKind::NotificationDispatch) else {
        return Err(AppError::ServiceUnavailable(
            "notification_turn_active".into(),
        ));
    };
    deliver_database_batch(
        &DeliveryDatabase::Managed(state),
        tenant,
        &state.notification_channel,
        state.eligibility,
        Some(&turn),
        transport,
    )
    .await
}

async fn deliver_database_batch(
    access: &DeliveryDatabase<'_>,
    tenant: &str,
    channel: &broadcast::Sender<TenantNotificationEvent>,
    eligibility: &dyn DeliveryEligibilityPort,
    turn: Option<&TenantTurn<'_>>,
    transport: DeliveryTransport<'_>,
) -> Result<dispatch::QueueStatus, AppError> {
    let (tenant_id, batch) = database(resolved_phase(
        access,
        tenant,
        None,
        turn,
        "pending",
        |id, pool| async move { Ok((id, dispatch::pending(&pool).await?)) },
    ))
    .await?;
    let outcome = deliver_batch(batch, DELIVERY_CONCURRENCY, |pending| async move {
        let notification = match database(resolved_phase(
            access,
            tenant,
            Some(tenant_id),
            turn,
            "consume",
            |_, pool| async move { dispatch::consume(&pool, &pending, eligibility).await },
        ))
        .await
        {
            Ok(notification) => notification,
            Err(AppError::Conflict(_)) => {
                database(resolved_phase(
                    access,
                    tenant,
                    Some(tenant_id),
                    turn,
                    "block",
                    |_, pool| async move { dispatch::block(&pool, &pending).await },
                ))
                .await?;
                tracing::warn!(
                    error_code = "notification_presentation_blocked",
                    "Corrupted presentation identity blocked without transport; inbox retained"
                );
                return Ok::<(), AppError>(());
            }
            Err(error) => {
                tracing::warn!(
                    error_code = "notification_presentation_deferred",
                    "Presentation remains unconsumed; database recovery deferred"
                );
                return Err(error);
            }
        };
        let Some(notification) = notification else {
            return Ok::<(), AppError>(());
        };
        let publisher = TenantNotificationPublisher::new(tenant, channel);
        // Consume committed before SSE; Push query/configuration errors cannot suppress SSE.
        publisher.publish(pending.recipient_id, notification.clone());
        let prepared = database(resolved_phase(
            access,
            tenant,
            Some(tenant_id),
            turn,
            "devices",
            |_, pool| async move {
                NotificationService::prepare_stored(
                    &pool,
                    pending.recipient_id,
                    notification,
                    pending.expires_at,
                )
                .await
                .map_err(|_| {
                    AppError::ServiceUnavailable("notification_subscription_unavailable".into())
                })
            },
        ))
        .await;
        let prepared = match prepared {
            Ok(prepared) => prepared,
            Err(_) => {
                tracing::warn!(
                    error_code = "notification_push_snapshot_unavailable",
                    "SSE attempted; Push skipped without replay"
                );
                return Ok(());
            }
        };
        // No database pool/admission permit crosses this external I/O boundary.
        let receipt = transport
            .deliver(&publisher, pending.recipient_id, prepared)
            .await
            .map_err(|_| {
                tracing::warn!(
                    error_code = "notification_consumed_transport_failed",
                    "Consumed presentation transport ended without replay"
                );
                AppError::ServiceUnavailable("notification_transport_unavailable".into())
            })?;
        if receipt.invalid_devices.is_empty() {
            return Ok(());
        }
        database(resolved_phase(
            access,
            tenant,
            Some(tenant_id),
            turn,
            "cleanup",
            |_, pool| async move {
                NotificationService::complete_delivery(&pool, pending.recipient_id, receipt)
                    .await
                    .map_err(|_| {
                        AppError::ServiceUnavailable(
                            "notification_device_cleanup_unavailable".into(),
                        )
                    })
            },
        ))
        .await
    })
    .await;
    if outcome.failed > 0 {
        tracing::warn!(
            error_code = "notification_presentation_attempt_ended",
            failed = outcome.failed,
            "Pending database work may recover; consumed presentations will not replay"
        );
    }

    let mut progress = database(resolved_phase(
        access,
        tenant,
        Some(tenant_id),
        turn,
        "status",
        |_, pool| async move {
            crate::devices::cleanup(&pool).await?;
            dispatch::status(&pool).await
        },
    ))
    .await?;
    // Pre-consumption database/source errors recover on a bounded later probe, never a hot loop.
    if outcome.failed > 0 {
        progress.due = false;
    }
    Ok(progress)
}

struct TenantSchedule {
    pending: bool,
    next_probe: tokio::time::Instant,
}
impl TenantSchedule {
    fn scan() -> Self {
        Self {
            pending: true,
            next_probe: tokio::time::Instant::now(),
        }
    }
    fn defer(&mut self) {
        self.pending = true;
        self.next_probe = tokio::time::Instant::now() + Duration::from_secs(15);
    }
}
pub async fn start(state: NotificationRuntimeState) {
    if ["VAPID_PUBLIC_KEY", "VAPID_PRIVATE_KEY", "VAPID_SUBJECT"]
        .iter()
        .any(|key| std::env::var_os(key).is_some())
        && !crate::devices::push_config().available
    {
        tracing::warn!(
            error_code = "notification_push_configuration_unavailable",
            "Configured Push key pair/contact is invalid; inbox and SSE remain available"
        );
    }

    let mut tenants: BTreeMap<String, TenantSchedule> = BTreeMap::new();
    let mut refresh_due = true;
    let mut last_refresh = tokio::time::Instant::now();
    let mut interval = tokio::time::interval(Duration::from_secs(15));
    loop {
        let regular = tokio::select! { _=interval.tick()=>true,_=WAKE.notified()=>false };
        if last_refresh.elapsed() >= Duration::from_secs(300) {
            refresh_due = true;
        }
        if refresh_due && regular {
            match tokio::time::timeout(ADMISSION_TIMEOUT, state.admin_client.list_active_schools())
                .await
            {
                Ok(Ok(schools)) => {
                    let active: BTreeSet<_> = schools.into_iter().map(|s| s.subdomain).collect();
                    tenants.retain(|name, _| active.contains(name));
                    for name in active {
                        tenants.insert(name, TenantSchedule::scan());
                    }
                    refresh_due = false;
                    last_refresh = tokio::time::Instant::now();
                }
                _ => tracing::warn!(
                    error_code = "notification_directory_unavailable",
                    "Notification directory will retry"
                ),
            }
        }
        for name in std::mem::take(&mut *hint_guard()) {
            tenants.insert(name, TenantSchedule::scan());
        }
        let eligible: Vec<_> = tenants
            .iter()
            .filter(|(_, schedule)| {
                schedule.pending && schedule.next_probe <= tokio::time::Instant::now()
            })
            .map(|(tenant, _)| tenant.clone())
            .collect();
        let limit = state.pool_manager.background_turns().limit();
        let state_ref = &state;
        let mut running = stream::iter(eligible)
            .map(|tenant| async move {
                let result = deliver_managed_tenant_batch(state_ref, &tenant).await;
                (tenant, result)
            })
            .buffer_unordered(limit);
        let mut due = false;
        while let Some((tenant, result)) = running.next().await {
            let Some(schedule) = tenants.get_mut(&tenant) else {
                continue;
            };
            match result {
                Ok(progress) => {
                    schedule.pending = progress.pending;
                    if !progress.pending {
                        tracing::debug!(
                            operation = "notification_schedule",
                            phase = "idle",
                            "Notification tenant has no pending presentation"
                        );
                    }
                    schedule.next_probe = tokio::time::Instant::now()
                        + if progress.due {
                            Duration::ZERO
                        } else {
                            Duration::from_secs(15)
                        };
                    due |= progress.due;
                }
                Err(_) => {
                    schedule.defer();
                    tracing::warn!(
                        error_code = "notification_dispatch_unavailable",
                        "Notification tenant work will retry"
                    );
                }
            }
        }
        // All eligible tenants receive a turn before another bounded batch.
        // Idle/future/error registrations are skipped during hot-backlog passes.
        if due {
            WAKE.notify_one();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn network_wait_holds_no_database_admission_and_cancellation_joins_items() {
        use school_tenancy::{background::TenantJobKind, PoolManager};
        let turns = PoolManager::new().background_turns();
        let worker_turns = turns.clone();
        let (started, mut observed) = tokio::sync::mpsc::unbounded_channel();
        let task = tokio::spawn(async move {
            let turn = worker_turns
                .begin("slow", TenantJobKind::NotificationDispatch)
                .unwrap();
            deliver_batch(vec![0, 1], 2, |_| {
                let turn = &turn;
                let started = &started;
                async move {
                    admitted(Some(turn), || async { Ok(()) }).await.unwrap();
                    started.send(()).unwrap();
                    std::future::pending::<()>().await;
                    Ok::<(), ()>(())
                }
            })
            .await
        });
        observed.recv().await.unwrap();
        observed.recv().await.unwrap();
        assert_eq!(
            tokio::time::timeout(
                Duration::from_millis(100),
                turns.run("other", TenantJobKind::AuthRetention, || async { 7 })
            )
            .await
            .unwrap(),
            Some(7)
        );
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert!(turns
            .begin("slow", TenantJobKind::NotificationDispatch)
            .is_some());
    }
}
