use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct ResultContext {
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GradingPolicyBand {
    pub grade: String,
    pub lower_bound: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GradingPolicyInput {
    pub name: String,
    pub bands: Vec<GradingPolicyBand>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GradingPolicyVersion {
    pub id: Uuid,
    pub version_no: i32,
    pub name: String,
    pub lifecycle: String,
    pub row_version: i64,
    pub bands: Vec<GradingPolicyBand>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CourseOutcomeSelection {
    Derived,
    #[serde(rename = "manual_zero")]
    ExplicitZero,
    Incomplete,
    InsufficientAttendance,
}

impl CourseOutcomeSelection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Derived => "derived",
            Self::ExplicitZero => "manual_zero",
            Self::Incomplete => "incomplete",
            Self::InsufficientAttendance => "insufficient_attendance",
        }
    }
}

impl TryFrom<&str> for CourseOutcomeSelection {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "derived" => Ok(Self::Derived),
            "manual_zero" => Ok(Self::ExplicitZero),
            "incomplete" => Ok(Self::Incomplete),
            "insufficient_attendance" => Ok(Self::InsufficientAttendance),
            _ => Err("Unknown course outcome selection"),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ActivityOutcome {
    Pass,
    Fail,
}

impl ActivityOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
        }
    }
}

impl TryFrom<&str> for ActivityOutcome {
    type Error = &'static str;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "pass" => Ok(Self::Pass),
            "fail" => Ok(Self::Fail),
            _ => Err("Unknown activity outcome"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectionInput {
    pub student_academic_year_id: Uuid,
    pub selection: CourseOutcomeSelection,
    pub row_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityCellInput {
    pub student_academic_year_id: Uuid,
    pub outcome: Option<ActivityOutcome>,
    pub row_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityBatchInput {
    pub cells: Vec<ActivityCellInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResultConfirmationInput {
    pub source_checksum: String,
    pub roster_checksum: String,
    pub row_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PolicyActivationInput {
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ResultConfirmation {
    pub id: Uuid,
    pub row_version: i64,
    pub source_checksum: String,
    pub roster_checksum: String,
    pub confirmed_by: Uuid,
    pub invalidated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResultBlockerCode {
    MissingPhaseConfirmation,
    StalePhaseConfirmation,
    InvalidAssessmentPlan,
    InvalidGradingPolicy,
    MissingActivityOutcome,
    MissingPrimaryTeacher,
    MissingGroupConfirmation,
    StaleGroupConfirmation,
    AlreadyLocked,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResultBlocker {
    pub code: ResultBlockerCode,
    pub assessment_phase_id: Option<Uuid>,
    pub student_academic_year_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreparedCourseStudent {
    pub student_academic_year_id: Uuid,
    pub display_name: String,
    pub calculated_score: String,
    pub calculated_grade: Option<String>,
    pub selection: CourseOutcomeSelection,
    pub selection_row_version: Option<i64>,
    pub numeric_grade: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreparedActivityStudent {
    pub student_academic_year_id: Uuid,
    pub display_name: String,
    pub outcome: Option<ActivityOutcome>,
    pub row_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CoursePreparationWorkspace {
    pub learning_group_id: Uuid,
    pub subject_id: Uuid,
    pub policy: GradingPolicyVersion,
    pub students: Vec<PreparedCourseStudent>,
    pub source_checksum: String,
    pub roster_checksum: String,
    pub confirmation: Option<ResultConfirmation>,
    pub confirmation_is_current: bool,
    pub blockers: Vec<ResultBlocker>,
    pub locked: bool,
    pub can_manage: bool,
    pub can_confirm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityPreparationWorkspace {
    pub learning_group_id: Uuid,
    pub students: Vec<PreparedActivityStudent>,
    pub source_checksum: String,
    pub roster_checksum: String,
    pub confirmation: Option<ResultConfirmation>,
    pub confirmation_is_current: bool,
    pub blockers: Vec<ResultBlocker>,
    pub locked: bool,
    pub can_manage: bool,
    pub can_confirm: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GroupResultReadiness {
    pub learning_group_id: Uuid,
    pub learning_offering_id: Uuid,
    pub subject_id: Option<Uuid>,
    pub group_name: String,
    pub offering_name: String,
    pub assigned: bool,
    pub ready: bool,
    pub locked: bool,
    pub blockers: Vec<ResultBlocker>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubjectResultReadiness {
    pub subject_id: Uuid,
    pub ready: bool,
    pub groups: Vec<GroupResultReadiness>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResultReadiness {
    pub courses: Vec<SubjectResultReadiness>,
    pub activities: Vec<GroupResultReadiness>,
}
