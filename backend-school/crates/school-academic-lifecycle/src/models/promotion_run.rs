use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum PromotionRunStatus {
    Draft,
    Calculated,
    Reviewed,
    Approved,
    Executing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreatePromotionRunInput {
    pub request_id: Uuid,
    pub source_year_id: Uuid,
    pub target_year_id: Uuid,
    pub policy_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApprovePromotionRunInput {
    pub request_id: Uuid,
    pub row_version: i64,
    pub source_checksum: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PromotionRun {
    pub id: Uuid,
    pub source_year_id: Uuid,
    pub target_year_id: Uuid,
    pub policy_id: Uuid,
    pub status: PromotionRunStatus,
    pub row_version: i64,
    pub created_by: Uuid,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<chrono::DateTime<chrono::Utc>>,
    pub approved_by: Option<Uuid>,
    pub approved_at: Option<chrono::DateTime<chrono::Utc>>,
    pub approval_id: Option<Uuid>,
    pub executed_by: Option<Uuid>,
    pub executed_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum PromotionDecisionOutcome {
    Promote,
    Repeat,
    Graduate,
    TransferOut,
    Hold,
    Conditional,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PromotionDecisionInput {
    pub outcome: PromotionDecisionOutcome,
    pub target_grade_level_id: Option<Uuid>,
    pub target_study_program_id: Option<Uuid>,
    pub target_homeroom_id: Option<Uuid>,
    pub reason: Option<String>,
    pub condition: Option<String>,
}

impl PromotionDecisionInput {
    pub(crate) fn core_command(&self) -> school_academic_core::models::PromotionDestinationCommand {
        use school_academic_core::models::PromotionDestinationOutcome as CoreOutcome;

        let outcome = match self.outcome {
            PromotionDecisionOutcome::Promote => CoreOutcome::Promote,
            PromotionDecisionOutcome::Repeat => CoreOutcome::Repeat,
            PromotionDecisionOutcome::Graduate => CoreOutcome::Graduate,
            PromotionDecisionOutcome::TransferOut => CoreOutcome::TransferOut,
            PromotionDecisionOutcome::Hold => CoreOutcome::Hold,
            PromotionDecisionOutcome::Conditional => CoreOutcome::Conditional,
        };
        school_academic_core::models::PromotionDestinationCommand {
            outcome,
            target_grade_level_id: self.target_grade_level_id,
            target_study_program_id: self.target_study_program_id,
            target_homeroom_id: self.target_homeroom_id,
            reason: self.reason.clone(),
            condition: self.condition.clone(),
        }
    }
}
