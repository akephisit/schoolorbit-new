use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

// ==================== Instructor Preferences ====================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct InstructorPreference {
    pub id: Uuid,
    pub instructor_id: Uuid,
    pub academic_year_id: Uuid,
    
    // Unavailable time slots (HARD constraint)
    #[sqlx(default)]
    pub hard_unavailable_slots: serde_json::Value, // Array of {day, period_id}
    
    // Preferred time slots (SOFT constraint)
    #[sqlx(default)]
    pub preferred_slots: serde_json::Value, // Array of {day, period_id}
    
    // Daily load preferences
    pub max_periods_per_day: Option<i32>,
    pub min_periods_per_day: Option<i32>,
    
    // Day preferences
    #[sqlx(default)]
    pub preferred_days: serde_json::Value, // Array of day strings
    
    #[sqlx(default)]
    pub avoid_days: serde_json::Value, // Array of day strings
    
    pub notes: Option<String>,
    
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimeSlot {
    pub day: String, // "MON", "TUE", etc.
    pub period_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct CreateInstructorPreferenceRequest {
    pub instructor_id: Uuid,
    pub academic_year_id: Uuid,
    pub hard_unavailable_slots: Option<Vec<TimeSlot>>,
    pub preferred_slots: Option<Vec<TimeSlot>>,
    pub max_periods_per_day: Option<i32>,
    pub min_periods_per_day: Option<i32>,
    pub preferred_days: Option<Vec<String>>,
    pub avoid_days: Option<Vec<String>>,
    pub notes: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateInstructorPreferenceRequest {
    pub hard_unavailable_slots: Option<Vec<TimeSlot>>,
    pub preferred_slots: Option<Vec<TimeSlot>>,
    pub max_periods_per_day: Option<i32>,
    pub min_periods_per_day: Option<i32>,
    pub preferred_days: Option<Vec<String>>,
    pub avoid_days: Option<Vec<String>>,
    pub notes: Option<String>,
}

// ==================== Instructor Room Assignments ====================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct InstructorRoomAssignment {
    pub id: Uuid,
    pub instructor_id: Uuid,
    pub room_id: Uuid,
    pub academic_year_id: Uuid,
    
    pub is_preferred: Option<bool>,
    pub is_required: Option<bool>,
    
    #[sqlx(default)]
    pub for_subjects: serde_json::Value, // Array of subject codes or empty
    
    pub reason: Option<String>,
    
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInstructorRoomAssignmentRequest {
    pub instructor_id: Uuid,
    pub room_id: Uuid,
    pub academic_year_id: Uuid,
    pub is_preferred: Option<bool>,
    pub is_required: Option<bool>,
    pub for_subjects: Option<Vec<String>>, // Subject codes
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateInstructorRoomAssignmentRequest {
    pub is_preferred: Option<bool>,
    pub is_required: Option<bool>,
    pub for_subjects: Option<Vec<String>>,
    pub reason: Option<String>,
}

// ==================== Timetable Locked Slots ====================

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum LockedSlotScope {
    Classroom,
    GradeLevel,
    AllSchool,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct TimetableLockedSlot {
    pub id: Uuid,
    pub academic_semester_id: Uuid,
    
    pub scope_type: String, // Will be converted to/from LockedSlotScope
    
    #[sqlx(default)]
    pub scope_ids: serde_json::Value, // Array of UUIDs or null
    
    pub subject_id: Uuid,
    pub day_of_week: String,
    
    #[sqlx(default)]
    pub period_ids: serde_json::Value, // Array of period UUIDs
    
    pub room_id: Option<Uuid>,
    pub instructor_id: Option<Uuid>,
    
    pub reason: Option<String>,
    pub locked_by: Option<Uuid>,
    
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateLockedSlotRequest {
    pub academic_semester_id: Uuid,
    pub scope_type: LockedSlotScope,
    pub scope_ids: Option<Vec<Uuid>>, // null for ALL_SCHOOL
    pub subject_id: Uuid,
    pub day_of_week: String,
    pub period_ids: Vec<Uuid>,
    pub room_id: Option<Uuid>,
    pub instructor_id: Option<Uuid>,
    pub reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateLockedSlotRequest {
    pub scope_type: Option<LockedSlotScope>,
    pub scope_ids: Option<Vec<Uuid>>,
    pub day_of_week: Option<String>,
    pub period_ids: Option<Vec<Uuid>>,
    pub room_id: Option<Uuid>,
    pub instructor_id: Option<Uuid>,
    pub reason: Option<String>,
}

// ==================== Timetable Scheduling Jobs ====================

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SchedulingStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SchedulingAlgorithm {
    Greedy,
    Backtracking,
    Hybrid,
}

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct TimetableSchedulingJob {
    pub id: Uuid,
    pub academic_semester_id: Uuid,
    
    #[sqlx(default)]
    pub classroom_ids: serde_json::Value, // Array of classroom UUIDs
    
    pub algorithm: String, // Will be converted to/from SchedulingAlgorithm
    
    #[sqlx(default)]
    pub config: serde_json::Value, // Scheduling configuration
    
    pub status: String, // Will be converted to/from SchedulingStatus
    pub progress: Option<i32>, // 0-100
    
    #[sqlx(default)]
    pub quality_score: Option<f32>,
    pub scheduled_courses: Option<i32>,
    pub total_courses: Option<i32>,
    
    #[sqlx(default)]
    pub failed_courses: serde_json::Value, // Array of failed course info
    
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_seconds: Option<i32>,
    
    pub error_message: Option<String>,
    
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SchedulingConfig {
    pub force_overwrite: Option<bool>,
    pub respect_preferences: Option<bool>,
    pub allow_partial: Option<bool>,
    pub min_quality_score: Option<f64>,
    pub timeout_seconds: Option<u32>,
    
    // Soft constraint weights (optional overrides)
    pub weight_distribution: Option<f64>,
    pub weight_consecutive: Option<f64>,
    pub weight_time_of_day: Option<f64>,
    pub weight_instructor_preference: Option<f64>,
    pub weight_daily_load: Option<f64>,
}

impl Default for SchedulingConfig {
    fn default() -> Self {
        Self {
            force_overwrite: Some(false),
            respect_preferences: Some(true),
            allow_partial: Some(false),
            min_quality_score: Some(70.0),
            timeout_seconds: Some(300),
            weight_distribution: None,
            weight_consecutive: None,
            weight_time_of_day: None,
            weight_instructor_preference: None,
            weight_daily_load: None,
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct CreateSchedulingJobRequest {
    pub academic_semester_id: Uuid,
    pub classroom_ids: Vec<Uuid>,
    pub algorithm: Option<SchedulingAlgorithm>,
    pub config: Option<SchedulingConfig>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct FailedCourseInfo {
    pub subject_code: String,
    pub subject_name: Option<String>,
    pub classroom: String,
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct SchedulingJobResponse {
    pub id: Uuid,
    pub academic_semester_id: Uuid,
    pub classroom_ids: Vec<Uuid>,
    pub algorithm: SchedulingAlgorithm,
    pub status: SchedulingStatus,
    pub progress: i32,
    pub quality_score: Option<f64>,
    pub scheduled_courses: i32,
    pub total_courses: i32,
    pub failed_courses: Vec<FailedCourseInfo>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub duration_seconds: Option<i32>,
    pub error_message: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}
