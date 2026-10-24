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
    AdminClient, PoolManager, TenantPool,
};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use tokio::sync::broadcast;

const DELIVERY_CONCURRENCY: usize = 4;
const DELIVERY_ITEM_TIMEOUT: Duration = Duration::from_secs(5);
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
    timed_out: usize,
}
async fn deliver_batch<T, F, Fut, E>(
    items: Vec<T>,
    concurrency: usize,
    timeout: Duration,
    deliver: F,
) -> BatchDelivery
where
    F: Fn(T) -> Fut,
    Fut: std::future::Future<Output = Result<(), E>>,
{
    // Unspawned futures cannot outlive the caller's logical TenantPool lease.
    stream::iter(items)
        .map(|item| tokio::time::timeout(timeout, deliver(item)))
        .buffer_unordered(concurrency)
        .fold(BatchDelivery::default(), |mut result, item| async move {
            match item {
                Ok(Ok(())) => result.completed += 1,
                Ok(Err(_)) => result.failed += 1,
                Err(_) => result.timed_out += 1,
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
                    invalid_endpoints: result.invalid_endpoints,
                    retryable: result.retryable,
                })
            }
        }
    }
}

#[cfg(feature = "test-support")]
pub mod test_support {
    use super::*;
    pub struct DeliveryResult {
        pub invalid_endpoints: Vec<String>,
        pub retryable: bool,
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
    async fn pool(
        &self,
        tenant: &str,
        expected: Option<uuid::Uuid>,
    ) -> Result<(uuid::Uuid, TenantPool), AppError> {
        match self {
            Self::Unmanaged(pool) => Ok((uuid::Uuid::nil(), (*pool).clone().into())),
            Self::Managed(state) => {
                let school = state
                    .admin_client
                    .get_school_database_info(tenant)
                    .await
                    .map_err(|_| {
                        AppError::ServiceUnavailable("notification_tenant_unavailable".into())
                    })?;
                if expected.is_some_and(|id| id != school.tenant_id) {
                    return Err(AppError::ServiceUnavailable(
                        "notification_tenant_changed".into(),
                    ));
                }
                let pool = state
                    .pool_manager
                    .get_pool(&school.database_url, tenant)
                    .await
                    .map_err(|_| {
                        AppError::ServiceUnavailable("notification_tenant_unavailable".into())
                    })?;
                Ok((school.tenant_id, pool))
            }
        }
    }
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
    let claimed_at = tokio::time::Instant::now();
    let (tenant_id, batch) = database(admitted(turn, || async {
        let (id, pool) = access.pool(tenant, None).await?;
        Ok((id, dispatch::claim(&pool).await?))
    }))
    .await?;
    let outcome = deliver_batch(
        batch,
        DELIVERY_CONCURRENCY,
        DELIVERY_ITEM_TIMEOUT,
        |claimed| async move {
            let prepared = admitted(turn, || async {
                let (_, pool) = access.pool(tenant, Some(tenant_id)).await?;
                let Some(notification) =
                    dispatch::materialize(&pool, &claimed, eligibility).await?
                else {
                    return Ok(None);
                };
                NotificationService::prepare_stored(
                    &pool,
                    claimed.notification.recipient_id,
                    notification,
                    claimed.attempts,
                )
                .await
                .map(Some)
                .map_err(|_| {
                    AppError::ServiceUnavailable("notification_subscription_unavailable".into())
                })
            })
            .await?;
            let Some(prepared) = prepared else {
                return Ok(());
            };
            if claimed_at.elapsed() >= Duration::from_secs(115) {
                return Err(AppError::ServiceUnavailable(
                    "notification_claim_expired".into(),
                ));
            }
            // Only owned transport DTOs cross this boundary; no pool or permit survives.
            let publisher = TenantNotificationPublisher::new(tenant, channel);
            let receipt = transport
                .deliver(&publisher, claimed.notification.recipient_id, prepared)
                .await
                .map_err(|_| {
                    AppError::ServiceUnavailable("notification_transport_unavailable".into())
                })?;
            admitted(turn, || async {
                let (_, pool) = access.pool(tenant, Some(tenant_id)).await?;
                dispatch::finish_delivery(&pool, &claimed, receipt).await
            })
            .await
        },
    )
    .await;
    if outcome.failed > 0 || outcome.timed_out > 0 {
        tracing::warn!(
            error_code = "notification_dispatch_retry",
            failed = outcome.failed,
            timed_out = outcome.timed_out,
            "Notification jobs remain leased for retry"
        );
    }
    database(admitted(turn, || async {
        let (_, pool) = access.pool(tenant, Some(tenant_id)).await?;
        dispatch::status(&pool).await
    }))
    .await
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
    use super::{deliver_batch, BatchDelivery};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    #[tokio::test]
    async fn slow_transport_releases_shared_database_slots_for_other_kinds_and_joins_cancellation()
    {
        use school_tenancy::{background::TenantJobKind, PoolManager};
        let turns = PoolManager::new().background_turns();
        let worker_turns = turns.clone();
        let (started, mut observed) = tokio::sync::mpsc::unbounded_channel();
        let active = Arc::new(AtomicUsize::new(0));
        let worker_active = active.clone();
        struct Active(Arc<AtomicUsize>);
        impl Drop for Active {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }
        let worker = tokio::spawn(async move {
            let turn = worker_turns
                .begin("network", TenantJobKind::NotificationDispatch)
                .unwrap();
            deliver_batch(vec![0, 1], 2, std::time::Duration::from_millis(200), |_| {
                let turn = &turn;
                let started = &started;
                let active = worker_active.clone();
                async move {
                    super::admitted(Some(turn), || async { Ok(()) })
                        .await
                        .unwrap();
                    active.fetch_add(1, Ordering::SeqCst);
                    let _active = Active(active);
                    started.send(()).unwrap();
                    std::future::pending::<()>().await;
                    Ok::<(), ()>(())
                }
            })
            .await
        });
        observed.recv().await.unwrap();
        observed.recv().await.unwrap();
        for kind in [
            TenantJobKind::CalendarReminders,
            TenantJobKind::AuthRetention,
            TenantJobKind::AttendanceMaintenance,
            TenantJobKind::FileReconciliation,
        ] {
            assert_eq!(
                tokio::time::timeout(
                    std::time::Duration::from_millis(100),
                    turns.run("healthy", kind, || async { 7 })
                )
                .await
                .unwrap(),
                Some(7)
            );
        }
        assert!(turns
            .begin("network", TenantJobKind::NotificationDispatch)
            .is_none());
        assert_eq!(worker.await.unwrap().timed_out, 2);
        assert_eq!(active.load(Ordering::SeqCst), 0);
        assert!(turns
            .begin("network", TenantJobKind::NotificationDispatch)
            .is_some());
    }

