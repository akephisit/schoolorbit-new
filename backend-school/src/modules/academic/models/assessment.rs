use bigdecimal::BigDecimal;
use serde::{Deserialize, Serialize};
use sqlx::{types::Json, FromRow};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::modules::academic::delivery::models::CourseGradingPolicy;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentPhaseCode {
    BeforeMidterm,
    Midterm,
    AfterMidterm,
    Final,
}

impl AssessmentPhaseCode {
    pub const ALL: [Self; 4] = [
        Self::BeforeMidterm,
        Self::Midterm,
        Self::AfterMidterm,
        Self::Final,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::BeforeMidterm => "before_midterm",
            Self::Midterm => "midterm",
            Self::AfterMidterm => "after_midterm",
            Self::Final => "final",
        }
    }

    pub fn label_th(self) -> &'static str {
        match self {
            Self::BeforeMidterm => "ก่อนกลางภาค",
            Self::Midterm => "กลางภาค",
            Self::AfterMidterm => "หลังกลางภาค",
            Self::Final => "ปลายภาค",
        }
    }

    pub fn order(self) -> i32 {
        match self {
            Self::BeforeMidterm => 1,
            Self::Midterm => 2,
            Self::AfterMidterm => 3,
            Self::Final => 4,
        }
    }

    pub fn supports_exam_arrangement(self) -> bool {
        matches!(self, Self::Midterm | Self::Final)
    }
}

impl TryFrom<&str> for AssessmentPhaseCode {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "before_midterm" => Ok(Self::BeforeMidterm),
            "midterm" => Ok(Self::Midterm),
            "after_midterm" => Ok(Self::AfterMidterm),
            "final" => Ok(Self::Final),
            _ => Err(format!("unknown assessment phase code: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentExamArrangement {
    None,
    InTimetable,
    OutsideTimetable,
}

impl AssessmentExamArrangement {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::InTimetable => "in_timetable",
            Self::OutsideTimetable => "outside_timetable",
        }
    }
}

impl TryFrom<&str> for AssessmentExamArrangement {
    type Error = String;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "none" => Ok(Self::None),
            "in_timetable" => Ok(Self::InTimetable),
            "outside_timetable" => Ok(Self::OutsideTimetable),
            _ => Err(format!("unknown assessment exam arrangement: {value}")),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentReadinessFinding {
    MissingCoordinator,
    CoordinatorNotCandidate,
    MissingPhase,
    TotalMismatch,
    MidtermMissingExamDuration,
    FinalMissingExamDuration,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssessmentPlanListQuery {
    pub academic_term_id: Uuid,
    pub subject_id: Option<Uuid>,
    pub instructor_id: Option<Uuid>,
    pub ready: Option<bool>,
    pub exam_arrangement: Option<AssessmentExamArrangement>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssessmentPhaseControlListQuery {
    pub academic_term_id: Uuid,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveAssessmentPlanRequest {
    /// Omit only when the offering does not have a persisted plan yet.
    pub row_version: Option<i64>,
    pub assessment_coordinator_id: Option<Uuid>,
    pub phases: Vec<SaveAssessmentPhaseRequest>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveAssessmentPhaseRequest {
    pub id: Option<Uuid>,
    pub phase_code: AssessmentPhaseCode,
    #[schema(value_type = String, pattern = r"^(0|[1-9]\d*)(\.\d{1,2})?$")]
    pub max_score: String,
    pub exam_arrangement: AssessmentExamArrangement,
    pub exam_duration_minutes: Option<i32>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateAssessmentPhaseControlRequest {
    pub row_version: i64,
    pub plan_editing_enabled: bool,
    pub score_entry_enabled: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentReadiness {
    pub ready: bool,
    pub findings: Vec<AssessmentReadinessFinding>,
    pub total_score: String,
    pub expected_total_score: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentCoordinatorOption {
    pub teacher_id: Uuid,
    pub display_name: String,
    pub learning_group_count: i64,
    pub primary_learning_group_count: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentPhase {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    pub phase_code: AssessmentPhaseCode,
    pub label: String,
    pub order: i32,
    pub max_score: String,
    pub exam_arrangement: AssessmentExamArrangement,
    pub exam_duration_minutes: Option<i32>,
    pub row_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentPlanSummary {
    pub plan_id: Option<Uuid>,
    pub offering_id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub subject_id: Uuid,
    pub subject_version_display_label: String,
    pub offering_code: String,
    pub offering_name: String,
    pub row_version: Option<i64>,
    pub learning_group_ids: Vec<Uuid>,
    pub learning_group_count: i64,
    pub assessment_coordinator_id: Option<Uuid>,
    pub assessment_coordinator_name: Option<String>,
    pub suggested_coordinator_id: Option<Uuid>,
    pub suggested_coordinator_name: Option<String>,
    pub phases: Vec<AssessmentPhase>,
    pub readiness: AssessmentReadiness,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentPlanDetail {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<Uuid>,
    pub offering_id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub subject_id: Uuid,
    pub subject_version_display_label: String,
    pub offering_code: String,
    pub offering_name: String,
    pub grading_policy: CourseGradingPolicy,
    pub row_version: Option<i64>,
    pub learning_group_ids: Vec<Uuid>,
    pub assessment_coordinator_id: Option<Uuid>,
    pub assessment_coordinator_name: Option<String>,
    pub suggested_coordinator_id: Option<Uuid>,
    pub suggested_coordinator_name: Option<String>,
    pub coordinator_candidates: Vec<AssessmentCoordinatorOption>,
    pub phases: Vec<AssessmentPhase>,
    pub readiness: AssessmentReadiness,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AssessmentPhaseControl {
    pub id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub phase_code: AssessmentPhaseCode,
    pub label: String,
    pub order: i32,
    pub plan_editing_enabled: bool,
    pub score_entry_enabled: bool,
    pub row_version: i64,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct AssessmentOfferingScopeRow {
    pub offering_id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_term_status: String,
    pub subject_version_id: Uuid,
    pub subject_id: Uuid,
    pub subject_version_display_label: String,
    pub offering_code: String,
    pub offering_name: String,
    pub grading_policy: Json<CourseGradingPolicy>,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct AssessmentPlanRow {
    pub id: Uuid,
    pub assessment_coordinator_id: Option<Uuid>,
    pub assessment_coordinator_name: Option<String>,
    pub row_version: i64,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct AssessmentPhaseRow {
    pub id: Uuid,
    pub plan_id: Uuid,
    pub phase_code: String,
    pub max_score: BigDecimal,
    pub exam_arrangement: String,
    pub exam_duration_minutes: Option<i32>,
    pub row_version: i64,
}

#[derive(Debug, Clone, FromRow)]
pub(crate) struct AssessmentPhaseControlRow {
    pub id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub phase_code: String,
    pub plan_editing_enabled: bool,
    pub score_entry_enabled: bool,
    pub row_version: i64,
}
