use super::*;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct YearRolloverStudentInput {
    pub student_academic_year_id: Uuid,
    pub decision: PromotionDecisionInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct YearRolloverPreviewInput {
    pub source_year_id: Uuid,
    pub target_year_id: Uuid,
    pub decisions: Vec<YearRolloverStudentInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConfirmYearRolloverInput {
    pub request_id: Uuid,
    pub source_checksum: String,
    pub preview: YearRolloverPreviewInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct YearRolloverStudent {
    pub student_academic_year_id: Uuid,
    pub student_id: Uuid,
    pub student_code: Option<String>,
    pub student_name: String,
    pub source_grade_level_id: Uuid,
    pub source_study_program_id: Uuid,
    pub source_homeroom_id: Option<Uuid>,
    pub source_homeroom_name: Option<String>,
    pub source_row_version: i64,
    pub existing_target_student_year_id: Option<Uuid>,
    pub decision: Option<PromotionDecisionInput>,
    pub suggested_decision: Option<PromotionDecisionInput>,
    pub current_target: Option<YearRolloverCurrentTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct YearRolloverProgramReference {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub curriculum_level_id: Uuid,
    pub level_name: String,
    pub edition_name: String,
    #[sqlx(json)]
    pub grade_level_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct YearRolloverHomeroomReference {
    pub id: Uuid,
    pub name: String,
    pub grade_level_id: Uuid,
    pub study_program_id: Uuid,
    pub capacity: Option<i32>,
    pub occupied_count: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct YearRolloverPreview {
    pub source_year: school_academic_core::models::YearLifecycleContext,
    pub target_year: school_academic_core::models::YearLifecycleContext,
    pub students: Vec<YearRolloverStudent>,
    pub grades: Vec<PromotionGradeReference>,
    pub programs: Vec<YearRolloverProgramReference>,
    pub homerooms: Vec<YearRolloverHomeroomReference>,
    pub findings: Vec<LifecycleFinding>,
    pub can_confirm: bool,
    pub source_checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct YearRolloverOutcome {
    pub request_id: Uuid,
    pub run_id: Uuid,
    pub source_year_id: Uuid,
    pub target_year_id: Uuid,
    pub prepared: bool,
    pub activated: bool,
    pub student_count: usize,
    pub receipts: Vec<PromotionExecutionReceipt>,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct YearRolloverOptions {
    pub target_years: Vec<school_academic_core::models::YearLifecycleContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct YearRolloverCurrentTarget {
    pub student_academic_year_id: Uuid,
    pub row_version: i64,
    pub status: school_academic_core::models::StudentAcademicYearStatus,
    pub grade_level_id: Uuid,
    pub study_program_id: Uuid,
    pub study_program_name: String,
    pub placement_id: Option<Uuid>,
    pub placement_row_version: Option<i64>,
    pub placement_starts_on: Option<chrono::NaiveDate>,
    pub homeroom_id: Option<Uuid>,
    pub homeroom_name: Option<String>,
}
