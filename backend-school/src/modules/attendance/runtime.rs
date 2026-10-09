//! Process-owned schedule cache: idle tenants are not polled every minute.
use crate::AppState;
use chrono::{NaiveDate, Timelike, Utc};
use dashmap::DashMap;
use school_attendance::{
    models::*,
    services::{faces, notifications, sessions},
};
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
#[derive(Clone)]
struct TermSchedule {
    term: Uuid,
    start: NaiveDate,
    end: NaiveDate,
    configuration: AttendanceConfiguration,
    days: Vec<AttendanceDay>,
    completed: HashSet<NaiveDate>,
}
#[derive(Clone)]
struct Registration {
    pool: PgPool,
    channel: broadcast::Sender<TenantNotificationEvent>,
    terms: Vec<TermSchedule>,
    pending: bool,
    generation: Uuid,
    digests: HashSet<String>,
    last_retention: Option<NaiveDate>,
}
static WAKE: tokio::sync::Notify = tokio::sync::Notify::const_new();
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
        let completed = sqlx::query_scalar(
            "SELECT date FROM attendance_calendar_snapshots WHERE academic_term_id=$1",
        )
        .bind(term)
        .fetch_all(&pool)
        .await?
        .into_iter()
        .collect();
        terms.push(TermSchedule {
            term,
            start,
            end,
            configuration: c.0,
            days,
            completed,
        });
    }
    let pending = notifications::has_pending(&pool).await?;
    let prior = tenants()
        .get(tenant)
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
            digests,
            last_retention,
        },
    );
    WAKE.notify_one();
    Ok(())
}
pub async fn start(state: AppState) {
    match state.admin_client.list_active_schools().await {
        Ok(schools) => {
            for school in schools {
                if let Some(url) = school.db_connection_string.filter(|s| !s.is_empty()) {
                    match state
                        .pool_manager
                        .get_pool_without_migrations(&url, &school.subdomain)
                        .await
                    {
                        Ok(pool) => {
                            if touch(&school.subdomain, pool, state.notification_channel.clone())
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
    let mut interval = tokio::time::interval(std::time::Duration::from_secs(15));
    loop {
        tokio::select! {_=interval.tick()=>{},_=WAKE.notified()=>{}}
        let work: Vec<_> = tenants()
            .iter()
            .map(|v| (v.key().clone(), v.value().clone()))
            .collect();
        for (tenant, mut r) in work {
            if run(&tenant, &mut r).await.is_err() {
                tracing::warn!(
                    error_code = "attendance_background_retry",
                    "Attendance background work will retry"
                );
            }
            if let Some(mut live) = tenants().get_mut(&tenant) {
                live.digests.extend(r.digests);
                live.last_retention = r.last_retention;
                for updated in &r.terms {
                    if let Some(current) = live.terms.iter_mut().find(|t| t.term == updated.term) {
                        current.completed.extend(&updated.completed);
                    }
                }
                if live.generation == r.generation {
                    live.pending = r.pending;
                    live.terms = r.terms;
                }
            }
        }
    }
}
async fn run(tenant: &str, r: &mut Registration) -> Result<(), AppError> {
    let now = Utc::now().with_timezone(&chrono_tz::Asia::Bangkok);
    let date = now.date_naive();
    // Backfill at most two completed school days per tick. Missing checks stay unchecked;
    // the report owner never writes facts, and idle complete calendars require no DB polling.
    let mut snapshot_budget = 2;
    for t in r.terms.clone() {
        let end = (date - chrono::Duration::days(1)).min(t.end);
        let mut candidate = t.start;
        while candidate <= end && snapshot_budget > 0 {
            let override_day = t
                .days
                .iter()
                .find(|d| d.date == candidate)
                .map(|d| d.counted);
            if !t.completed.contains(&candidate)
                && school_attendance::rules::counted(&t.configuration, candidate, override_day)
            {
                match snapshot_day(&r.pool, t.term, candidate).await {
                    Ok(()) => {
                        if let Some(live) = r.terms.iter_mut().find(|v| v.term == t.term) {
                            live.completed.insert(candidate);
                        }
                        snapshot_budget -= 1;
                    }
                    Err(AppError::Conflict(_)) => {
                        r.terms.retain(|v| v.term != t.term);
                        break;
                    }
                    Err(error) => return Err(error),
                }
            }
            let Some(next) = candidate.succ_opt() else {
                break;
            };
            candidate = next;
        }

        if date < t.start || date > t.end {
            continue;
        }
        let day = t.days.iter().find(|d| d.date == date).map(|d| d.counted);
        if !school_attendance::rules::counted(&t.configuration, date, day) {
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
                let seeds = super::services::seeds(&r.pool, t.term, date).await?;
                let mut tx = r.pool.begin().await?;
                school_attendance::services::lock_term(&mut tx, t.term, true, false).await?;
                for s in seeds
                    .into_iter()
                    .filter(|s| s.session.kind != AttendanceKind::Arrival)
                {
                    sessions::insert_seed(&mut tx, &s).await?;
                }
                tx.commit().await?;
                notifications::queue_digest(&r.pool, t.term, date, time).await?;
                r.pending = true;
                r.digests.insert(key);
            }
        }
    }
    if r.pending {
        for n in notifications::pending(&r.pool).await? {
            notifications::store(&r.pool, &n).await?;
            let notice:Notification=sqlx::query_as("SELECT id,title,message,type type_,link,read_at,created_at FROM notifications WHERE id=$1").bind(n.notification_id).fetch_one(&r.pool).await?;
            let publisher = TenantNotificationPublisher::new(tenant, &r.channel);
            if NotificationService::deliver_stored(&r.pool, &publisher, n.recipient_id, notice)
                .await
                .is_ok()
            {
                notifications::delivered(&r.pool, n.id).await?;
            }
        }
        r.pending = notifications::has_pending(&r.pool).await?;
    }
    if !r.terms.is_empty() && r.last_retention != Some(date) {
        faces::expire_evidence(&r.pool).await?;
        r.last_retention = Some(date);
    }
    Ok(())
}
fn digest_due(current: u32, due: u32, delivered: bool) -> bool {
    !delivered && current >= due && current <= due + 60
}
#[cfg(test)]
mod tests {
    use super::digest_due;
    #[test]
    fn digest_window_catches_up_once_and_does_not_send_early() {
        assert!(!digest_due(479, 480, false));
        assert!(digest_due(480, 480, false));
        assert!(digest_due(540, 480, false));
        assert!(!digest_due(541, 480, false));
        assert!(!digest_due(480, 480, true));
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
            return;
        }
    }
    refresh_after_commit(tenant, pool, channel).await;
}

pub(super) async fn snapshot_day(
    pool: &PgPool,
    term: Uuid,
    date: NaiveDate,
) -> Result<(), AppError> {
    let seeds = super::services::seeds(pool, term, date).await?;
    let mut tx = pool.begin().await?;
    school_attendance::services::lock_term(&mut tx, term, true, false).await?;
    let fresh:Option<NaiveDate>=sqlx::query_scalar("INSERT INTO attendance_calendar_snapshots(academic_term_id,date) VALUES($1,$2) ON CONFLICT DO NOTHING RETURNING date").bind(term).bind(date).fetch_optional(&mut *tx).await?;
    if fresh.is_some() {
        for seed in seeds
            .into_iter()
            .filter(|s| s.session.kind != AttendanceKind::Arrival)
        {
            sessions::insert_seed(&mut tx, &seed).await?;
        }
    }
    tx.commit().await?;
    Ok(())
}
