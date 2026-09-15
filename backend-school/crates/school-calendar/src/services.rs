mod categories_and_tags;
mod events;
mod shared;
mod visibility;

#[cfg(test)]
mod tests;

/// Notification intent emitted by a calendar mutation and delivered by the application adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalendarNotificationKind {
    Created,
    Updated,
    Reminder { days_before: i32 },
}

pub use categories_and_tags::{
    create_category, create_tag, hard_delete_category, hard_delete_tag, list_categories, list_tags,
    update_category, update_tag,
};
pub use events::{create_event, soft_delete_event, update_event, CalendarEventMutationOutcome};
pub use shared::{dedupe_user_ids, tenant_today};
pub use visibility::{
    get_event_for_response, list_child_events, list_management_events, list_my_events,
    list_public_events,
};

#[cfg(test)]
use categories_and_tags::normalized_tag_name;
#[cfg(test)]
use chrono::{NaiveDate, NaiveTime};
#[cfg(test)]
use school_errors::AppError;
#[cfg(test)]
use shared::{
    dedupe_uuid_ids, normalized_event_range, reminder_dates, reminder_schedule,
    validate_event_date_time, validate_targets,
};
#[cfg(test)]
use uuid::Uuid;
#[cfg(test)]
use visibility::{
    calendar_search_pattern, self_calendar_user_type_access, target_visible_to_child_view,
    target_visible_to_user_type,
};

#[cfg(test)]
use crate::models::{CalendarAudienceType, CalendarEventQuery, CalendarEventTargetInput};
