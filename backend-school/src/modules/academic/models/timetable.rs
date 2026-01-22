use chrono::NaiveTime;
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ============================================
// Academic Periods (คาบเวลา)
// ============================================

#[derive(Debug, Serialize, Deserialize, FromRow, Clone)]
pub struct AcademicPeriod {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub name: String,
    pub start_time: NaiveTime,
    pub end_time: NaiveTime,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub period_type: String, // TEACHING, BREAK, ACTIVITY, HOMEROOM
    pub order_index: i32,
    pub applicable_days: Option<String>, // "MON,TUE,WED" or null
    pub is_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePeriodRequest {
    pub academic_year_id: Uuid,
    pub name: String,
    pub start_time: String,  // Format: "HH:MM"
    pub end_time: String,    // Format: "HH:MM"
    #[serde(rename = "type")]
    pub period_type: String,
    pub order_index: i32,
    pub applicable_days: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePeriodRequest {
    pub name: Option<String>,
    pub start_time: Option<String>,
    pub end_time: Option<String>,
    #[serde(rename = "type")]
    pub period_type: Option<String>,
    pub order_index: Option<i32>,
    pub applicable_days: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct PeriodQuery {
    pub academic_year_id: Option<Uuid>,
    pub period_type: Option<String>,
    pub active_only: Option<bool>,
}

// ============================================
// Timetable Entries (ตารางสอน)
// ============================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct TimetableEntry {
    pub id: Uuid,
    pub classroom_course_id: Uuid,
    pub day_of_week: String, // MON, TUE, WED, THU, FRI
    pub period_id: Uuid,
    pub room_id: Option<Uuid>,
    pub note: Option<String>,
    pub is_active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<chrono::DateTime<chrono::Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_by: Option<Uuid>,
    pub updated_by: Option<Uuid>,
    
    // Joined fields (for display)
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_code: Option<String>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_name_th: Option<String>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructor_name: Option<String>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub classroom_name: Option<String>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_code: Option<String>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period_name: Option<String>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_time: Option<NaiveTime>,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end_time: Option<NaiveTime>,
}

#[derive(Debug, Deserialize)]
pub struct CreateTimetableEntryRequest {
    pub classroom_course_id: Uuid,
    pub day_of_week: String,
    pub period_id: Uuid,
    pub room_id: Option<Uuid>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateTimetableEntryRequest {
    pub day_of_week: Option<String>,
    pub period_id: Option<Uuid>,
    pub room_id: Option<Uuid>,
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TimetableQuery {
    pub classroom_id: Option<Uuid>,
    pub instructor_id: Option<Uuid>,
    pub room_id: Option<Uuid>,
    pub academic_semester_id: Option<Uuid>,
    pub day_of_week: Option<String>,
}

// ============================================
// Conflict Detection Response
// ============================================

#[derive(Debug, Serialize)]
pub struct ConflictInfo {
    pub conflict_type: String, // "INSTRUCTOR_CONFLICT", "ROOM_CONFLICT", "CLASSROOM_CONFLICT"
    pub message: String,
    pub existing_entry: Option<TimetableEntry>,
}

#[derive(Debug, Serialize)]
pub struct TimetableValidationResponse {
    pub is_valid: bool,
    pub conflicts: Vec<ConflictInfo>,
}
