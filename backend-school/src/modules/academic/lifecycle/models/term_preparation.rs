use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::{LifecycleFinding, LifecycleSeverity};

#[derive(
    Debug, Clone, Copy, Deserialize, Serialize, Eq, Hash, Ord, PartialEq, PartialOrd, ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TermPreparationModule {
    Delivery,
    Assessments,
    Timetable,
    Exams,
    Supervision,
}

impl TermPreparationModule {
    pub const ALL: [Self; 5] = [
        Self::Delivery,
        Self::Assessments,
        Self::Timetable,
        Self::Exams,
        Self::Supervision,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Delivery => "delivery",
            Self::Assessments => "assessments",
            Self::Timetable => "timetable",
            Self::Exams => "exams",
            Self::Supervision => "supervision",
        }
    }

    pub fn label_th(self) -> &'static str {
        match self {
            Self::Delivery => "รายการเปิดสอนและกลุ่มเรียน",
            Self::Assessments => "โครงสร้างคะแนน",
            Self::Timetable => "แบบร่างตารางสอน",
            Self::Exams => "แบบร่างตารางสอบ",
            Self::Supervision => "แบบร่างรอบนิเทศ",
        }
    }
}

#[derive(
    Debug, Clone, Copy, Deserialize, Serialize, Eq, Hash, Ord, PartialEq, PartialOrd, ToSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum TermPreparationMappingKind {
    LearningOffering,
    LearningGroup,
    Teacher,
    Homeroom,
    Room,
    BellPeriod,
    AssessmentPlan,
}

impl TermPreparationMappingKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::LearningOffering => "learning_offering",
            Self::LearningGroup => "learning_group",
            Self::Teacher => "teacher",
            Self::Homeroom => "homeroom",
            Self::Room => "room",
            Self::BellPeriod => "bell_period",
            Self::AssessmentPlan => "assessment_plan",
        }
    }
}

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
pub struct TermPreparationContext {
    pub source_term_id: Uuid,
    pub source_year_id: Uuid,
    pub source_label: String,
    pub source_status: String,
    pub target_term_id: Uuid,
    pub target_year_id: Uuid,
    pub target_label: String,
    pub target_status: String,
    pub target_start_date: NaiveDate,
    pub target_end_date: NaiveDate,
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
pub struct TermPreparationModuleOutcome {
    pub module: TermPreparationModule,
    pub created_count: usize,
    pub target_ids: Vec<Uuid>,
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