    #[tokio::test]
    async fn delivery_batch_bounds_concurrency_and_joins_failed_and_timed_out_siblings() {
        struct Active(Arc<AtomicUsize>);
        impl Drop for Active {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicUsize::new(0));
        let result = deliver_batch(
            (0..10).collect(),
            4,
            std::time::Duration::from_millis(100),
            |item| {
                let active = Arc::clone(&active);
                let peak = Arc::clone(&peak);
                let completed = Arc::clone(&completed);
                async move {
                    let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                    let _active = Active(active);
                    peak.fetch_max(count, Ordering::SeqCst);
                    if item == 0 {
                        std::future::pending::<()>().await;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                    if item == 1 {
                        return Err(());
                    }
                    completed.fetch_add(1, Ordering::SeqCst);
                    Ok(())
                }
            },
        )
        .await;
        assert_eq!(
            result,
            BatchDelivery {
                completed: 8,
                failed: 1,
                timed_out: 1
            }
        );
        assert_eq!(
            peak.load(Ordering::SeqCst),
            4,
            "independent work must overlap up to the bound"
        );
        assert_eq!(
            active.load(Ordering::SeqCst),
            0,
            "no delivery may outlive the batch"
        );
        assert_eq!(
            completed.load(Ordering::SeqCst),
            8,
            "one failure must not cancel siblings"
        );
    }
}
