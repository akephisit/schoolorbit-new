use super::*;

#[derive(Debug, Clone, Deserialize, ToSchema, utoipa::IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct PromotionRunListQuery {
    pub source_year_id: Uuid,
    pub target_year_id: Option<Uuid>,
    pub before_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromotionRunList {
    pub runs: Vec<PromotionRun>,
    pub next_cursor: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromotionRunStudent {
    pub item: PromotionRunItem,
    pub student_code: Option<String>,
    pub student_name: String,
    pub annual_result_current: bool,
    pub needs_recalculation: bool,
    pub receipt: Option<PromotionExecutionReceipt>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromotionRunWorkspace {
    pub run: PromotionRun,
    pub source_year: crate::modules::academic::core::models::YearLifecycleContext,
    pub target_year: crate::modules::academic::core::models::YearLifecycleContext,
    pub students: Vec<PromotionRunStudent>,
    pub approval_checksum: String,
}
