//! Process-owned schedule cache: idle tenants are not polled every minute.
use std::sync::Arc;
#[async_trait::async_trait]
pub trait AttendanceBehaviorPort: Send + Sync {
    async fn run(&self, pool: &sqlx::PgPool) -> Result<(), school_errors::AppError>;
}
#[derive(Clone)]
pub struct AttendanceRuntimeState {
    pub admin_client: Arc<school_tenancy::AdminClient>,
    pub pool_manager: Arc<school_tenancy::PoolManager>,
    pub notification_channel: broadcast::Sender<TenantNotificationEvent>,
    pub behavior: &'static dyn AttendanceBehaviorPort,
}

use crate::{
    models::*,
    services::{faces, notifications},
};
use chrono::{NaiveDate, Timelike, Utc};
use dashmap::DashMap;
use futures::{stream, StreamExt};
use school_errors::AppError;
use school_notifications::{
    events::TenantNotificationEvent,
    models::Notification,
    publisher::{NotificationService, TenantNotificationPublisher},
};
use sqlx::{types::Json, PgPool};
use std::{collections::HashSet, sync::OnceLock};
use tokio::sync::broadcast;
use uuid::Uuid;
const DELIVERY_CONCURRENCY: usize = 4;
const DELIVERY_ITEM_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

#[derive(Default, Debug, PartialEq)]
struct BatchDelivery {
    completed: usize,
    failed: usize,
    timed_out: usize,
}

async fn deliver_batch<T, F, Fut, E>(
    items: Vec<T>,
    concurrency: usize,
    timeout: std::time::Duration,
    deliver: F,
) -> BatchDelivery
where
    F: Fn(T) -> Fut,
    Fut: std::future::Future<Output = Result<(), E>>,
{
    // These futures are not spawned. The scheduled run's TenantPool lease owns
    // their complete lifetime, including cancellation and every sibling result.
    stream::iter(items)
        .map(|item| tokio::time::timeout(timeout, deliver(item)))
        .buffer_unordered(concurrency)
        .fold(BatchDelivery::default(), |mut outcome, result| async move {
            match result {
                Ok(Ok(())) => outcome.completed += 1,
                Ok(Err(_)) => outcome.failed += 1,
                Err(_) => outcome.timed_out += 1,
            }
            outcome
        })
        .await
}
#[derive(Clone)]
struct TermSchedule {
    term: Uuid,
    start: NaiveDate,
    end: NaiveDate,
    configuration: AttendanceConfiguration,
    days: Vec<AttendanceDay>,
}
#[derive(Clone)]
struct Registration {
    pool: PgPool,
    channel: broadcast::Sender<TenantNotificationEvent>,
    terms: Vec<TermSchedule>,
    pending: bool,
    generation: Uuid,
    module_generation: Uuid,
    digests: HashSet<String>,
    last_retention: Option<NaiveDate>,
    last_finalization_poll: Option<NaiveDate>,
}
static WAKE: tokio::sync::Notify = tokio::sync::Notify::const_new();
pub fn wake_background_work() {
    WAKE.notify_one();
}
static TENANTS: OnceLock<DashMap<String, Registration>> = OnceLock::new();
fn tenants() -> &'static DashMap<String, Registration> {
    TENANTS.get_or_init(DashMap::new)
}
pub async fn touch(
    tenant: &str,
    pool: PgPool,
    channel: broadcast::Sender<TenantNotificationEvent>,
) -> Result<(), AppError> {
    let rows:Vec<(Uuid,NaiveDate,NaiveDate,Json<AttendanceConfiguration>)>=sqlx::query_as("SELECT s.academic_term_id,t.start_date,COALESCE(t.closed_on,t.planned_end_date,y.end_date),s.configuration FROM attendance_settings s JOIN academic_terms t ON t.id=s.academic_term_id JOIN academic_years y ON y.id=t.academic_year_id WHERE y.status NOT IN ('closed','archived') AND t.status NOT IN ('closed','cancelled') AND NOT EXISTS(SELECT 1 FROM attendance_term_archives a WHERE a.academic_term_id=t.id)").fetch_all(&pool).await?;
    let mut terms = vec![];
    for (term, start, end, c) in rows {
        let days = sqlx::query_as(
            "SELECT date,counted,note FROM attendance_days WHERE academic_term_id=$1 ORDER BY date",
        )
        .bind(term)
        .fetch_all(&pool)
        .await?;
        terms.push(TermSchedule {
            term,
            start,
            end,
            configuration: c.0,
            days,
        });
    }
    let module_generation = crate::services::sessions::module_generation(&pool).await?;
    let pending = notifications::has_pending(&pool).await?;
    let prior = tenants()
        .get(tenant)
        .filter(|v| v.module_generation == module_generation)
        .map(|v| (v.digests.clone(), v.last_retention));
    let (digests, last_retention) = prior.unwrap_or_default();
    tenants().insert(
        tenant.into(),
        Registration {
            pool,
            channel,
            terms,
            pending,
            generation: Uuid::new_v4(),
            module_generation,
            digests,
            last_retention,
            last_finalization_poll: None,
        },
    );
    WAKE.notify_one();
    Ok(())
}
async fn refresh_directory(state: &AttendanceRuntimeState) {
    match state.admin_client.list_active_schools().await {
        Ok(schools) => {
            let active = schools
                .iter()
                .map(|s| s.subdomain.clone())
                .collect::<HashSet<_>>();
            tenants().retain(|tenant, _| active.contains(tenant));
            for school in schools {
                if let Some(url) = school.db_connection_string.filter(|s| !s.is_empty()) {
                    match state
                        .pool_manager
                        .get_pool_without_migrations(&url, &school.subdomain)
                        .await
                    {
                        Ok(pool) => {
                            if touch(
                                &school.subdomain,
                                pool.as_ref().clone(),
                                state.notification_channel.clone(),
                            )
                            .await
                            .is_err()
                            {
                                tracing::warn!(
                                    error_code = "attendance_schedule_unavailable",
                                    "Attendance startup schedule could not load"
                                );
                            }
                        }
                        Err(_) => tracing::warn!(
                            error_code = "attendance_tenant_unavailable",
                            "Attendance startup tenant unavailable"
                        ),
                    }
                }
            }
        }
        Err(_) => tracing::warn!(
            error_code = "attendance_directory_unavailable",
            "Attendance directory unavailable"
        ),
    }
}

