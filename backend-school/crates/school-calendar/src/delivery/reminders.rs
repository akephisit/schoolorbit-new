use std::sync::Arc;

use chrono::{DateTime, NaiveDate, Utc};
use futures::{stream::FuturesUnordered, StreamExt};
use sqlx::{Connection, PgConnection, PgPool};
use std::time::Duration;
use std::{collections::VecDeque, future::Future};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::services::{
    get_event_for_response_on_connection, tenant_today, CalendarNotificationKind,
};
use school_errors::AppError;
use school_notifications::events::TenantNotificationEvent;
use school_tenancy::{
    background::{TenantJobKind, TenantTurnExecutor},
    AdminClient, PoolManager,
};

use super::notifications::accept_event_in_transaction;

const SELECT_DUE_CALENDAR_REMINDER_CANDIDATES_SQL: &str = r#"
SELECT id, event_id, days_before, remind_on, created_at
FROM calendar_event_reminders
WHERE remind_on <= $1
  AND sent_at IS NULL
  AND ($2::date IS NULL OR (remind_on, created_at, id) > ($2, $3, $4))
ORDER BY remind_on ASC, created_at ASC, id ASC
LIMIT 200
"#;

const REFETCH_DUE_CALENDAR_REMINDER_AFTER_LOCK_SQL: &str = r#"
SELECT id, event_id, days_before, remind_on, created_at
FROM calendar_event_reminders
WHERE id = $1 AND sent_at IS NULL
"#;

const MARK_CALENDAR_REMINDER_SENT_SQL: &str = r#"
UPDATE calendar_event_reminders
SET sent_at = NOW()
WHERE id = $1 AND sent_at IS NULL
"#;

const TRY_CALENDAR_REMINDER_ADVISORY_LOCK_SQL: &str = "SELECT pg_try_advisory_lock($1, $2)";
const RELEASE_CALENDAR_REMINDER_ADVISORY_LOCK_SQL: &str = "SELECT pg_advisory_unlock($1, $2)";

#[derive(sqlx::FromRow)]
struct DueCalendarEventReminderRow {
    id: Uuid,
    event_id: Uuid,
    days_before: i32,
    remind_on: NaiveDate,
    created_at: DateTime<Utc>,
}

#[derive(Clone, Copy)]
struct ReminderCursor {
    remind_on: NaiveDate,
    created_at: DateTime<Utc>,
    id: Uuid,
}

struct ReminderTurn {
    marked_sent: i64,
    cursor: Option<ReminderCursor>,
    has_more: bool,
}

const REMINDER_TURN_SOFT_BUDGET: Duration = Duration::from_secs(10);

/// One bounded turn. The all-tenant orchestrator requeues unfinished tenants;
/// this standalone provider never drains an unbounded backlog.
pub async fn process_due_reminders(
    pool: &PgPool,
    notification_channel: &broadcast::Sender<TenantNotificationEvent>,
    tenant: &str,
    tenant_current_date: NaiveDate,
) -> Result<i64, AppError> {
    Ok(process_due_reminder_turn(
        pool,
        notification_channel,
        tenant,
        tenant_current_date,
        None,
    )
    .await?
    .marked_sent)
}

async fn process_due_reminder_turn(
    pool: &PgPool,
    notification_channel: &broadcast::Sender<TenantNotificationEvent>,
    tenant: &str,
    tenant_current_date: NaiveDate,
    after: Option<ReminderCursor>,
) -> Result<ReminderTurn, AppError> {
    let started = tokio::time::Instant::now();
    let candidates = fetch_due_reminder_candidates(pool, tenant_current_date, after).await?;
    let candidate_count = candidates.len();
    let mut result = ReminderTurn {
        marked_sent: 0,
        cursor: after,
        has_more: false,
    };
    for (index, candidate) in candidates.into_iter().enumerate() {
        result.cursor = Some(ReminderCursor {
            remind_on: candidate.remind_on,
            created_at: candidate.created_at,
            id: candidate.id,
        });
        if process_due_reminder_candidate(pool, notification_channel, tenant, candidate).await? {
            result.marked_sent += 1;
        }
        // Soft budget only: never cancel a recipient fan-out after a partial
        // committed acceptance. Failed/contended rows are passed until next run.
        if started.elapsed() >= REMINDER_TURN_SOFT_BUDGET {
            result.has_more = index + 1 < candidate_count || candidate_count == 200;
            return Ok(result);
        }
    }
    result.has_more = candidate_count == 200;
    Ok(result)
}

