use super::*;

#[derive(Debug, Clone, Deserialize, ToSchema, utoipa::IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct PromotionImpactQuery {
    pub after_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromotionCorrectionImpact {
    pub id: Uuid,
    pub item_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub evidence: crate::modules::academic::results::models::AnnualCorrectionEvidence,
    pub resolution: Option<PromotionImpactResolution>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromotionImpactWorkspace {
    pub run_id: Uuid,
    pub source_year_id: Uuid,
    pub target_year_id: Uuid,
    pub impacts: Vec<PromotionCorrectionImpact>,
    pub total_count: usize,
    pub pending_count: usize,
    pub next_cursor: Option<Uuid>,
    pub source_checksum: String,
}