pub async fn start(state: AttendanceRuntimeState) {
    refresh_directory(&state).await;
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
    let mut directory_interval = tokio::time::interval_at(
        tokio::time::Instant::now() + std::time::Duration::from_secs(300),
        std::time::Duration::from_secs(300),
    );
    loop {
        tokio::select! {
            _=interval.tick()=>{},
            _=WAKE.notified()=>{},
            _=directory_interval.tick()=>{ refresh_directory(&state).await; }
        }
        let work: Vec<_> = tenants()
            .iter()
            .map(|v| (v.key().clone(), v.value().clone()))
            .collect();
        for (tenant, mut r) in work {
            // Dormant registrations never pin a pool. Reacquire for the whole logical run.
            let school = match state.admin_client.get_school_database_info(&tenant).await {
                Ok(school) => school,
                Err(_) => continue,
            };
            let operation_pool = match state
                .pool_manager
                .get_pool(&school.database_url, &tenant)
                .await
            {
                Ok(pool) => pool,
                Err(_) => continue,
            };
            r.pool = operation_pool.as_ref().clone();
            if run(&tenant, &mut r, state.behavior).await.is_err() {
                tracing::warn!(
                    error_code = "attendance_background_retry",
                    "Attendance background work will retry"
                );
            }
            if let Some(mut live) = tenants().get_mut(&tenant) {
                if live.generation == r.generation {
                    live.digests.extend(r.digests);
                    live.last_retention = r.last_retention;
                    live.pending = r.pending;
                    live.terms = r.terms;
                    live.last_finalization_poll = r.last_finalization_poll;
                }
            }
        }
    }
}
async fn run(
    tenant: &str,
    r: &mut Registration,
    behavior: &dyn AttendanceBehaviorPort,
) -> Result<(), AppError> {
    let now = Utc::now().with_timezone(&chrono_tz::Asia::Bangkok);
    let date = now.date_naive();
    if r.last_finalization_poll != Some(date) {
        let processed = crate::application::finalization::catch_up(&r.pool, Utc::now(), 4).await?;
        if processed < 4 {
            r.last_finalization_poll = Some(date);
        }
    }
    if behavior.run(&r.pool).await.is_err() {
        tracing::warn!(
            error_code = "behavior_background_retry",
            "Score delivery remains queued and will retry"
        );
    }
    for t in r.terms.clone() {
        if date < t.start || date > t.end {
            continue;
        }
        let day = t.days.iter().find(|d| d.date == date).map(|d| d.counted);
        if !crate::rules::counted(&t.configuration, date, day) {
            continue;
        }
        if !t.configuration.enabled {
            continue;
        }
        for time in t.configuration.digest_times {
            let key = format!("{}:{date}:{time}", t.term);
            let current = now.hour() * 60 + now.minute();
            let due = time.hour() * 60 + time.minute();
            if digest_due(current, due, r.digests.contains(&key)) {
                let mut tx = r.pool.begin().await?;
                crate::services::lock_term(&mut tx, t.term, false, true).await?;
                let expected =
                    crate::application::projection::expected(&mut *tx, t.term, Some((date, date)))
                        .await?;
                notifications::queue_digest_in_transaction(
                    tx,
                    t.term,
                    date,
                    time,
                    r.module_generation,
                    &expected,
                )
                .await?;
                r.pending = true;
                r.digests.insert(key);
            }
        }
    }
    if r.pending {
        let batch = notifications::pending(&r.pool).await?;
        let pool = &r.pool;
        let channel = &r.channel;
        let outcome = deliver_batch(
            batch,
            DELIVERY_CONCURRENCY,
            DELIVERY_ITEM_TIMEOUT,
            |n| async move {
                if !notifications::store(pool, &n).await? {
                    return Ok::<(), AppError>(());
                }
                let notice:Notification=sqlx::query_as("SELECT id,title,message,type type_,link,read_at,created_at FROM notifications WHERE id=$1").bind(n.notification_id).fetch_one(pool).await?;
                let publisher = TenantNotificationPublisher::new(tenant, channel);
                NotificationService::deliver_stored(pool, &publisher, n.recipient_id, notice)
                    .await
                    .map_err(|_| AppError::ServiceUnavailable("notification_transport_unavailable".into()))?;
                notifications::delivered(pool, n.id).await
            },
        ).await;
        if outcome.failed > 0 || outcome.timed_out > 0 {
            tracing::warn!(
                error_code = "attendance_notification_retry",
                failed = outcome.failed,
                timed_out = outcome.timed_out,
                "Attendance notices remain leased for retry"
            );
        }
        r.pending = notifications::has_pending(&r.pool).await?;
        if notifications::has_due(&r.pool).await? {
            // Finish the remaining tenants in this pass before the next batch.
            // Future leased retries wait for the regular tick instead of spinning.
            WAKE.notify_one();
        }
    }
    if r.last_retention != Some(date) {
        if faces::expire_evidence(&r.pool).await? < faces::EVIDENCE_EXPIRY_BATCH {
            r.last_retention = Some(date);
        }
    }
    Ok(())
}
fn digest_due(current: u32, due: u32, delivered: bool) -> bool {
    !delivered && current >= due && current <= due + 60
}
#[cfg(test)]
mod tests {
    use super::{deliver_batch, digest_due, BatchDelivery};
    use std::sync::{atomic::{AtomicUsize, Ordering}, Arc};
    #[test]
    fn digest_window_catches_up_once_and_does_not_send_early() {
        assert!(!digest_due(479, 480, false));
        assert!(digest_due(480, 480, false));
        assert!(digest_due(540, 480, false));
        assert!(!digest_due(541, 480, false));
        assert!(!digest_due(480, 480, true));
    }
    #[tokio::test]
    async fn delivery_batch_bounds_concurrency_and_joins_failed_and_timed_out_siblings() {
        struct Active(Arc<AtomicUsize>);
        impl Drop for Active {
            fn drop(&mut self) { self.0.fetch_sub(1, Ordering::SeqCst); }
        }
        let active = Arc::new(AtomicUsize::new(0));
        let peak = Arc::new(AtomicUsize::new(0));
        let completed = Arc::new(AtomicUsize::new(0));
        let result = deliver_batch((0..10).collect(), 4, std::time::Duration::from_millis(100), |item| {
            let active = Arc::clone(&active);
            let peak = Arc::clone(&peak);
            let completed = Arc::clone(&completed);
            async move {
                let count = active.fetch_add(1, Ordering::SeqCst) + 1;
                let _active = Active(active);
                peak.fetch_max(count, Ordering::SeqCst);
                if item == 0 { std::future::pending::<()>().await; }
                tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                if item == 1 { return Err(()); }
                completed.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }).await;
        assert_eq!(result, BatchDelivery { completed: 8, failed: 1, timed_out: 1 });
        assert_eq!(peak.load(Ordering::SeqCst), 4, "independent work must overlap up to the bound");
        assert_eq!(active.load(Ordering::SeqCst), 0, "no delivery may outlive the batch");
        assert_eq!(completed.load(Ordering::SeqCst), 8, "one failure must not cancel siblings");
    }
}

pub async fn refresh_after_commit(
    subdomain: &str,
    pool: sqlx::PgPool,
    channel: broadcast::Sender<TenantNotificationEvent>,
) {
    if touch(subdomain, pool, channel).await.is_err() {
        tracing::error!(
            error_code = "attendance_cache_refresh",
            "Attendance schedule refresh failed after commit; durable notices remain queued"
        );
    }
}

pub async fn wake_after_commit(
    tenant: &str,
    pool: PgPool,
    channel: broadcast::Sender<TenantNotificationEvent>,
) {
    {
        if let Some(mut entry) = tenants().get_mut(tenant) {
            WAKE.notify_one();
            entry.pending = true;
            entry.generation = Uuid::new_v4();
            entry.last_finalization_poll = None;
            return;
        }
    }
    refresh_after_commit(tenant, pool, channel).await;
}
