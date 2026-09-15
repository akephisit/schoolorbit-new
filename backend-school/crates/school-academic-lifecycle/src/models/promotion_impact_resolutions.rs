use super::*;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum PromotionImpactResolutionKind {
    KeepExisting,
    ReplaceDecision,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvePromotionImpactInput {
    pub request_id: Uuid,
    pub source_checksum: String,
    pub resolution_kind: PromotionImpactResolutionKind,
    pub replacement_decision: Option<PromotionDecisionInput>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromotionImpactResolutionOutcome {
    pub adjusted: bool,
    pub target_student_year_id: Option<Uuid>,
    pub target_placement_id: Option<Uuid>,
    pub source_row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PromotionImpactResolution {
    pub id: Uuid,
    pub request_id: Uuid,
    pub run_id: Uuid,
    pub item_id: Uuid,
    pub correction_id: Uuid,
    pub impact_id: Uuid,
    pub resolution_kind: PromotionImpactResolutionKind,
    #[sqlx(json(nullable))]
    pub replacement_decision: Option<PromotionDecisionInput>,
    pub reason: String,
    pub source_checksum: String,
    #[sqlx(json)]
    pub outcome: PromotionImpactResolutionOutcome,
    pub resolved_by: Uuid,
    pub resolved_at: DateTime<Utc>,
}
