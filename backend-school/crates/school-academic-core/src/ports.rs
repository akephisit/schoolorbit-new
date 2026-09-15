use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

/// Minimal pending-work evidence returned by an academic provider to Lifecycle.
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PendingTermWork {
    pub id: Uuid,
    pub revision: String,
    pub blocks_closure: bool,
}

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
pub struct TermPreparationModuleOutcome {
    pub module: TermPreparationModule,
    pub created_count: usize,
    pub target_ids: Vec<Uuid>,
}
