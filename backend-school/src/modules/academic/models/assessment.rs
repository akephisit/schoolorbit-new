use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentPlanListQuery {
    pub academic_semester_id: Option<Uuid>,
    pub classroom_id: Option<Uuid>,
    pub instructor_id: Option<Uuid>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAssessmentPlanRequest {
    pub categories: Vec<SaveAssessmentCategoryRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAssessmentCategoryRequest {
    pub id: Option<Uuid>,
    pub code: Option<String>,
    pub name: String,
    pub max_score: f64,
    pub exam_mode: String,
    pub display_order: i32,
    pub items: Vec<SaveAssessmentItemRequest>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveAssessmentItemRequest {
    pub id: Option<Uuid>,
    pub name: String,
    pub max_score: f64,
    pub display_order: i32,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentPlanSummary {
    pub plan_id: Option<Uuid>,
    pub classroom_course_id: Uuid,
    pub classroom_id: Uuid,
    pub subject_id: Uuid,
    pub academic_semester_id: Uuid,
    pub primary_instructor_id: Option<Uuid>,
    pub status: String,
    pub subject_code: Option<String>,
    pub subject_name_th: Option<String>,
    pub subject_name_en: Option<String>,
    pub classroom_name: Option<String>,
    pub instructor_name: Option<String>,
    pub category_count: i64,
    pub item_count: i64,
    pub total_score: f64,
    pub outside_timetable_count: i64,
    pub in_timetable_count: i64,
    pub has_unallocated_categories: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentPlanDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    pub classroom_course_id: Uuid,
    pub status: String,
    pub submitted_at: Option<DateTime<Utc>>,
    pub locked_at: Option<DateTime<Utc>>,
    pub categories: Vec<AssessmentCategory>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentCategory {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    pub code: Option<String>,
    pub name: String,
    pub max_score: f64,
    pub exam_mode: String,
    pub display_order: i32,
    pub item_total_score: f64,
    pub allocation_status: String,
    pub items: Vec<AssessmentItem>,
}

#[derive(Debug, Clone, Serialize, FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentItem {
    pub id: Uuid,
    pub category_id: Uuid,
    pub name: String,
    pub max_score: f64,
    pub display_order: i32,
    pub is_active: bool,
}

#[derive(Debug, Clone, FromRow)]
pub struct AssessmentPlanRow {
    pub id: Uuid,
    pub classroom_course_id: Uuid,
    pub status: String,
    pub submitted_at: Option<DateTime<Utc>>,
    pub locked_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, FromRow)]
pub struct AssessmentCategoryRow {
    pub id: Uuid,
    pub code: Option<String>,
    pub name: String,
    pub max_score: f64,
    pub exam_mode: String,
    pub display_order: i32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentSettingsResponse {
    pub teacher_access_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateAssessmentSettingsRequest {
    pub teacher_access_enabled: bool,
}
