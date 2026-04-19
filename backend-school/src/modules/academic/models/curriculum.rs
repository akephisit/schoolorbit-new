use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use uuid::Uuid;
use chrono::{DateTime, Utc};

// ==========================================
// Curriculum: Subject Group Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct SubjectGroup {
    pub id: Uuid,
    pub code: String,
    pub name_th: String,
    pub name_en: String,
    pub display_order: i32,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

// ==========================================
// Curriculum: Subject Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow)]
pub struct Subject {
    pub id: Uuid,
    pub code: String,
    pub start_academic_year_id: Uuid, // version key: this subject definition is effective from this year
    pub name_th: String,
    pub name_en: Option<String>,
    pub credit: f64, // Changed from Decimal to f64 to avoid extra dependency
    pub hours_per_semester: Option<i32>,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub subject_type: String, // 'type' is a reserved keyword in Rust
    pub group_id: Option<Uuid>,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub term: Option<String>,
    pub default_instructor_id: Option<Uuid>,
    
    // Joined Fields (Optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub group_name_th: Option<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub grade_level_ids: Option<Vec<Uuid>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub default_instructor_name: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSubjectRequest {
    pub code: String,
    pub start_academic_year_id: Uuid, // effective-from year for this version
    pub name_th: String,
    pub name_en: Option<String>,
    pub credit: Option<f64>,
    pub hours_per_semester: Option<i32>,
    #[serde(rename = "type")]
    pub subject_type: String,
    pub group_id: Option<Uuid>,
    pub description: Option<String>,
    pub grade_level_ids: Option<Vec<Uuid>>,
    pub term: Option<String>,
    pub default_instructor_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSubjectRequest {
    pub code: Option<String>,
    pub name_th: Option<String>,
    pub name_en: Option<String>,
    pub credit: Option<f64>,
    pub hours_per_semester: Option<i32>,
    #[serde(rename = "type")]
    pub subject_type: Option<String>,
    pub group_id: Option<Uuid>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
    pub start_academic_year_id: Option<Uuid>,
    pub grade_level_ids: Option<Vec<Uuid>>,
    pub term: Option<String>,
    pub default_instructor_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct SubjectFilter {
    pub group_id: Option<Uuid>,
    #[serde(rename = "type")]
    pub subject_type: Option<String>,
    pub search: Option<String>,
    pub active_only: Option<bool>,
    /// For each subject code, return the latest version whose
    /// start_academic_year_id <= this target year.
    pub active_in_year_id: Option<Uuid>,
    pub term: Option<String>,
    /// When true (default), return only the latest version per `code`.
    /// When false, return all versions (for history/management views).
    pub latest_only: Option<bool>,
}

