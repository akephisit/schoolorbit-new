use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, NaiveDate, Utc};

// ==========================================
// Admission Round Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionRound {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub grade_level_id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub apply_start_date: NaiveDate,
    pub apply_end_date: NaiveDate,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exam_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result_announce_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enrollment_start_date: Option<NaiveDate>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enrollment_end_date: Option<NaiveDate>,
    pub status: String,
    pub is_visible: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub report_config: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub selection_settings: Option<serde_json::Value>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,

    // Joined fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub academic_year_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grade_level_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_count: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAdmissionRoundRequest {
    pub academic_year_id: Uuid,
    pub grade_level_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub apply_start_date: NaiveDate,
    pub apply_end_date: NaiveDate,
    pub exam_date: Option<NaiveDate>,
    pub result_announce_date: Option<NaiveDate>,
    pub enrollment_start_date: Option<NaiveDate>,
    pub enrollment_end_date: Option<NaiveDate>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAdmissionRoundRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub apply_start_date: Option<NaiveDate>,
    pub apply_end_date: Option<NaiveDate>,
    pub exam_date: Option<NaiveDate>,
    pub result_announce_date: Option<NaiveDate>,
    pub enrollment_start_date: Option<NaiveDate>,
    pub enrollment_end_date: Option<NaiveDate>,
    pub report_config: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSelectionSettingsRequest {
    pub subjects_by_track: Option<serde_json::Value>,
    pub method_by_track: Option<serde_json::Value>,
    pub room_assignment_method: Option<String>,
    /// "per_track" (default) หรือ "global"
    pub assignment_mode: Option<String>,
    /// แสดงคะแนนบน portal ผู้สมัคร
    pub show_scores: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRoundStatusRequest {
    pub status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRoundVisibilityRequest {
    pub is_visible: bool,
}

// ==========================================
// Exam Subject Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionExamSubject {
    pub id: Uuid,
    pub admission_round_id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    pub max_score: f64,
    pub display_order: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateExamSubjectRequest {
    pub name: String,
    pub code: Option<String>,
    pub max_score: Option<f64>,
    pub display_order: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateExamSubjectRequest {
    pub name: Option<String>,
    pub code: Option<String>,
    pub max_score: Option<f64>,
    pub display_order: Option<i32>,
}

// ==========================================
// Admission Track Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionTrack {
    pub id: Uuid,
    pub admission_round_id: Uuid,
    pub study_plan_id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacity_override: Option<i32>,
    pub scoring_subject_ids: serde_json::Value, // JSONB array of UUID strings
    pub tiebreak_method: String,
    pub display_order: i32,
    pub created_at: DateTime<Utc>,

    // Joined/computed fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub study_plan_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed_capacity: Option<i64>,  // ดึงจาก class_rooms ถ้า capacity_override = NULL
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_count: Option<i64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_count: Option<i64>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAdmissionTrackRequest {
    pub study_plan_id: Uuid,
    pub name: String,
    pub capacity_override: Option<i32>,
    pub scoring_subject_ids: Option<Vec<Uuid>>,
    pub tiebreak_method: Option<String>,
    pub display_order: Option<i32>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAdmissionTrackRequest {
    pub name: Option<String>,
    pub capacity_override: Option<i32>,
    pub scoring_subject_ids: Option<Vec<Uuid>>,
    pub tiebreak_method: Option<String>,
    pub display_order: Option<i32>,
}
