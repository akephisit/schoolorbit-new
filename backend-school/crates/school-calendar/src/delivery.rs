pub(crate) mod notifications;
pub use notifications::authorize_reminder_on_connection;
mod reminders;
mod request_workflow;
pub use request_workflow::CalendarRequestWorkflow;

#[cfg(test)]
mod tests;

pub use crate::services::create_event;
pub use crate::services::{
    create_category, create_tag, hard_delete_category, hard_delete_tag, list_categories,
    list_management_events, list_my_events, list_public_events, list_tags, soft_delete_event,
    update_category, update_event, update_tag,
};
#[cfg(test)]
use chrono::{NaiveDate, Utc};
pub use notifications::resolve_event_recipient_user_ids;
pub use notifications::send_event_notification;
#[cfg(test)]
use notifications::{calendar_notification_link_for_user_type, calendar_notification_text};
pub use reminders::process_due_calendar_reminders_for_all_tenants;
pub use reminders::process_due_reminders;
#[cfg(test)]
use reminders::{
    calendar_reminder_advisory_lock_keys, mark_calendar_reminder_sent_sql,
    release_calendar_reminder_advisory_lock_sql, select_due_calendar_reminder_candidates_sql,
    try_calendar_reminder_advisory_lock_sql,
};
#[cfg(test)]
use uuid::Uuid;

#[cfg(test)]
use crate::models::CalendarEvent;
#[cfg(test)]
use crate::services::CalendarNotificationKind;
