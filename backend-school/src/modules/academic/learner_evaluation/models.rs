use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema, sqlx::Type,
)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum LearnerEvaluationDomain {
    DesirableCharacteristic,
    ReadingThinkingWriting,
}
impl LearnerEvaluationDomain {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DesirableCharacteristic => "desirable_characteristic",
            Self::ReadingThinkingWriting => "reading_thinking_writing",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(try_from = "i16", into = "i16")]
#[schema(value_type=i16)]
pub struct LearnerEvaluationLevel(i16);
impl TryFrom<i16> for LearnerEvaluationLevel {
    type Error = &'static str;
    fn try_from(value: i16) -> Result<Self, Self::Error> {
        if (0..=3).contains(&value) {
            Ok(Self(value))
        } else {
            Err("Quality level must be 0 through 3")
        }
    }
}
impl From<LearnerEvaluationLevel> for i16 {
    fn from(value: LearnerEvaluationLevel) -> Self {
        value.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in=Query)]
pub struct EvaluationContext {
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationCriterion {
    pub id: Uuid,
    pub school_criterion_id: Option<Uuid>,
    pub name: String,
    pub lifecycle: String,
    pub display_order: i32,
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CriterionInput {
    pub name: String,
    pub display_order: i32,
    pub active: bool,
    pub row_version: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct CatalogCriterion {
    pub id: Uuid,
    pub domain: LearnerEvaluationDomain,
    pub name: String,
    pub applicability: String,
    pub lifecycle: String,
    pub display_order: i32,
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CatalogInput {
    pub domain: LearnerEvaluationDomain,
    pub applicability: String,
    #[serde(flatten)]
    pub criterion: CriterionInput,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationConfiguration {
    pub subject_id: Uuid,
    pub domain: LearnerEvaluationDomain,
    pub row_version: i64,
    pub locked: bool,
    pub can_manage: bool,
    pub criteria: Vec<EvaluationCriterion>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationControl {
    pub id: Uuid,
    pub domain: LearnerEvaluationDomain,
    pub entry_enabled: bool,
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ControlInput {
    pub entry_enabled: bool,
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationStudent {
    pub membership_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub display_name: String,
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationResponse {
    pub subject_term_criterion_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub quality_level: i16,
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResponseInput {
    pub subject_term_criterion_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub quality_level: Option<LearnerEvaluationLevel>,
    pub row_version: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ResponseBatchInput {
    pub cells: Vec<ResponseInput>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationConfirmation {
    pub id: Uuid,
    pub source_checksum: String,
    pub roster_checksum: String,
    pub row_version: i64,
    pub invalidated: bool,
    pub confirmed_by: Uuid,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmationInput {
    pub source_checksum: String,
    pub roster_checksum: String,
    pub row_version: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MissingEvaluation {
    pub student_academic_year_id: Uuid,
    pub subject_term_criterion_id: Uuid,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmationOutcome {
    pub confirmation: Option<EvaluationConfirmation>,
    pub missing: Vec<MissingEvaluation>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationWorkspace {
    pub learning_group_id: Uuid,
    pub subject_id: Uuid,
    pub domain: LearnerEvaluationDomain,
    pub criteria: Vec<EvaluationCriterion>,
    pub students: Vec<EvaluationStudent>,
    pub responses: Vec<EvaluationResponse>,
    pub entry_enabled: bool,
    pub locked: bool,
    pub can_manage: bool,
    pub can_confirm: bool,
    pub source_checksum: String,
    pub roster_checksum: String,
    pub confirmation: Option<EvaluationConfirmation>,
    pub confirmation_is_current: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationLock {
    pub id: Uuid,
    pub subject_id: Uuid,
    pub domain: LearnerEvaluationDomain,
    pub source_checksum: String,
    pub roster_checksum: String,
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LockBlocker {
    pub learning_group_id: Uuid,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LockOutcome {
    pub lock: Option<EvaluationLock>,
    pub blockers: Vec<LockBlocker>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CriterionRemoval {
    pub id: Uuid,
    pub deleted: bool,
    pub criterion: Option<EvaluationCriterion>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VersionInput {
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExactAverage {
    pub numerator: String,
    pub denominator: String,
    pub decimal: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct LockedCriterionValue {
    pub id: Uuid,
    pub subject_id: Uuid,
    pub domain: LearnerEvaluationDomain,
    pub subject_term_criterion_id: Uuid,
    pub school_criterion_id: Option<Uuid>,
    pub name: String,
    pub quality_level: i16,
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SubjectEvaluationSummary {
    pub subject_id: Uuid,
    pub average: ExactAverage,
    pub criteria: Vec<LockedCriterionValue>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEvaluationSummary {
    pub school_criterion_id: Uuid,
    pub average: ExactAverage,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MissingSubject {
    pub subject_id: Uuid,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DomainEvaluationSummary {
    pub domain: LearnerEvaluationDomain,
    pub average: Option<ExactAverage>,
    pub quality_level: Option<i16>,
    pub complete: bool,
    pub missing_subjects: Vec<MissingSubject>,
    pub catalog_criteria: Vec<CatalogEvaluationSummary>,
    pub subjects: Vec<SubjectEvaluationSummary>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StudentEvaluationSummary {
    pub student_academic_year_id: Uuid,
    pub policy_version_id: Uuid,
    pub domains: Vec<DomainEvaluationSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationSubject {
    pub subject_id: Uuid,
    pub learning_group_id: Uuid,
    pub learning_offering_id: Uuid,
    pub code: String,
    pub name: String,
    pub group_name: String,
    pub assigned: bool,
}
