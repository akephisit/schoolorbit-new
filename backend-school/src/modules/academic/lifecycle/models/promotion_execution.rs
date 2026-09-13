use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecutePromotionRunInput {
    pub request_id: Uuid,
    pub row_version: i64,
    pub limit: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PromotionExecutionReceipt {
    pub item_id: Uuid,
    pub run_id: Uuid,
    pub request_id: Uuid,
    pub approval_id: Uuid,
    pub target_student_year_id: Option<Uuid>,
    pub target_placement_id: Option<Uuid>,
    pub source_row_version: i64,
    pub executed_by: Uuid,
    pub executed_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromotionExecutionFailure {
    pub item_id: Uuid,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromotionExecutionResult {
    pub run: PromotionRun,
    pub receipts: Vec<PromotionExecutionReceipt>,
    pub failures: Vec<PromotionExecutionFailure>,
    pub remaining_count: i64,
    pub hold_count: i64,
}
