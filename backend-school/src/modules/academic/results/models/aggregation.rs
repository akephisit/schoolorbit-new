use super::CourseOfficialOutcome;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

/// One expected subject in one student's term, including an absent locked result.
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CourseAggregateInput {
    pub subject_id: Uuid,
    pub learning_offering_id: Uuid,
    pub result_id: Option<Uuid>,
    pub effective_version: Option<i64>,
    pub credits: String,
    pub outcome: Option<CourseOfficialOutcome>,
    pub numeric_grade: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CourseCreditTotals {
    pub attempted_credits: String,
    pub graded_credits: String,
    pub earned_credits: String,
    pub unresolved_credits: String,
    pub weighted_grade_points: String,
    /// Display-only average of numeric results; never an official aggregate lock.
    pub provisional_gpa: Option<String>,
    pub missing_result_count: usize,
    pub exceptional_result_count: usize,
    pub coverage_complete: bool,
    pub all_outcomes_numeric: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct TermResultPreviewQuery {
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    /// Explicit preview criterion, not authority to change school grading policy.
    pub passing_grade: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TermResultPreview {
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub passing_grade: String,
    pub courses: Vec<CourseAggregateInput>,
    pub totals: CourseCreditTotals,
    pub source_checksum: String,
}
