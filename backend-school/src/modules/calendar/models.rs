use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CalendarAudienceType {
    All,
    Staff,
    Student,
    Parent,
}

impl CalendarAudienceType {
    pub fn as_str(&self) -> &'static str {
        match self {
            CalendarAudienceType::All => "all",
            CalendarAudienceType::Staff => "staff",
            CalendarAudienceType::Student => "student",
            CalendarAudienceType::Parent => "parent",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CalendarVisibility {
    Public,
    Private,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CalendarCategory {
    pub id: Uuid,
    pub name: String,
    pub color: String,
    pub order_index: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCalendarCategoryRequest {
    pub name: String,
    pub color: String,
    pub order_index: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventTarget {
    pub id: Uuid,
    pub audience_type: String,
    pub grade_level_id: Option<Uuid>,
    pub class_room_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventTargetInput {
    pub audience_type: CalendarAudienceType,
    pub grade_level_id: Option<Uuid>,
    pub class_room_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventReminder {
    pub id: Uuid,
    pub days_before: i32,
    pub remind_on: NaiveDate,
    pub sent_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEvent {
    pub id: Uuid,
    pub category_id: Option<Uuid>,
    pub category_name: Option<String>,
    pub category_color: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub all_day: bool,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    pub is_public: bool,
    pub targets: Vec<CalendarEventTarget>,
    pub reminders: Vec<CalendarEventReminder>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct CalendarEventRow {
    pub id: Uuid,
    pub category_id: Option<Uuid>,
    pub category_name: Option<String>,
    pub category_color: Option<String>,
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub all_day: bool,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    pub is_public: bool,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventQuery {
    pub from: Option<NaiveDate>,
    pub to: Option<NaiveDate>,
    #[serde(alias = "category_id")]
    pub category_id: Option<Uuid>,
    pub audience: Option<CalendarAudienceType>,
    pub visibility: Option<CalendarVisibility>,
    pub q: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCalendarEventRequest {
    pub title: String,
    pub description: Option<String>,
    pub location: Option<String>,
    pub category_id: Option<Uuid>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub all_day: bool,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    pub is_public: bool,
    pub targets: Vec<CalendarEventTargetInput>,
    pub reminder_offsets_days: Vec<i32>,
    pub notify_audience: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn calendar_event_query_accepts_snake_case_category_id() {
        let category_id = Uuid::new_v4();

        let query: CalendarEventQuery =
            serde_json::from_value(json!({ "category_id": category_id.to_string() })).unwrap();

        assert_eq!(query.category_id, Some(category_id));
    }

    #[test]
    fn calendar_event_query_accepts_public_private_visibility_values() {
        let public_query: CalendarEventQuery =
            serde_json::from_value(json!({ "visibility": "public" })).unwrap();
        let private_query: CalendarEventQuery =
            serde_json::from_value(json!({ "visibility": "private" })).unwrap();

        assert_eq!(public_query.visibility, Some(CalendarVisibility::Public));
        assert_eq!(private_query.visibility, Some(CalendarVisibility::Private));
    }

    #[test]
    fn calendar_event_query_rejects_unknown_visibility_value() {
        let result = serde_json::from_value::<CalendarEventQuery>(json!({ "visibility": "all" }));

        assert!(result.is_err());
    }
}
