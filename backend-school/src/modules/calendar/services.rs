use chrono::NaiveDate;
#[cfg(test)]
use chrono::{NaiveTime, Utc};
use sqlx::pool::PoolConnection;
use sqlx::{PgPool, Postgres};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::db::{admin_client::AdminClient, pool_manager::PoolManager};
use crate::error::AppError;
#[cfg(test)]
use crate::modules::calendar::models::{
    CalendarAudienceType, CalendarEvent, CalendarEventQuery, CalendarEventTargetInput,
};
use crate::modules::notification::events::TenantNotificationEvent;

mod categories_and_tags;
mod events;
mod notifications;
mod shared;
mod visibility;

#[cfg(test)]
use categories_and_tags::normalized_tag_name;
pub use categories_and_tags::{
    create_category, create_tag, hard_delete_category, hard_delete_tag, list_categories, list_tags,
    update_category, update_tag,
};
#[allow(unused_imports)]
pub use events::{create_event, soft_delete_event, update_event, CalendarEventMutationOutcome};
#[cfg(test)]
use notifications::{calendar_notification_link_for_user_type, calendar_notification_text};
#[allow(unused_imports)]
pub use notifications::{
    resolve_event_recipient_user_ids, send_event_notification, CalendarNotificationKind,
    CalendarNotificationSendOutcome,
};
#[cfg(test)]
pub use shared::reminder_dates;
use shared::tenant_today;
#[allow(unused_imports)]
pub use shared::{dedupe_user_ids, validate_event_date_time, validate_targets};
#[cfg(test)]
use shared::{dedupe_uuid_ids, normalized_event_range, reminder_schedule};
use visibility::get_event_for_response;
#[cfg(test)]
use visibility::{
    calendar_search_pattern, self_calendar_user_type_access, target_visible_to_child_view,
    target_visible_to_user_type,
};
pub use visibility::{
    list_child_events, list_management_events, list_my_events, list_public_events,
};

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

fn calendar_reminder_advisory_lock_keys(reminder_id: Uuid) -> (i32, i32) {
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

fn select_due_calendar_reminder_candidates_sql() -> &'static str {
    SELECT_DUE_CALENDAR_REMINDER_CANDIDATES_SQL
}

fn refetch_due_calendar_reminder_after_lock_sql() -> &'static str {
    REFETCH_DUE_CALENDAR_REMINDER_AFTER_LOCK_SQL
}

fn mark_calendar_reminder_sent_sql() -> &'static str {
    MARK_CALENDAR_REMINDER_SENT_SQL
}

fn try_calendar_reminder_advisory_lock_sql() -> &'static str {
    TRY_CALENDAR_REMINDER_ADVISORY_LOCK_SQL
}

