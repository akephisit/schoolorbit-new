use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "UPPERCASE")]
pub enum SubjectType {
    Basic,
    Additional,
    Activity,
}

#[derive(Debug, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum CurriculumInstructorRole {
    Primary,
    Secondary,
}

// ==========================================
// Curriculum: Subject Group Models
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow, ToSchema)]
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

#[derive(Debug, Serialize, Deserialize, FromRow, ToSchema)]
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
    #[schema(value_type = SubjectType)]
    pub subject_type: String, // 'type' is a reserved keyword in Rust
    pub group_id: Option<Uuid>,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub term: Option<String>,

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

#[derive(Debug, Deserialize, ToSchema)]
pub struct DefaultInstructorInput {
    pub instructor_id: Uuid,
    #[schema(value_type = CurriculumInstructorRole)]
    pub role: String, // "primary" | "secondary"
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateSubjectRequest {
    pub code: String,
    pub start_academic_year_id: Uuid, // effective-from year for this version
    pub name_th: String,
    pub name_en: Option<String>,
    pub credit: Option<f64>,
    pub hours_per_semester: Option<i32>,
    #[serde(rename = "type")]
    #[schema(value_type = SubjectType)]
    pub subject_type: String,
    pub group_id: Option<Uuid>,
    pub description: Option<String>,
    pub grade_level_ids: Option<Vec<Uuid>>,
    pub term: Option<String>,
    /// Full team to store in subject_default_instructors. When provided,
    /// junction rows are written exactly as listed.
    pub default_instructors: Option<Vec<DefaultInstructorInput>>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSubjectRequest {
    pub code: Option<String>,
    pub name_th: Option<String>,
    pub name_en: Option<String>,
    pub credit: Option<f64>,
    pub hours_per_semester: Option<i32>,
    #[serde(rename = "type")]
    #[schema(value_type = Option<SubjectType>)]
    pub subject_type: Option<String>,
    pub group_id: Option<Uuid>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
    pub start_academic_year_id: Option<Uuid>,
    pub grade_level_ids: Option<Vec<Uuid>>,
    pub term: Option<String>,
    /// When provided, replaces the subject's default team atomically.
    /// Pass `Some([])` to clear all defaults. Leave None to skip team update.
    pub default_instructors: Option<Vec<DefaultInstructorInput>>,
}

// ==========================================
// Subject Default Instructors (team teaching at catalog level)
// ==========================================

#[derive(Debug, Serialize, Deserialize, FromRow, ToSchema)]
pub struct SubjectDefaultInstructor {
    pub id: Uuid,
    pub subject_id: Uuid,
    pub instructor_id: Uuid,
    #[schema(value_type = CurriculumInstructorRole)]
    pub role: String,
    pub created_at: DateTime<Utc>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub instructor_name: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct AddSubjectDefaultInstructorRequest {
    pub instructor_id: Uuid,
    #[schema(value_type = Option<CurriculumInstructorRole>)]
    pub role: Option<String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateSubjectDefaultInstructorRoleRequest {
    #[schema(value_type = CurriculumInstructorRole)]
    pub role: String,
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
