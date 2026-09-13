use super::*;
use crate::modules::academic::learner_evaluation::models::LearnerEvaluationDomain;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct AnnualCorrectionEvidence {
    pub annual_revision_id: Uuid,
    pub academic_term_id: Uuid,
    pub term_name: String,
    pub offering_code: String,
    pub offering_name: String,
    pub criterion_name: Option<String>,
    pub evaluation_domain: Option<LearnerEvaluationDomain>,
    pub result_id: Uuid,
    pub source_effective_version: i64,
    #[sqlx(json)]
    pub correction: ResultCorrectionRecord,
}
