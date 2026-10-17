//! The sole durable notification dispatcher. Registrations hold names, not pools.
use crate::{
    dispatch::{self, DeliveryEligibilityPort},
    events::TenantNotificationEvent,
    publisher::{NotificationService, TenantNotificationPublisher},
};
use futures::{stream, StreamExt};
use school_errors::AppError;
use school_tenancy::{AdminClient, PoolManager};
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
pub async fn deliver_tenant_batch(
    pool: &sqlx::PgPool,
    tenant: &str,
    channel: &broadcast::Sender<TenantNotificationEvent>,
    eligibility: &dyn DeliveryEligibilityPort,
) -> Result<dispatch::QueueStatus, AppError> {
    let batch = database(dispatch::claim(pool)).await?;
    let outcome = deliver_batch(
        batch,
        DELIVERY_CONCURRENCY,
        DELIVERY_ITEM_TIMEOUT,
        |claimed| async move {
            if let Some(notification) = dispatch::materialize(pool, &claimed, eligibility).await? {
                let publisher = TenantNotificationPublisher::new(tenant, channel);
                NotificationService::deliver_stored(
                    pool,
                    &publisher,
                    claimed.notification.recipient_id,
                    notification,
                )
                .await
                .map_err(|_| {
                    AppError::ServiceUnavailable("notification_transport_unavailable".into())
                })?;
                dispatch::acknowledge(pool, &claimed).await?;
            }
            Ok::<(), AppError>(())
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
    database(dispatch::status(pool)).await
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
        let mut due = false;
        for (tenant, schedule) in &mut tenants {
            if !schedule.pending || schedule.next_probe > tokio::time::Instant::now() {
                continue;
            }
            let admitted = tokio::time::timeout(ADMISSION_TIMEOUT, async {
                let school = state.admin_client.get_school_database_info(tenant).await?;
                state
                    .pool_manager
                    .get_pool(&school.database_url, tenant)
                    .await
            })
            .await;
            let pool = match admitted {
                Ok(Ok(pool)) => pool,
                _ => {
                    schedule.defer();
                    tracing::warn!(
                        error_code = "notification_tenant_unavailable",
                        "Notification tenant admission will retry"
                    );
                    continue;
                }
            };
            // Keep this logical lease through every independent delivery future.
            match deliver_tenant_batch(
                &pool,
                tenant,
                &state.notification_channel,
                state.eligibility,
            )
            .await
            {
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
