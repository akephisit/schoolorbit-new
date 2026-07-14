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
pub struct CalendarTag {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CalendarEventTag {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpsertCalendarTagRequest {
    pub name: String,
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
    pub tags: Vec<CalendarEventTag>,
    pub targets: Vec<CalendarEventTarget>,
    pub reminders: Vec<CalendarEventReminder>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarPublicEvent {
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
    pub tags: Vec<CalendarEventTag>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarViewerEvent {
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
    pub tags: Vec<CalendarEventTag>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl From<CalendarEvent> for CalendarViewerEvent {
    fn from(event: CalendarEvent) -> Self {
        Self {
            id: event.id,
            category_id: event.category_id,
            category_name: event.category_name,
            category_color: event.category_color,
            title: event.title,
            description: event.description,
            location: event.location,
            start_date: event.start_date,
            end_date: event.end_date,
            all_day: event.all_day,
            start_time: event.start_time,
            end_time: event.end_time,
            is_public: event.is_public,
            tags: event.tags,
            created_at: event.created_at,
            updated_at: event.updated_at,
        }
    }
}

impl From<CalendarEvent> for CalendarPublicEvent {
    fn from(event: CalendarEvent) -> Self {
        Self {
            id: event.id,
            category_id: event.category_id,
            category_name: event.category_name,
            category_color: event.category_color,
            title: event.title,
            description: event.description,
            location: event.location,
            start_date: event.start_date,
            end_date: event.end_date,
            all_day: event.all_day,
            start_time: event.start_time,
            end_time: event.end_time,
            is_public: event.is_public,
            tags: event.tags,
            created_at: event.created_at,
            updated_at: event.updated_at,
        }
    }
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
    #[serde(alias = "tag_id")]
    pub tag_id: Option<Uuid>,
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
    #[serde(default)]
    pub tag_ids: Vec<Uuid>,
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
    fn calendar_event_query_accepts_snake_case_tag_id() {
        let tag_id = Uuid::new_v4();

        let query: CalendarEventQuery =
            serde_json::from_value(json!({ "tag_id": tag_id.to_string() })).unwrap();

        assert_eq!(query.tag_id, Some(tag_id));
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

    #[test]
    fn public_event_serialization_excludes_internal_fields() {
        let category_id = Uuid::new_v4();
        let category_id_text = category_id.to_string();
        let event = CalendarPublicEvent {
            id: Uuid::new_v4(),
            category_id: Some(category_id),
            category_name: Some("Activities".to_string()),
            category_color: Some("#2563eb".to_string()),
            title: "Open House".to_string(),
            description: Some("Public event description".to_string()),
            location: Some("Main hall".to_string()),
            start_date: NaiveDate::from_ymd_opt(2026, 7, 10).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2026, 7, 10).unwrap(),
            all_day: true,
            start_time: None,
            end_time: None,
            is_public: true,
            tags: vec![CalendarEventTag {
                id: Uuid::new_v4(),
                name: "Featured".to_string(),
            }],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let value = serde_json::to_value(event).unwrap();

        assert!(value.get("id").is_some());
        assert_eq!(
            value.get("categoryId").and_then(|value| value.as_str()),
            Some(category_id_text.as_str())
        );
        assert!(value.get("targets").is_none());
        assert_eq!(value["tags"][0]["name"], "Featured");
        assert!(value.get("reminders").is_none());
        assert!(value.get("createdBy").is_none());
        assert!(value.get("updatedBy").is_none());
    }

    #[test]
    fn viewer_event_serialization_excludes_management_fields() {
        let event = CalendarViewerEvent {
            id: Uuid::new_v4(),
            category_id: None,
            category_name: Some("Exam".to_string()),
            category_color: Some("#2563eb".to_string()),
            title: "Midterm".to_string(),
            description: None,
            location: Some("Building 1".to_string()),
            start_date: NaiveDate::from_ymd_opt(2026, 7, 15).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2026, 7, 15).unwrap(),
            all_day: true,
            start_time: None,
            end_time: None,
            is_public: false,
            tags: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        let value = serde_json::to_value(event).unwrap();

        assert!(value.get("title").is_some());
        assert!(value.get("targets").is_none());
        assert!(value.get("reminders").is_none());
        assert!(value.get("createdBy").is_none());
        assert!(value.get("updatedBy").is_none());
    }
}
