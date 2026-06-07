use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::types::Json;
use sqlx::FromRow;
use std::collections::BTreeMap;
use uuid::Uuid;

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
    pub selection_settings: Option<Json<SelectionSettings>>,
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
    pub subjects_by_track: Option<BTreeMap<Uuid, Vec<Uuid>>>,
    pub method_by_track: Option<BTreeMap<Uuid, String>>,
    pub room_assignment_method: Option<String>,
    /// "per_track" (default) หรือ "global"
    pub assignment_mode: Option<String>,
    /// แสดงคะแนนบน portal ผู้สมัคร
    pub show_scores: Option<bool>,
}

fn default_room_assignment_method() -> String {
    "sequential".to_string()
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SelectionSettings {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub subjects_by_track: BTreeMap<Uuid, Vec<Uuid>>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub method_by_track: BTreeMap<Uuid, String>,
    #[serde(default = "default_room_assignment_method")]
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignment_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_scores: Option<bool>,
}

impl Default for SelectionSettings {
    fn default() -> Self {
        Self {
            subjects_by_track: BTreeMap::new(),
            method_by_track: BTreeMap::new(),
            method: default_room_assignment_method(),
            assignment_mode: None,
            show_scores: None,
        }
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct SelectionSettingsPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subjects_by_track: Option<BTreeMap<Uuid, Vec<Uuid>>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub method_by_track: Option<BTreeMap<Uuid, String>>,
    #[serde(rename = "method", skip_serializing_if = "Option::is_none")]
    pub room_assignment_method: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assignment_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_scores: Option<bool>,
}

impl SelectionSettingsPatch {
    pub fn is_empty(&self) -> bool {
        self.subjects_by_track.is_none()
            && self.method_by_track.is_none()
            && self.room_assignment_method.is_none()
            && self.assignment_mode.is_none()
            && self.show_scores.is_none()
    }
}

impl From<UpdateSelectionSettingsRequest> for SelectionSettingsPatch {
    fn from(request: UpdateSelectionSettingsRequest) -> Self {
        Self {
            subjects_by_track: request.subjects_by_track,
            method_by_track: request.method_by_track,
            room_assignment_method: request.room_assignment_method,
            assignment_mode: request.assignment_mode,
            show_scores: request.show_scores,
        }
    }
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

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdmissionTrack {
    pub id: Uuid,
    pub admission_round_id: Uuid,
    pub study_plan_id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capacity_override: Option<i32>,
    pub scoring_subject_ids: Vec<Uuid>,
    pub tiebreak_method: String,
    pub display_order: i32,
    pub created_at: DateTime<Utc>,

    // Joined/computed fields
    #[serde(skip_serializing_if = "Option::is_none")]
    pub study_plan_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub computed_capacity: Option<i64>, // ดึงจาก class_rooms ถ้า capacity_override = NULL
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
