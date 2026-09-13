use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PromotionSuccessOutcome {
    Promote,
    Graduate,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromotionRuleInput {
    pub from_grade_level_id: Uuid,
    pub from_study_program_id: Uuid,
    pub target_grade_level_id: Option<Uuid>,
    pub target_study_program_id: Option<Uuid>,
    pub success_outcome: PromotionSuccessOutcome,
    pub minimum_earned_credits: String,
    pub require_no_exceptional_outcomes: bool,
    pub require_activities_passed: bool,
    pub minimum_learner_level: i16,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromotionPolicyInput {
    pub name: String,
    pub rules: Vec<PromotionRuleInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromotionPolicyVersion {
    pub id: Uuid,
    pub name: String,
    pub rules: Vec<PromotionRuleInput>,
    pub progression_row_version: i64,
    pub reviewed_by: Uuid,
    pub reviewed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PromotionGradeReference {
    pub id: Uuid,
    pub level_type: String,
    pub year: i32,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PromotionProgramReference {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub curriculum_id: Uuid,
    pub curriculum_name: String,
    pub version_name: String,
    pub status: crate::modules::academic::core::models::VersionStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromotionPolicyOptions {
    pub grades: Vec<PromotionGradeReference>,
    pub programs: Vec<PromotionProgramReference>,
    pub progression_set: crate::modules::academic::core::models::GradeProgressionSet,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PromotionRecommendationFinding {
    PolicyRuleMissing,
    AnnualResultMissing,
    AnnualResultStale,
    ReviewedAnnualHold,
    InsufficientEarnedCredits,
    ExceptionalOutcomes,
    ActivitiesNotPassed,
    LearnerEvaluationNotPassed,
    LearnerEvaluationMissing,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromotionRecommendation {
    pub suggested_outcome: Option<PromotionSuccessOutcome>,
    pub target_grade_level_id: Option<Uuid>,
    pub target_study_program_id: Option<Uuid>,
    pub findings: Vec<PromotionRecommendationFinding>,
}
