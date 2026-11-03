use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum AcademicRetentionBoundary {
    Term,
    Year,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct AcademicRetentionPolicy {
    pub enabled: bool,
    pub primary_boundary: AcademicRetentionBoundary,
    pub secondary_boundary: AcademicRetentionBoundary,
    pub grace_days: i32,
    pub effective_from: Option<DateTime<Utc>>,
    pub row_version: i64,
    pub updated_by: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAcademicRetentionPolicy {
    pub expected_row_version: i64,
    pub enabled: bool,
    pub primary_boundary: AcademicRetentionBoundary,
    pub secondary_boundary: AcademicRetentionBoundary,
    pub grace_days: i32,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct AcademicRetentionImpact {
    pub attendance_records: i64,
    pub evidence_files: i64,
    pub evidence_bytes: i64,
    pub behavior_entries: i64,
    pub attendance_archived: bool,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct AcademicRetentionTermPreview {
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub level_types: Vec<String>,
    pub required_boundary: AcademicRetentionBoundary,
    pub closed_at: Option<DateTime<Utc>>,
    pub eligible_at: Option<DateTime<Utc>>,
    pub eligible: bool,
    pub blocked_reasons: Vec<String>,
    pub impact: AcademicRetentionImpact,
    pub preserves_attendance_summaries: bool,
    pub preserves_behavior_totals: bool,
    pub job: Option<AcademicRetentionJobStatus>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct AcademicRetentionJobStatus {
    pub id: Uuid,
    pub status: String,
    pub logical_cleanup_complete: bool,
    pub physical_cleanup_complete: bool,
    pub pending_files: i64,
    pub failed_files: i64,
    pub last_error_code: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct AcademicRetentionWorkspace {
    pub policy: AcademicRetentionPolicy,
    pub terms: Vec<AcademicRetentionTermPreview>,
    pub whole_term_cleanup: bool,
}