async fn fetch_due_reminder_candidates(
    pool: &PgPool,
    tenant_current_date: NaiveDate,
    after: Option<ReminderCursor>,
) -> Result<Vec<DueCalendarEventReminderRow>, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "calendar.reminder_candidates")
            .await?;
    Ok(school_database::DbSession::new(&mut *connection)
        .query_as::<DueCalendarEventReminderRow>(
            "calendar.delivery.reminders.fetch_due_reminder_candidates",
            select_due_calendar_reminder_candidates_sql(),
        )
        .bind(tenant_current_date)
        .bind(after.map(|cursor| cursor.remind_on))
        .bind(after.map(|cursor| cursor.created_at))
        .bind(after.map(|cursor| cursor.id))
        .fetch_all()
        .await?)
}

async fn process_due_reminder_candidate(
    pool: &PgPool,
    notification_channel: &broadcast::Sender<TenantNotificationEvent>,
    tenant: &str,
    candidate: DueCalendarEventReminderRow,
) -> Result<bool, AppError> {
    let lock_keys = calendar_reminder_advisory_lock_keys(candidate.id);
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "calendar.reminder").await?;
    // Cancellation can happen while PostgreSQL accepts the lock but before the
    // driver receives its result. Such a session must never return to the pool.
    connection.close_on_drop();

    let acquired = school_database::DbSession::new(&mut *connection)
        .scalar::<bool>(
            "calendar.delivery.reminders.process_due_reminder_candidate",
            try_calendar_reminder_advisory_lock_sql(),
        )
        .bind(lock_keys.0)
        .bind(lock_keys.1)
        .fetch_one()
        .await?;

    if !acquired {
        connection.return_to_pool().await;
        return Ok(false);
    }

    let processing_result = process_advisory_locked_reminder(
        notification_channel,
        tenant,
        &mut connection,
        candidate.id,
    )
    .await;
    let release_result =
        release_calendar_reminder_advisory_lock(&mut connection, candidate.id, lock_keys).await;

    if let Err(error) = release_result {
        tracing::error!(
            reminder_id = %candidate.id,
            error = %error,
            "Failed to release calendar reminder advisory lock"
        );
        if processing_result.is_ok() {
            return Err(error);
        }
    } else {
        // Only a confirmed unlock permits reusing the session. This method
        // takes its live connection before awaiting and remains cancel-safe.
        connection.return_to_pool().await;
    }

    processing_result
}

async fn process_advisory_locked_reminder(
    notification_channel: &broadcast::Sender<TenantNotificationEvent>,
    tenant: &str,
    connection: &mut PgConnection,
    reminder_id: Uuid,
) -> Result<bool, AppError> {
    // One reminder's full recipient snapshot, inbox acceptance and sent_at commit atomically.
    let mut tx = connection.begin().await?;
    let reminder = school_database::DbSession::new(&mut *tx)
        .query_as::<DueCalendarEventReminderRow>(
            "calendar.reminder.refetch",
            refetch_due_calendar_reminder_after_lock_sql(),
        )
        .bind(reminder_id)
        .fetch_optional()
        .await?;
    let Some(reminder) = reminder else {
        tx.commit().await?;
        return Ok(false);
    };
    school_database::DbSession::new(&mut *tx)
        .query(
            "calendar.reminder.lock_event",
            "SELECT id FROM calendar_events WHERE id=$1 FOR SHARE",
        )
        .bind(reminder.event_id)
        .execute()
        .await?;
    let current: Option<Uuid> = school_database::DbSession::new(&mut *tx)
        .scalar(
            "calendar.reminder.lock_source",
            "SELECT id FROM calendar_event_reminders WHERE id=$1 AND sent_at IS NULL FOR UPDATE",
        )
        .bind(reminder.id)
        .fetch_optional()
        .await?;
    if current.is_none() {
        tx.commit().await?;
        return Ok(false);
    }
    let event = match get_event_for_response_on_connection(&mut tx, reminder.event_id).await {
        Ok(event) => Some(event),
        Err(AppError::NotFound(_)) => None,
        Err(error) => return Err(error),
    };
    if let Some(event) = event {
        accept_event_in_transaction(
            &mut tx,
            &event,
            &CalendarNotificationKind::Reminder {
                days_before: reminder.days_before,
            },
            Some(reminder.id),
        )
        .await?;
    }
    let marked = mark_calendar_reminder_sent(&mut tx, reminder.id).await?;
    tx.commit().await?;
    school_notifications::runtime::wake_after_commit(tenant);
    let _ = notification_channel;
    Ok(marked)
}