fn release_calendar_reminder_advisory_lock_sql() -> &'static str {
    RELEASE_CALENDAR_REMINDER_ADVISORY_LOCK_SQL
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_event_date_time_rejects_end_date_before_start() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 7, 9).unwrap();

        assert!(validate_event_date_time(start, end, true, None, None).is_err());
    }

    #[test]
    fn validate_event_date_time_rejects_same_day_end_time_before_start_time() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let start_time = NaiveTime::from_hms_opt(10, 0, 0).unwrap();
        let end_time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();

        assert!(
            validate_event_date_time(date, date, false, Some(start_time), Some(end_time)).is_err()
        );
    }

    #[test]
    fn validate_event_date_time_rejects_same_day_equal_start_and_end_time() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let time = NaiveTime::from_hms_opt(10, 0, 0).unwrap();

        assert!(validate_event_date_time(date, date, false, Some(time), Some(time)).is_err());
    }

    #[test]
    fn validate_event_date_time_accepts_multi_day_timed_event() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 7, 12).unwrap();
        let start_time = NaiveTime::from_hms_opt(15, 0, 0).unwrap();
        let end_time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();

        assert!(
            validate_event_date_time(start, end, false, Some(start_time), Some(end_time)).is_ok()
        );
    }

    #[test]
    fn reminder_dates_are_day_based_and_sorted() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let result = reminder_dates(start, &[1, 7, 3]).unwrap();

        assert_eq!(
            result,
            vec![
                NaiveDate::from_ymd_opt(2026, 7, 3).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 7).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 9).unwrap()
            ]
        );
    }

    #[test]
    fn reminder_dates_dedupes_duplicate_offsets() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let result = reminder_dates(start, &[3, 1, 3, 1]).unwrap();

        assert_eq!(
            result,
            vec![
                NaiveDate::from_ymd_opt(2026, 7, 7).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 9).unwrap()
            ]
        );
    }

    #[test]
    fn reminder_schedule_pairs_dedupe_offsets_and_sort_by_reminder_date() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let result = reminder_schedule(start, &[1, 7, 1, 3]).unwrap();

        assert_eq!(
            result,
            vec![
                (7, NaiveDate::from_ymd_opt(2026, 7, 3).unwrap()),
                (3, NaiveDate::from_ymd_opt(2026, 7, 7).unwrap()),
                (1, NaiveDate::from_ymd_opt(2026, 7, 9).unwrap()),
            ]
        );
    }

    #[test]
    fn reminder_schedule_pairs_reject_overflowing_offsets() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();

        assert!(reminder_schedule(start, &[i32::MAX]).is_err());
    }

    #[test]
    fn reminder_dates_reject_zero_or_negative_offsets() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();

        assert!(reminder_dates(start, &[0]).is_err());
        assert!(reminder_dates(start, &[-1]).is_err());
    }

    #[test]
    fn reminder_dates_rejects_offsets_outside_representable_date_range() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();

        assert!(reminder_dates(start, &[i32::MAX]).is_err());
    }

    #[test]
    fn validate_targets_rejects_empty_targets() {
        assert!(validate_targets(&[]).is_err());
    }

    #[test]
    fn validate_targets_accepts_student_grade_target() {
        let grade_level_id = Uuid::new_v4();
        let targets = vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::Student,
            grade_level_id: Some(grade_level_id),
            class_room_id: None,
        }];

        assert!(validate_targets(&targets).is_ok());
    }

    #[test]
    fn validate_targets_rejects_student_or_parent_with_grade_and_classroom_filter() {
        let targets = vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::Parent,
            grade_level_id: Some(Uuid::new_v4()),
            class_room_id: Some(Uuid::new_v4()),
        }];

        assert!(validate_targets(&targets).is_err());
    }

    #[test]
    fn validate_targets_rejects_all_with_grade_or_classroom_filter() {
        let grade_targets = vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::All,
            grade_level_id: Some(Uuid::new_v4()),
            class_room_id: None,
        }];
        let classroom_targets = vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::All,
            grade_level_id: None,
            class_room_id: Some(Uuid::new_v4()),
        }];

        assert!(validate_targets(&grade_targets).is_err());
        assert!(validate_targets(&classroom_targets).is_err());
    }

    #[test]
    fn target_visible_to_user_type_allows_all_and_matching_user_type_only() {
        assert!(target_visible_to_user_type("all", "staff"));
        assert!(target_visible_to_user_type("all", "student"));
        assert!(!target_visible_to_user_type("all", "parent"));
        assert!(!target_visible_to_user_type("parent", "parent"));
        assert!(!target_visible_to_user_type("student", "parent"));
        assert!(!target_visible_to_user_type("parent", "student"));
    }

    #[test]
    fn self_calendar_user_type_access_rejects_parent_users() {
        assert!(self_calendar_user_type_access("staff").is_ok());
        assert!(self_calendar_user_type_access("student").is_ok());
        assert!(matches!(
            self_calendar_user_type_access("parent"),
            Err(AppError::Forbidden(message))
                if message.contains("/api/parent/students/{student_id}/calendar/events")
        ));
    }

    #[test]
    fn target_visible_to_child_view_allows_all_and_parent_only() {
        assert!(target_visible_to_child_view("all"));
        assert!(target_visible_to_child_view("parent"));
        assert!(!target_visible_to_child_view("student"));
        assert!(!target_visible_to_child_view("staff"));
    }

    #[test]
    fn validate_targets_rejects_duplicate_global_targets() {
        let targets = vec![
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Student,
                grade_level_id: None,
                class_room_id: None,
            },
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Student,
                grade_level_id: None,
                class_room_id: None,
            },
        ];

        assert!(validate_targets(&targets).is_err());
    }

    #[test]
    fn validate_targets_rejects_duplicate_grade_targets() {
        let grade_level_id = Uuid::new_v4();
        let targets = vec![
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Parent,
                grade_level_id: Some(grade_level_id),
                class_room_id: None,
            },
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Parent,
                grade_level_id: Some(grade_level_id),
                class_room_id: None,
            },
        ];

        assert!(validate_targets(&targets).is_err());
    }

    #[test]
    fn validate_targets_rejects_duplicate_classroom_targets() {
        let class_room_id = Uuid::new_v4();
        let targets = vec![
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Student,
                grade_level_id: None,
                class_room_id: Some(class_room_id),
            },
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Student,
                grade_level_id: None,
                class_room_id: Some(class_room_id),
            },
        ];

        assert!(validate_targets(&targets).is_err());
    }

    #[test]
    fn validate_targets_rejects_staff_with_classroom_filter() {
        let targets = vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::Staff,
            grade_level_id: None,
            class_room_id: Some(Uuid::new_v4()),
        }];

        assert!(validate_targets(&targets).is_err());
    }

    #[test]
    fn dedupe_user_ids_preserves_first_seen_order() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        assert_eq!(dedupe_user_ids(vec![a, b, a]), vec![a, b]);
    }

    #[test]
    fn dedupe_uuid_ids_keeps_each_tag_once_in_selection_order() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();

        assert_eq!(
            dedupe_uuid_ids(&[first, second, first]),
            vec![first, second]
        );
    }

    #[test]
    fn normalized_tag_name_trims_and_rejects_blank_values() {
        assert_eq!(
            normalized_tag_name("  กิจกรรมเด่น  ".to_string()).unwrap(),
            "กิจกรรมเด่น"
        );
        assert!(normalized_tag_name("   ".to_string()).is_err());
    }

    #[test]
    fn normalized_event_range_uses_complete_query_range() {
        let from = NaiveDate::from_ymd_opt(2026, 5, 2).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let query = CalendarEventQuery {
            from: Some(from),
            to: Some(to),
            category_id: None,
            tag_id: None,
            audience: None,
            visibility: None,
            q: None,
        };

        assert_eq!(normalized_event_range(&query, today).unwrap(), (from, to));
    }

    #[test]
    fn normalized_event_range_rejects_reversed_complete_query_range() {
        let from = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 5, 2).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let query = CalendarEventQuery {
            from: Some(from),
            to: Some(to),
            category_id: None,
            tag_id: None,
            audience: None,
            visibility: None,
            q: None,
        };

        assert!(matches!(
            normalized_event_range(&query, today),
            Err(AppError::BadRequest(_))
        ));
    }

    #[test]
    fn normalized_event_range_defaults_to_current_month_when_bound_is_missing() {
        let today = NaiveDate::from_ymd_opt(2026, 2, 10).unwrap();
        let query = CalendarEventQuery {
            from: Some(NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()),
            to: None,
            category_id: None,
            tag_id: None,
            audience: None,
            visibility: None,
            q: None,
        };

        assert_eq!(
            normalized_event_range(&query, today).unwrap(),
            (
                NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 2, 28).unwrap(),
            )
        );
    }

    #[test]
    fn normalized_event_range_handles_december_current_month() {
        let today = NaiveDate::from_ymd_opt(2026, 12, 15).unwrap();
        let query = CalendarEventQuery {
            from: None,
            to: None,
            category_id: None,
            tag_id: None,
            audience: None,
            visibility: None,
            q: None,
        };

        assert_eq!(
            normalized_event_range(&query, today).unwrap(),
            (
                NaiveDate::from_ymd_opt(2026, 12, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            )
        );
    }

    #[test]
    fn calendar_search_pattern_escapes_like_wildcards() {
        assert_eq!(
            calendar_search_pattern(" 100%_activity\\plan "),
            "%100\\%\\_activity\\\\plan%"
        );
    }

    #[test]
    fn calendar_notification_text_formats_created_updated_and_reminder_messages() {
        let event = calendar_event_for_notification("สอบกลางภาค");

        assert_eq!(
            calendar_notification_text(&event, &CalendarNotificationKind::Created),
            (
                "เพิ่มกำหนดการ: สอบกลางภาค".to_string(),
                "มีกำหนดการใหม่ในปฏิทินโรงเรียน".to_string(),
            )
        );
        assert_eq!(
            calendar_notification_text(&event, &CalendarNotificationKind::Updated),
            (
                "อัปเดตกำหนดการ: สอบกลางภาค".to_string(),
                "มีการเปลี่ยนแปลงกำหนดการในปฏิทินโรงเรียน".to_string(),
            )
        );
        assert_eq!(
            calendar_notification_text(
                &event,
                &CalendarNotificationKind::Reminder { days_before: 3 },
            ),
            (
                "เตือนล่วงหน้า: สอบกลางภาค".to_string(),
                "กำหนดการนี้จะเริ่มในอีก 3 วัน".to_string(),
            )
        );
    }

    #[test]
    fn calendar_notification_link_matches_supported_user_types() {
        assert_eq!(
            calendar_notification_link_for_user_type("staff"),
            Some("/staff/calendar")
        );
        assert_eq!(
            calendar_notification_link_for_user_type("student"),
            Some("/student/calendar")
        );
        assert_eq!(
            calendar_notification_link_for_user_type("parent"),
            Some("/parent")
        );
        assert_eq!(calendar_notification_link_for_user_type("guest"), None);
    }

    #[test]
    fn reminder_advisory_lock_keys_are_stable_from_uuid_bytes() {
        let reminder_id = Uuid::parse_str("01020304-0506-0708-090a-0b0c0d0e0f10").unwrap();

        assert_eq!(
            calendar_reminder_advisory_lock_keys(reminder_id),
            (16_909_060, 84_281_096)
        );
    }

    #[test]
    fn due_reminder_candidate_query_does_not_lock_or_mark_sent() {
        let sql = select_due_calendar_reminder_candidates_sql();

        assert!(sql.contains("LIMIT 200"));
        assert!(sql.contains("sent_at IS NULL"));
        assert!(sql.contains("$2::uuid[]"));
        assert!(!sql.contains("FOR UPDATE"));
        assert!(!sql.contains("SET sent_at"));
    }

    #[test]
    fn due_reminder_candidate_query_excludes_attempted_ids_for_batching() {
        let sql = select_due_calendar_reminder_candidates_sql();

        assert!(sql.contains("NOT (id = ANY($2::uuid[]))"));
        assert!(sql.contains("LIMIT 200"));
    }

    #[test]
    fn due_reminder_mark_query_sets_sent_after_attempt() {
        let sql = mark_calendar_reminder_sent_sql();

        assert!(sql.contains("UPDATE calendar_event_reminders"));
        assert!(sql.contains("SET sent_at = NOW()"));
        assert!(sql.contains("WHERE id = $1 AND sent_at IS NULL"));
    }

    #[test]
    fn notification_outcome_marks_reminders_sent_only_when_none_or_some_success() {
        assert!(CalendarNotificationSendOutcome {
            recipient_count: 0,
            successful_count: 0,
            failed_count: 0,
        }
        .should_mark_reminder_sent());
        assert!(CalendarNotificationSendOutcome {
            recipient_count: 2,
            successful_count: 1,
            failed_count: 1,
        }
        .should_mark_reminder_sent());
        assert!(!CalendarNotificationSendOutcome {
            recipient_count: 2,
            successful_count: 0,
            failed_count: 2,
        }
        .should_mark_reminder_sent());
    }

    #[test]
    fn advisory_lock_queries_use_two_integer_keys() {
        assert!(try_calendar_reminder_advisory_lock_sql().contains("pg_try_advisory_lock($1, $2)"));
        assert!(
            release_calendar_reminder_advisory_lock_sql().contains("pg_advisory_unlock($1, $2)")
        );
    }

    fn calendar_event_for_notification(title: &str) -> CalendarEvent {
        let now = Utc::now();
        let date = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();

        CalendarEvent {
            id: Uuid::new_v4(),
            category_id: None,
            category_name: None,
            category_color: None,
            title: title.to_string(),
            description: None,
            location: None,
            start_date: date,
            end_date: date,
            all_day: true,
            start_time: None,
            end_time: None,
            is_public: false,
            tags: Vec::new(),
            targets: Vec::new(),
            reminders: Vec::new(),
            created_by: None,
            updated_by: None,
            created_at: now,
            updated_at: now,
        }
    }
}
