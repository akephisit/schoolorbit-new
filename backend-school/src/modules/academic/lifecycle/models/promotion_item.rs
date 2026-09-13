use super::*;
use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum PromotionItemStatus {
    Calculated,
    Reviewed,
    Executed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PromotionRunItem {
    pub id: Uuid,
    pub run_id: Uuid,
    pub source_year_id: Uuid,
    pub target_year_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub student_id: Uuid,
    pub source_grade_level_id: Uuid,
    pub source_study_program_id: Uuid,
    pub source_row_version: i64,
    pub annual_revision_id: Option<Uuid>,
    pub source_checksum: String,
    #[sqlx(json)]
    pub recommendation: PromotionRecommendation,
    pub existing_target_student_year_id: Option<Uuid>,
    #[sqlx(json(nullable))]
    pub decision: Option<PromotionDecisionInput>,
    pub reviewed_by: Option<Uuid>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub status: PromotionItemStatus,
    pub row_version: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CalculatePromotionRunInput {
    pub request_id: Uuid,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromotionRunCalculation {
    pub run: PromotionRun,
    pub items: Vec<PromotionRunItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReviewPromotionItemInput {
    pub row_version: i64,
    pub decision: PromotionDecisionInput,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromotionItemReview {
    pub run: PromotionRun,
    pub item: PromotionRunItem,
}