async fn mark_calendar_reminder_sent(
    connection: &mut PgConnection,
    reminder_id: Uuid,
) -> Result<bool, AppError> {
    let result = school_database::DbSession::new(&mut *connection)
        .query(
            "calendar.delivery.reminders.mark_calendar_reminder_sent",
            mark_calendar_reminder_sent_sql(),
        )
        .bind(reminder_id)
        .execute()
        .await?;

    Ok(result.rows_affected() > 0)
}

async fn release_calendar_reminder_advisory_lock(
    connection: &mut PgConnection,
    reminder_id: Uuid,
    lock_keys: (i32, i32),
) -> Result<(), AppError> {
    let released = school_database::DbSession::new(&mut *connection)
        .scalar::<bool>(
            "calendar.delivery.reminders.release_calendar_reminder_advisory_lock",
            release_calendar_reminder_advisory_lock_sql(),
        )
        .bind(lock_keys.0)
        .bind(lock_keys.1)
        .fetch_one()
        .await?;

    if !released {
        tracing::warn!(
            reminder_id = %reminder_id,
            "Calendar reminder advisory lock was not held during release"
        );
        return Err(AppError::ServiceUnavailable(
            "calendar_reminder_unlock_unconfirmed".into(),
        ));
    }

    Ok(())
}

pub(super) fn calendar_reminder_advisory_lock_keys(reminder_id: Uuid) -> (i32, i32) {
    let bytes = reminder_id.as_bytes();
    (
        i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        i32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
    )
}

pub async fn process_due_calendar_reminders_for_all_tenants(
    admin_client: Arc<AdminClient>,
    pool_manager: Arc<PoolManager>,
    notification_channel: broadcast::Sender<TenantNotificationEvent>,
    turns: Arc<TenantTurnExecutor>,
) {
    let tenant_current_date = tenant_today();
    let schools = match admin_client.list_active_schools().await {
        Ok(schools) => schools,
        Err(error) => {
            tracing::error!(%error, "Failed to fetch schools for calendar reminders");
            return;
        }
    };
    process_tenant_queue(
        schools.into_iter().map(|school| (school, None)).collect(),
        turns.limit(),
        |(school, cursor)| {
            let turns = &turns;
            let pools = &pool_manager;
            let notifications = &notification_channel;
            async move {
                let result = turns
                    .run(
                        &school.subdomain,
                        TenantJobKind::CalendarReminders,
                        || async {
                            let Some(url) = school
                                .db_connection_string
                                .as_ref()
                                .filter(|url| !url.is_empty())
                            else {
                                return None;
                            };
                            let pool = match pools
                                .get_pool_without_migrations(url, &school.subdomain)
                                .await
                            {
                                Ok(pool) => pool,
                                Err(error) => {
                                    tracing::error!(%error, "Failed to open calendar tenant pool");
                                    return None;
                                }
                            };
                            match process_due_reminder_turn(
                                &pool,
                                notifications,
                                &school.subdomain,
                                tenant_current_date,
                                cursor,
                            )
                            .await
                            {
                                Ok(turn) => Some(turn),
                                Err(error) => {
                                    tracing::error!(%error, "Calendar reminder turn failed");
                                    None
                                }
                            }
                        },
                    )
                    .await
                    .flatten();
                match result {
                    Some(turn) if turn.has_more => Some((school, turn.cursor)),
                    _ => None,
                }
            }
        },
    )
    .await;
}

async fn process_tenant_queue<T, F, Fut>(mut pending: VecDeque<T>, limit: usize, turn: F)
where
    F: Fn(T) -> Fut,
    Fut: Future<Output = Option<T>>,
{
    let mut running = FuturesUnordered::new();
    loop {
        while running.len() < limit {
            let Some(tenant) = pending.pop_front() else {
                break;
            };
            running.push(turn(tenant));
        }
        let Some(remaining) = running.next().await else {
            break;
        };
        if let Some(remaining) = remaining {
            pending.push_back(remaining);
        }
    }
}

pub(super) fn select_due_calendar_reminder_candidates_sql() -> &'static str {
    SELECT_DUE_CALENDAR_REMINDER_CANDIDATES_SQL
}

fn refetch_due_calendar_reminder_after_lock_sql() -> &'static str {
    REFETCH_DUE_CALENDAR_REMINDER_AFTER_LOCK_SQL
}

pub(super) fn mark_calendar_reminder_sent_sql() -> &'static str {
    MARK_CALENDAR_REMINDER_SENT_SQL
}

pub(super) fn try_calendar_reminder_advisory_lock_sql() -> &'static str {
    TRY_CALENDAR_REMINDER_ADVISORY_LOCK_SQL
}

pub(super) fn release_calendar_reminder_advisory_lock_sql() -> &'static str {
    RELEASE_CALENDAR_REMINDER_ADVISORY_LOCK_SQL
}

