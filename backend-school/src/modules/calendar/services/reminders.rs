use std::sync::Arc;

use chrono::NaiveDate;
use sqlx::pool::PoolConnection;
use sqlx::{PgPool, Postgres};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::db::{admin_client::AdminClient, pool_manager::PoolManager};
use crate::error::AppError;
use crate::modules::notification::events::TenantNotificationEvent;

use super::notifications::{send_event_notification, CalendarNotificationKind};
use super::shared::tenant_today;
use super::visibility::get_event_for_response;

const SELECT_DUE_CALENDAR_REMINDER_CANDIDATES_SQL: &str = r#"
SELECT id, event_id, days_before
FROM calendar_event_reminders
WHERE remind_on <= $1
  AND sent_at IS NULL
  AND NOT (id = ANY($2::uuid[]))
ORDER BY remind_on ASC, created_at ASC
LIMIT 200
"#;

const REFETCH_DUE_CALENDAR_REMINDER_AFTER_LOCK_SQL: &str = r#"
SELECT id, event_id, days_before
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
}

pub async fn process_due_reminders(
    pool: &PgPool,
    notification_channel: &broadcast::Sender<TenantNotificationEvent>,
    tenant: &str,
    tenant_current_date: NaiveDate,
) -> Result<i64, AppError> {
    let mut marked_sent_count = 0;
    let mut attempted_ids = Vec::new();

    loop {
        let candidates =
            fetch_due_reminder_candidates(pool, tenant_current_date, &attempted_ids).await?;
        if candidates.is_empty() {
            break;
        }

        for candidate in candidates {
            attempted_ids.push(candidate.id);
            if process_due_reminder_candidate(pool, notification_channel, tenant, candidate).await?
            {
                marked_sent_count += 1;
            }
        }
    }

    Ok(marked_sent_count)
}

async fn fetch_due_reminder_candidates(
    pool: &PgPool,
    tenant_current_date: NaiveDate,
    attempted_ids: &[Uuid],
) -> Result<Vec<DueCalendarEventReminderRow>, AppError> {
    Ok(sqlx::query_as::<_, DueCalendarEventReminderRow>(
        select_due_calendar_reminder_candidates_sql(),
    )
    .bind(tenant_current_date)
    .bind(attempted_ids)
    .fetch_all(pool)
    .await?)
}

async fn process_due_reminder_candidate(
    pool: &PgPool,
    notification_channel: &broadcast::Sender<TenantNotificationEvent>,
    tenant: &str,
    candidate: DueCalendarEventReminderRow,
) -> Result<bool, AppError> {
    let lock_keys = calendar_reminder_advisory_lock_keys(candidate.id);
    let mut connection = pool.acquire().await?;

    let acquired = sqlx::query_scalar::<_, bool>(try_calendar_reminder_advisory_lock_sql())
        .bind(lock_keys.0)
        .bind(lock_keys.1)
        .fetch_one(&mut *connection)
        .await?;

    if !acquired {
        return Ok(false);
    }

    let processing_result = process_advisory_locked_reminder(
        pool,
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
    }

    processing_result
}

async fn process_advisory_locked_reminder(
    pool: &PgPool,
    notification_channel: &broadcast::Sender<TenantNotificationEvent>,
    tenant: &str,
    connection: &mut PoolConnection<Postgres>,
    reminder_id: Uuid,
) -> Result<bool, AppError> {
    // The session-level advisory lock is held on this connection only for one reminder.
    // No row transaction is kept open while event loading and notification fan-out run.
    let reminder = sqlx::query_as::<_, DueCalendarEventReminderRow>(
        refetch_due_calendar_reminder_after_lock_sql(),
    )
    .bind(reminder_id)
    .fetch_optional(&mut **connection)
    .await?;

    let Some(reminder) = reminder else {
        return Ok(false);
    };

    let event = match get_event_for_response(pool, reminder.event_id).await {
        Ok(event) => event,
        Err(AppError::NotFound(_)) => {
            tracing::warn!(
                reminder_id = %reminder.id,
                event_id = %reminder.event_id,
                "Skipping calendar reminder for missing or deleted event"
            );
            return mark_calendar_reminder_sent(connection, reminder.id).await;
        }
        Err(error) => {
            tracing::error!(
                reminder_id = %reminder.id,
                event_id = %reminder.event_id,
                error = %error,
                "Failed to load calendar event for reminder"
            );
            return Ok(false);
        }
    };

    let send_outcome = match send_event_notification(
        pool,
        notification_channel,
        tenant,
        &event,
        CalendarNotificationKind::Reminder {
            days_before: reminder.days_before,
        },
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(error) => {
            tracing::error!(
                reminder_id = %reminder.id,
                event_id = %reminder.event_id,
                error = %error,
                "Calendar reminder notification failed"
            );
            return Ok(false);
        }
    };

    if !send_outcome.should_mark_reminder_sent() {
        tracing::warn!(
            reminder_id = %reminder.id,
            event_id = %reminder.event_id,
            recipient_count = send_outcome.recipient_count,
            failed_count = send_outcome.failed_count,
            "Calendar reminder notification had no successful sends; leaving reminder pending"
        );
        return Ok(false);
    }

    mark_calendar_reminder_sent(connection, reminder.id).await
}

async fn mark_calendar_reminder_sent(
    connection: &mut PoolConnection<Postgres>,
    reminder_id: Uuid,
) -> Result<bool, AppError> {
    let result = sqlx::query(mark_calendar_reminder_sent_sql())
        .bind(reminder_id)
        .execute(&mut **connection)
        .await?;

    Ok(result.rows_affected() > 0)
}

async fn release_calendar_reminder_advisory_lock(
    connection: &mut PoolConnection<Postgres>,
    reminder_id: Uuid,
    lock_keys: (i32, i32),
) -> Result<(), AppError> {
    let released = sqlx::query_scalar::<_, bool>(release_calendar_reminder_advisory_lock_sql())
        .bind(lock_keys.0)
        .bind(lock_keys.1)
        .fetch_one(&mut **connection)
        .await?;

    if !released {
        tracing::warn!(
            reminder_id = %reminder_id,
            "Calendar reminder advisory lock was not held during release"
        );
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
) {
    let tenant_current_date = tenant_today();

    let schools = match admin_client.list_active_schools().await {
        Ok(schools) => schools,
        Err(error) => {
            tracing::error!("Failed to fetch schools for calendar reminders: {}", error);
            return;
        }
    };

    for school in schools {
        let Some(db_url) = school
            .db_connection_string
            .filter(|value| !value.is_empty())
        else {
            tracing::warn!(
                "Skipping calendar reminders for {}: no database URL",
                school.subdomain
            );
            continue;
        };

        match pool_manager.get_pool(&db_url, &school.subdomain).await {
            Ok(pool) => {
                if let Err(error) = process_due_reminders(
                    &pool,
                    &notification_channel,
                    &school.subdomain,
                    tenant_current_date,
                )
                .await
                {
                    tracing::error!(
                        "Calendar reminder processing failed for {}: {}",
                        school.subdomain,
                        error
                    );
                }
            }
            Err(error) => {
                tracing::error!(
                    "Failed to open tenant pool for calendar reminders {}: {}",
                    school.subdomain,
                    error
                );
            }
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
