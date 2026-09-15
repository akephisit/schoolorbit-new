mod notifications;
mod reminders;

#[cfg(test)]
mod tests;

#[cfg(test)]
use chrono::{NaiveDate, Utc};
#[cfg(test)]
pub use notifications::resolve_event_recipient_user_ids;
pub use notifications::send_event_notification;
#[cfg(test)]
use notifications::CalendarNotificationSendOutcome;
#[cfg(test)]
use notifications::{calendar_notification_link_for_user_type, calendar_notification_text};
pub use reminders::process_due_calendar_reminders_for_all_tenants;
#[cfg(test)]
pub use reminders::process_due_reminders;
#[cfg(test)]
use reminders::{
    calendar_reminder_advisory_lock_keys, mark_calendar_reminder_sent_sql,
    release_calendar_reminder_advisory_lock_sql, select_due_calendar_reminder_candidates_sql,
    try_calendar_reminder_advisory_lock_sql,
};
pub use school_calendar::services::create_event;
pub use school_calendar::services::{
    create_category, create_tag, hard_delete_category, hard_delete_tag, list_categories,
    list_management_events, list_my_events, list_public_events, list_tags, soft_delete_event,
    update_category, update_event, update_tag,
};
#[cfg(test)]
use uuid::Uuid;

#[cfg(test)]
use school_calendar::models::CalendarEvent;
#[cfg(test)]
use school_calendar::services::CalendarNotificationKind;
