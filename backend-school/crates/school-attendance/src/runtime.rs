//! Process-owned schedule cache: idle tenants are not polled every minute.
use school_database::DbSession;
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
use school_errors::AppError;
use school_notifications::events::TenantNotificationEvent;
use school_tenancy::background::TenantJobKind;
use sqlx::{PgPool, types::Json};
use std::{
    collections::{HashMap, HashSet},
    sync::OnceLock,
};
use tokio::sync::broadcast;
use uuid::Uuid;
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
    terms: Vec<TermSchedule>,
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
    _channel: broadcast::Sender<TenantNotificationEvent>,
) -> Result<(), AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(&pool, "attendance.runtime.touch")
            .await?;
    let rows:Vec<(Uuid,NaiveDate,NaiveDate,Json<AttendanceConfiguration>)>=DbSession::new(&mut *connection).query_as("attendance.src.runtime.touch", "SELECT s.academic_term_id,calendar.starts_on,calendar.ends_on,s.configuration FROM attendance_settings s JOIN academic_terms t ON t.id=s.academic_term_id JOIN academic_years y ON y.id=t.academic_year_id JOIN (SELECT academic_term_id,min(date) AS starts_on,max(date) AS ends_on FROM attendance_days GROUP BY academic_term_id) calendar ON calendar.academic_term_id=t.id WHERE y.status NOT IN ('closed','archived') AND t.status NOT IN ('closed','cancelled') AND NOT EXISTS(SELECT 1 FROM attendance_term_archives a WHERE a.academic_term_id=t.id)").fetch_all().await?;
    let ids = rows.iter().map(|row| row.0).collect::<Vec<_>>();
    let day_rows: Vec<(Uuid,NaiveDate,bool,String)> = DbSession::new(&mut *connection).query_as("attendance.src.runtime.touch", "SELECT academic_term_id,date,counted,note FROM attendance_days WHERE academic_term_id=ANY($1) ORDER BY academic_term_id,date")
        .bind(&ids).fetch_all().await?;
    let mut days_by_term = HashMap::<Uuid, Vec<AttendanceDay>>::new();
    for (term, date, counted, note) in day_rows {
        days_by_term.entry(term).or_default().push(AttendanceDay {
            date,
            counted,
            note,
        });
    }
    let terms = rows
        .into_iter()
        .map(|(term, start, end, c)| TermSchedule {
            term,
            start,
            end,
            configuration: c.0,
            days: days_by_term.remove(&term).unwrap_or_default(),
        })
        .collect();
    let module_generation = DbSession::new(&mut *connection)
        .scalar(
            "attendance.src.runtime.touch",
            "SELECT generation FROM attendance_module_state WHERE singleton",
        )
        .fetch_one()
        .await?;
    drop(connection);
    let prior = tenants()
        .get(tenant)
        .filter(|v| v.module_generation == module_generation)
        .map(|v| (v.digests.clone(), v.last_retention));
    let (digests, last_retention) = prior.unwrap_or_default();
    tenants().insert(
        tenant.into(),
        Registration {
            terms,
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
                    let executor = state.pool_manager.background_turns();
                    let result = executor
                        .run(
                            &school.subdomain,
                            TenantJobKind::AttendanceMaintenance,
                            || async {
                                let pool = state
                                    .pool_manager
                                    .get_pool_without_migrations(&url, &school.subdomain)
                                    .await
                                    .map_err(|_| {
                                        AppError::ServiceUnavailable(
                                            "Attendance tenant unavailable".into(),
                                        )
                                    })?;
                                touch(
                                    &school.subdomain,
                                    pool.as_ref().clone(),
                                    state.notification_channel.clone(),
                                )
                                .await
                            },
                        )
                        .await;
                    if matches!(result, Some(Err(_))) {
                        tracing::warn!(
                            error_code = "attendance_schedule_unavailable",
                            "Attendance startup schedule could not load"
                        );
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
            let executor = state.pool_manager.background_turns();
            let result = executor
                .run(&tenant, TenantJobKind::AttendanceMaintenance, || async {
                    let operation_pool = state
                        .pool_manager
                        .get_pool(&school.database_url, &tenant)
                        .await
                        .map_err(|_| {
                            AppError::ServiceUnavailable("Attendance tenant unavailable".into())
                        })?;
                    run(&tenant, &mut r, &operation_pool, state.behavior).await
                })
                .await;
            if result.is_none() {
                continue;
            }
            if matches!(result, Some(Err(_))) {
                tracing::warn!(
                    error_code = "attendance_background_retry",
                    "Attendance background work will retry"
                );
            }
            if let Some(mut live) = tenants().get_mut(&tenant) {
                if live.generation == r.generation {
                    live.digests.extend(r.digests);
                    live.last_retention = r.last_retention;
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
    pool: &PgPool,
    behavior: &dyn AttendanceBehaviorPort,
) -> Result<(), AppError> {
    let now = Utc::now().with_timezone(&chrono_tz::Asia::Bangkok);
    let date = now.date_naive();
    if r.last_finalization_poll != Some(date) {
        let processed = crate::application::finalization::catch_up(pool, Utc::now(), 4).await?;
        if processed > 0 {
            school_notifications::runtime::wake_after_commit(tenant);
        }
        if processed < 4 {
            r.last_finalization_poll = Some(date);
        }
    }
    if behavior.run(pool).await.is_err() {
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
                let mut tx = school_tenancy::TenantPool::begin_observed(
                    pool,
                    "attendance.runtime.transaction",
                )
                .await?;
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
                school_notifications::runtime::wake_after_commit(tenant);
                r.digests.insert(key);
            }
        }
    }
    if r.last_retention != Some(date) {
        if faces::expire_evidence(pool).await? < faces::EVIDENCE_EXPIRY_BATCH {
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
    school_notifications::runtime::wake_after_commit(subdomain);
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
    school_notifications::runtime::wake_after_commit(tenant);
    {
        if let Some(mut entry) = tenants().get_mut(tenant) {
            WAKE.notify_one();
            entry.generation = Uuid::new_v4();
            entry.last_finalization_poll = None;
            return;
        }
    }
    refresh_after_commit(tenant, pool, channel).await;
}