#[cfg(test)]
mod turn_tests {
    use super::*;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    };

    async fn reminder_fixture(name: &str, count: i32) -> PgPool {
        let pool = school_test_db::create_named_test_pool_with_max_connections(name, 5).await;
        school_test_db::run_test_migrations(&pool).await;
        let event = Uuid::new_v4();
        sqlx::query("INSERT INTO calendar_events(id,title,start_date,end_date,deleted_at) VALUES($1,'synthetic reminder',CURRENT_DATE,CURRENT_DATE,now())")
            .bind(event).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO calendar_event_reminders(id,event_id,days_before,remind_on,created_at) SELECT gen_random_uuid(),$1,n,CURRENT_DATE,'2026-01-01 00:00:00+00'::timestamptz FROM generate_series(1,$2) n")
            .bind(event).bind(count).execute(&pool).await.unwrap();
        pool
    }

    #[tokio::test]
    async fn one_connection_reminder_accepts_all_recipients_and_releases_advisory_lock() {
        let pool = school_test_db::create_named_test_pool_with_max_connections(
            "calendar_one_connection",
            1,
        )
        .await;
        school_test_db::run_test_migrations(&pool).await;
        let event = Uuid::new_v4();
        sqlx::query("INSERT INTO calendar_events(id,title,start_date,end_date) VALUES($1,'synthetic',CURRENT_DATE,CURRENT_DATE)")
            .bind(event).execute(&pool).await.unwrap();
        sqlx::query(
            "INSERT INTO calendar_event_targets(event_id,audience_type) VALUES($1,'staff')",
        )
        .bind(event)
        .execute(&pool)
        .await
        .unwrap();
        let recipients = [Uuid::new_v4(), Uuid::new_v4()];
        for recipient in recipients {
            sqlx::query("INSERT INTO users(id,password_hash,first_name,last_name,user_type,status) VALUES($1,'test-only','Synthetic','Recipient','staff','active')")
                .bind(recipient).execute(&pool).await.unwrap();
        }
        let reminder = Uuid::new_v4();
        sqlx::query("INSERT INTO calendar_event_reminders(id,event_id,days_before,remind_on) VALUES($1,$2,1,CURRENT_DATE)")
            .bind(reminder).bind(event).execute(&pool).await.unwrap();
        let (notifications, _) = broadcast::channel(8);
        let marked = tokio::time::timeout(
            Duration::from_secs(3),
            process_due_reminders(&pool, &notifications, "synthetic", tenant_today()),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(marked, 1);
        let stored: i64 =
            sqlx::query_scalar("SELECT count(*) FROM notifications WHERE user_id=ANY($1)")
                .bind(recipients.as_slice())
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(stored, 2);
        let jobs: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM notification_receipts WHERE recipient_id=ANY($1)",
        )
        .bind(recipients.as_slice())
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(jobs, 2);
        let keys = calendar_reminder_advisory_lock_keys(reminder);
        let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1,$2)")
            .bind(keys.0)
            .bind(keys.1)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert!(acquired);
        sqlx::query("SELECT pg_advisory_unlock_all()")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;
    }

    #[tokio::test]
    async fn reminder_keyset_passes_contended_prefix_and_retries_it_next_invocation() {
        let pool = reminder_fixture("calendar_turn_keyset", 401).await;
        let today = tenant_today();
        let initial = fetch_due_reminder_candidates(&pool, today, None)
            .await
            .unwrap();
        assert_eq!(initial.len(), 200);
        let mut locks = pool.acquire().await.unwrap();
        locks.close_on_drop();
        for candidate in &initial {
            let keys = calendar_reminder_advisory_lock_keys(candidate.id);
            sqlx::query("SELECT pg_advisory_lock($1,$2)")
                .bind(keys.0)
                .bind(keys.1)
                .execute(&mut *locks)
                .await
                .unwrap();
        }
        let (notifications, _) = broadcast::channel(8);
        let first = process_due_reminder_turn(&pool, &notifications, "synthetic", today, None)
            .await
            .unwrap();
        assert_eq!(first.marked_sent, 0);
        assert!(first.has_more);
        let second =
            process_due_reminder_turn(&pool, &notifications, "synthetic", today, first.cursor)
                .await
                .unwrap();
        assert_eq!(second.marked_sent, 200);
        assert!(second.has_more);
        let third =
            process_due_reminder_turn(&pool, &notifications, "synthetic", today, second.cursor)
                .await
                .unwrap();
        assert_eq!(third.marked_sent, 1);
        assert!(!third.has_more);
        let pending: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM calendar_event_reminders WHERE sent_at IS NULL",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(pending, 200);
        sqlx::query("SELECT pg_advisory_unlock_all()")
            .execute(&mut *locks)
            .await
            .unwrap();
        locks.return_to_pool().await;
        let retry = process_due_reminder_turn(&pool, &notifications, "synthetic", today, None)
            .await
            .unwrap();
        assert_eq!(retry.marked_sent, 200);
        pool.close().await;
    }

    #[tokio::test]
    async fn cancelling_locked_reminder_closes_session_before_pool_reuse() {
        let pool = reminder_fixture("calendar_turn_cancel", 1).await;
        let today = tenant_today();
        let candidate = fetch_due_reminder_candidates(&pool, today, None)
            .await
            .unwrap()
            .remove(0);
        let keys = calendar_reminder_advisory_lock_keys(candidate.id);
        let mut blocked_event = pool.begin().await.unwrap();
        sqlx::query("LOCK TABLE calendar_events IN ACCESS EXCLUSIVE MODE")
            .execute(&mut *blocked_event)
            .await
            .unwrap();
        let worker_pool = pool.clone();
        let (notifications, _) = broadcast::channel(8);
        let task = tokio::spawn(async move {
            process_due_reminders(&worker_pool, &notifications, "synthetic", today).await
        });
        let mut observer = pool.acquire().await.unwrap();
        observer.close_on_drop();
        // Observe the actual reminder session lock before dropping its future.
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let locked: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_locks WHERE locktype='advisory' AND classid=$1::bigint::oid AND objid=$2::bigint::oid AND objsubid=2 AND granted)")
                    .bind(i64::from(keys.0 as u32))
                    .bind(i64::from(keys.1 as u32))
                    .fetch_one(&mut *observer).await.unwrap();
                if locked { break; }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let acquired: bool = sqlx::query_scalar("SELECT pg_try_advisory_lock($1,$2)")
                    .bind(keys.0)
                    .bind(keys.1)
                    .fetch_one(&mut *observer)
                    .await
                    .unwrap();
                if acquired {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        sqlx::query("SELECT pg_advisory_unlock($1,$2)")
            .bind(keys.0)
            .bind(keys.1)
            .execute(&mut *observer)
            .await
            .unwrap();
        observer.return_to_pool().await;
        blocked_event.rollback().await.unwrap();
        let (notifications, _) = broadcast::channel(8);
        assert_eq!(
            process_due_reminders(&pool, &notifications, "synthetic", today)
                .await
                .unwrap(),
            1
        );
        pool.close().await;
    }

    #[tokio::test]
    async fn stalled_tenant_does_not_block_healthy_backlog_revisits() {
        let visited = Arc::new(Mutex::new(Vec::new()));
        let running = Arc::new(AtomicUsize::new(0));
        let maximum = Arc::new(AtomicUsize::new(0));
        let (finished_tx, finished_rx) = tokio::sync::oneshot::channel();
        let finished_tx = Arc::new(Mutex::new(Some(finished_tx)));
        let release = Arc::new(tokio::sync::Notify::new());
        let worker_release = release.clone();
        let worker_visited = visited.clone();
        let worker_maximum = maximum.clone();
        let task = tokio::spawn(async move {
            process_tenant_queue(
                VecDeque::from([("slow", 1), ("backlog", 3), ("small", 1)]),
                2,
                |(tenant, remaining)| {
                    let release = worker_release.clone();
                    let visited = worker_visited.clone();
                    let running = running.clone();
                    let maximum = worker_maximum.clone();
                    let finished_tx = finished_tx.clone();
                    async move {
                        let active = running.fetch_add(1, Ordering::SeqCst) + 1;
                        maximum.fetch_max(active, Ordering::SeqCst);
                        visited.lock().unwrap().push((tenant, remaining));
                        if tenant == "slow" {
                            release.notified().await;
                        }
                        if tenant == "backlog" && remaining == 1 {
                            finished_tx
                                .lock()
                                .unwrap()
                                .take()
                                .unwrap()
                                .send(())
                                .unwrap();
                        }
                        running.fetch_sub(1, Ordering::SeqCst);
                        (remaining > 1).then_some((tenant, remaining - 1))
                    }
                },
            )
            .await;
        });
        tokio::time::timeout(Duration::from_secs(1), finished_rx)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            *visited.lock().unwrap(),
            [
                ("slow", 1),
                ("backlog", 3),
                ("small", 1),
                ("backlog", 2),
                ("backlog", 1)
            ]
        );
        assert_eq!(maximum.load(Ordering::SeqCst), 2);
        release.notify_one();
        tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .unwrap()
            .unwrap();
    }
}
