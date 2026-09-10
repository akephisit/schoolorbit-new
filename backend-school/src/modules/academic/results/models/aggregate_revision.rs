use super::*;
use crate::modules::academic::learner_evaluation::models::StudentEvaluationSummary;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AggregatePolicyInput {
    pub name: String,
    pub passing_grade: String,
    pub minimum_learner_level: i16,
    pub allow_reviewed_holds: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AggregatePolicyVersion {
    pub id: Uuid,
    pub name: String,
    pub passing_grade: String,
    pub minimum_learner_level: i16,
    pub allow_reviewed_holds: bool,
    pub approved_by: Uuid,
    pub approved_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct AggregatePreviewQuery {
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub policy_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AggregateBlocker {
    NoCourseCoverage,
    MissingCourseResults,
    MissingActivityResults,
    MissingLearnerEvaluations,
    ReviewedHoldNotAllowed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AggregateHoldFinding {
    ExceptionalCourseOutcomes,
    FailedActivities,
    FailedLearnerEvaluations,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TermAggregatePreview {
    pub policy: AggregatePolicyVersion,
    pub results: TermResultPreview,
    pub learner_evaluations: StudentEvaluationSummary,
    pub blockers: Vec<AggregateBlocker>,
    pub hold_findings: Vec<AggregateHoldFinding>,
    pub can_lock: bool,
    pub source_checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AggregateLockInput {
    pub policy_id: Uuid,
    pub source_checksum: String,
    pub expected_revision: Option<i64>,
    pub request_id: Uuid,
    pub hold_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TermAggregateRevision {
    pub id: Uuid,
    pub revision: i64,
    pub snapshot: TermAggregatePreview,
    pub official_gpa: Option<String>,
    pub hold_reason: Option<String>,
    pub locked_by: Uuid,
    pub locked_at: DateTime<Utc>,
    /// Recalculated against the policy pinned to this revision, not a newer policy.
    pub is_current: bool,
}
