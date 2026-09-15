use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use school_academic_core::ports::{
    TermPreparationContext, TermPreparationMappingKind, TermPreparationModule,
    TermPreparationModuleOutcome,
};

use super::{LifecycleFinding, LifecycleSeverity};

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TermPreparationEntityMapping {
    pub kind: TermPreparationMappingKind,
    pub source_id: Uuid,
    pub target_id: Uuid,
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TermPreparationDateMapping {
    pub source_date: NaiveDate,
    pub target_date: NaiveDate,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, Eq, PartialEq, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TermPreparationMappings {
    #[serde(default)]
    pub entities: Vec<TermPreparationEntityMapping>,
    #[serde(default)]
    pub dates: Vec<TermPreparationDateMapping>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewTermPreparationInput {
    pub source_term_id: Uuid,
    pub target_term_id: Uuid,
    pub modules: Vec<TermPreparationModule>,
    #[serde(default)]
    pub mappings: TermPreparationMappings,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyTermPreparationInput {
    pub request_id: Uuid,
    pub source_checksum: String,
    pub source_term_id: Uuid,
    pub target_term_id: Uuid,
    pub modules: Vec<TermPreparationModule>,
    #[serde(default)]
    pub mappings: TermPreparationMappings,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TermPreparationMappingOption {
    pub id: Uuid,
    pub label: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TermPreparationMappingRequirement {
    pub kind: TermPreparationMappingKind,
    pub source_id: Uuid,
    pub source_label: String,
    pub required_by: Vec<TermPreparationModule>,
    pub selected_target_id: Option<Uuid>,
    pub suggested_target_id: Option<Uuid>,
    pub target_options: Vec<TermPreparationMappingOption>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TermPreparationDateRequirement {
    pub source_date: NaiveDate,
    pub source_label: String,
    pub required_by: Vec<TermPreparationModule>,
    pub selected_target_date: Option<NaiveDate>,
    pub suggested_target_date: Option<NaiveDate>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TermPreparationModuleEvidence {
    pub module: TermPreparationModule,
    pub source_count: usize,
    pub target_existing_count: usize,
    pub draft_count: usize,
    pub status: LifecycleSeverity,
    pub summary: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TermPreparationWorkspace {
    pub context: TermPreparationContext,
    pub modules: Vec<TermPreparationModuleEvidence>,
    pub mapping_requirements: Vec<TermPreparationMappingRequirement>,
    pub date_requirements: Vec<TermPreparationDateRequirement>,
    pub findings: Vec<LifecycleFinding>,
    pub can_apply: bool,
    pub source_checksum: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TermPreparationOutcome {
    pub run_id: Uuid,
    pub request_id: Uuid,
    pub source_term_id: Uuid,
    pub target_term_id: Uuid,
    pub modules: Vec<TermPreparationModuleOutcome>,
    pub created_at: DateTime<Utc>,
}
