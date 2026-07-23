mod categories_and_tags;
mod events;
mod notifications;
mod reminders;
mod shared;
mod visibility;

#[cfg(test)]
mod tests;

pub use categories_and_tags::{
    create_category, create_tag, hard_delete_category, hard_delete_tag, list_categories, list_tags,
    update_category, update_tag,
};
#[allow(unused_imports)]
pub use events::{create_event, soft_delete_event, update_event, CalendarEventMutationOutcome};
#[allow(unused_imports)]
pub use notifications::{
    resolve_event_recipient_user_ids, send_event_notification, CalendarNotificationKind,
    CalendarNotificationSendOutcome,
};
#[allow(unused_imports)]
pub use reminders::{process_due_calendar_reminders_for_all_tenants, process_due_reminders};
#[cfg(test)]
pub use shared::reminder_dates;
#[allow(unused_imports)]
pub use shared::{dedupe_user_ids, validate_event_date_time, validate_targets};
pub use visibility::{
    list_child_events, list_management_events, list_my_events, list_public_events,
};

#[cfg(test)]
use categories_and_tags::normalized_tag_name;
#[cfg(test)]
use chrono::{NaiveDate, NaiveTime, Utc};
#[cfg(test)]
use notifications::{calendar_notification_link_for_user_type, calendar_notification_text};
#[cfg(test)]
use reminders::{
    calendar_reminder_advisory_lock_keys, mark_calendar_reminder_sent_sql,
    release_calendar_reminder_advisory_lock_sql, select_due_calendar_reminder_candidates_sql,
    try_calendar_reminder_advisory_lock_sql,
};
#[cfg(test)]
use shared::{dedupe_uuid_ids, normalized_event_range, reminder_schedule};
#[cfg(test)]
use uuid::Uuid;
#[cfg(test)]
use visibility::{
    calendar_search_pattern, self_calendar_user_type_access, target_visible_to_child_view,
    target_visible_to_user_type,
};

#[cfg(test)]
use crate::error::AppError;
#[cfg(test)]
use crate::modules::calendar::models::{
    CalendarAudienceType, CalendarEvent, CalendarEventQuery, CalendarEventTargetInput,
};
