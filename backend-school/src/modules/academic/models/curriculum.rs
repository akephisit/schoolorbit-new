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
    pub academic_year_id: Uuid, // FK to academic_years for referential integrity
    pub name_th: String,
    pub name_en: Option<String>,
    pub credit: f64, // Changed from Decimal to f64 to avoid extra dependency
    pub hours_per_semester: Option<i32>,
    #[serde(rename = "type")]
    #[sqlx(rename = "type")]
    pub subject_type: String, // 'type' is a reserved keyword in Rust
    pub group_id: Option<Uuid>,
    pub level_scope: Option<String>,
    pub description: Option<String>,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    
    // Joined Fields (Optional)
    #[serde(skip_serializing_if = "Option::is_none")]
    #[sqlx(default)]
    pub group_name_th: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateSubjectRequest {
    pub code: String,
    pub academic_year_id: Uuid, // Required FK
    pub name_th: String,
    pub name_en: Option<String>,
    pub credit: f64,
    pub hours_per_semester: Option<i32>,
    #[serde(rename = "type")]
    pub subject_type: String,
    pub group_id: Option<Uuid>,
    pub level_scope: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateSubjectRequest {
    pub code: Option<String>,
    pub academic_year_id: Option<Uuid>,
    pub name_th: Option<String>,
    pub name_en: Option<String>,
    pub credit: Option<f64>,
    pub hours_per_semester: Option<i32>,
    #[serde(rename = "type")]
    pub subject_type: Option<String>,
    pub group_id: Option<Uuid>,
    pub level_scope: Option<String>,
    pub description: Option<String>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct SubjectFilter {
    pub group_id: Option<Uuid>,
    pub level_scope: Option<String>,
    #[serde(rename = "type")]
    pub subject_type: Option<String>,
    pub search: Option<String>,
    pub active_only: Option<bool>,
    pub academic_year_id: Option<Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct BulkCopySubjectsRequest {
    pub source_academic_year_id: Uuid,
    pub target_academic_year_id: Uuid,
}

#[derive(Debug, Serialize)]
pub struct BulkCopySubjectsResponse {
    pub copied_count: i32,
    pub skipped_count: i32,
    pub message: String,
}
