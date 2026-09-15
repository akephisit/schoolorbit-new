use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Core-owned outcome vocabulary for one promotion destination command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PromotionDestinationOutcome {
    Promote,
    Repeat,
    Graduate,
    TransferOut,
    Hold,
    Conditional,
}

/// A validated destination intent supplied by the Lifecycle orchestrator.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromotionDestinationCommand {
    pub outcome: PromotionDestinationOutcome,
    pub target_grade_level_id: Option<Uuid>,
    pub target_study_program_id: Option<Uuid>,
    pub target_homeroom_id: Option<Uuid>,
    pub reason: Option<String>,
    pub condition: Option<String>,
}
