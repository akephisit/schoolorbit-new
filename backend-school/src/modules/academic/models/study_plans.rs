use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};

// ==========================================
// Study Plan Models (หลักสูตรสถานศึกษา)
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct StudyPlan {
    pub id: Uuid,
    pub code: String,
    pub name_th: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub level_scope: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateStudyPlanRequest {
    pub code: String,
    pub name_th: String,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub level_scope: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStudyPlanRequest {
    pub code: Option<String>,
    pub name_th: Option<String>,
    pub name_en: Option<String>,
    pub description: Option<String>,
    pub level_scope: Option<String>,
    pub is_active: Option<bool>,
}

// ==========================================
// Study Plan Version Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct StudyPlanVersion {
    pub id: Uuid,
    pub study_plan_id: Uuid,
    pub version_name: String,
    pub start_academic_year_id: Uuid,
    pub end_academic_year_id: Option<Uuid>,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    
    // Joined fields (optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub study_plan_name_th: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub start_year_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateStudyPlanVersionRequest {
    pub study_plan_id: Uuid,
    pub version_name: String,
    pub start_academic_year_id: Uuid,
    pub end_academic_year_id: Option<Uuid>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStudyPlanVersionRequest {
    pub version_name: Option<String>,
    pub start_academic_year_id: Option<Uuid>,
    pub end_academic_year_id: Option<Uuid>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}

// ==========================================
// Study Plan Subject Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct StudyPlanSubject {
    pub id: Uuid,
    pub study_plan_version_id: Uuid,
    pub grade_level_id: Uuid,
    pub term: String,
    pub subject_id: Uuid,
    #[sqlx(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subject_code: Option<String>,  // joined from subjects.code
    pub display_order: i32,
    pub is_required: bool,
    #[sqlx(default)]
    pub metadata: serde_json::Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    
    // Joined fields
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub subject_name_th: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub subject_name_en: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub subject_credit: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub subject_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub grade_level_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct AddSubjectsToVersionRequest {
    pub subjects: Vec<SubjectInPlan>,
}

#[derive(Debug, Deserialize)]
pub struct SubjectInPlan {
    pub grade_level_id: Uuid,
    pub term: String,
    pub subject_id: Uuid,
    pub is_required: Option<bool>,
    pub display_order: Option<i32>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateStudyPlanSubjectRequest {
    pub display_order: Option<i32>,
    pub is_required: Option<bool>,
}

// ==========================================
// Query Filters
// ==========================================

#[derive(Debug, Deserialize)]
pub struct StudyPlanQuery {
    pub level_scope: Option<String>,
    pub active_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct StudyPlanVersionQuery {
    pub study_plan_id: Option<Uuid>,
    pub active_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct StudyPlanSubjectQuery {
    pub study_plan_version_id: Option<Uuid>,
    pub grade_level_id: Option<Uuid>,
    pub term: Option<String>,
}

// ==========================================
// Bulk Operations
// ==========================================

#[derive(Debug, Deserialize)]
pub struct GenerateCoursesFromPlanRequest {
    pub classroom_id: Uuid,
    pub academic_semester_id: Uuid,
    /// If true, will skip subjects that already exist in the classroom
    pub skip_existing: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct GenerateCoursesResponse {
    pub added_count: i32,
    pub skipped_count: i32,
    pub message: String,
}

// ==========================================
// Study Plan Version Activities (template กิจกรรมพัฒนาผู้เรียน)
// ==========================================

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct StudyPlanVersionActivity {
    pub id: Uuid,
    pub study_plan_version_id: Uuid,
    pub activity_catalog_id: Uuid,
    pub allowed_grade_level_ids: Option<serde_json::Value>,
    pub is_required: bool,
    pub display_order: i32,
    pub term: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,

    // Joined fields from catalog (for display)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub catalog_name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub catalog_activity_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub catalog_description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub catalog_periods_per_week: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub catalog_scheduling_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreatePlanActivityRequest {
    pub activity_catalog_id: Uuid,
    pub allowed_grade_level_ids: Option<Vec<Uuid>>,
    pub is_required: Option<bool>,
    pub display_order: Option<i32>,
    pub term: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdatePlanActivityRequest {
    pub allowed_grade_level_ids: Option<Vec<Uuid>>,
    pub is_required: Option<bool>,
    pub display_order: Option<i32>,
    pub term: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct GenerateActivitiesFromPlanRequest {
    pub study_plan_version_id: Uuid,
    pub semester_id: Uuid,
}

// ==========================================
// Activity Catalog (คลังกิจกรรม — pattern เดียวกับ subjects)
// ==========================================

#[derive(Debug, Serialize, Deserialize, sqlx::FromRow)]
pub struct ActivityCatalog {
    pub id: Uuid,
    pub name: String,
    pub activity_type: String,
    pub description: Option<String>,
    pub periods_per_week: i32,
    pub scheduling_mode: String,
    pub is_active: bool,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct CreateCatalogRequest {
    pub name: String,
    pub activity_type: String,
    pub description: Option<String>,
    pub periods_per_week: Option<i32>,
    pub scheduling_mode: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateCatalogRequest {
    pub name: Option<String>,
    pub activity_type: Option<String>,
    pub description: Option<String>,
    pub periods_per_week: Option<i32>,
    pub scheduling_mode: Option<String>,
    pub is_active: Option<bool>,
}
