use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, ToSchema, IntoParams)]
#[serde(rename_all = "camelCase")]
#[into_params(parameter_in = Query)]
pub struct GradebookContext {
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ItemInput {
    pub name: String,
    pub max_score: String,
    pub display_order: i32,
    pub row_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(
    tag = "operation",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum ScoreCellMutation {
    Set {
        score_item_id: Uuid,
        student_academic_year_id: Uuid,
        value: String,
        row_version: Option<i64>,
    },
    Clear {
        score_item_id: Uuid,
        student_academic_year_id: Uuid,
        row_version: Option<i64>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmInput {
    pub source_checksum: String,
    pub roster_checksum: String,
    pub row_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ScoreItem {
    pub id: Uuid,
    pub name: String,
    pub max_score: String,
    pub display_order: i32,
    pub lifecycle: String,
    pub row_version: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScoreItemRemovalDisposition {
    Deleted,
    Cancelled,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScoreItemRemovalOutcome {
    pub disposition: ScoreItemRemovalDisposition,
    pub item_id: Uuid,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct ScoreCell {
    pub score_item_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub value: Option<String>,
    pub row_version: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct GradebookStudent {
    pub membership_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub display_name: String,
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct PhaseConfirmation {
    pub id: Uuid,
    pub blank_score_count: i64,
    pub source_checksum: String,
    pub roster_checksum: String,
    pub row_version: i64,
    pub invalidated: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GroupPhaseWorkspace {
    pub learning_group_id: Uuid,
    pub learning_offering_id: Uuid,
    pub assessment_phase_id: Uuid,
    pub phase_code: String,
    pub phase_max_score: String,
    pub phase_row_version: i64,
    pub score_entry_enabled: bool,
    pub locked: bool,
    pub can_manage: bool,
    pub can_confirm: bool,
    pub items: Vec<ScoreItem>,
    pub students: Vec<GradebookStudent>,
    pub scores: Vec<ScoreCell>,
    pub source_checksum: String,
    pub roster_checksum: String,
    pub confirmation: Option<PhaseConfirmation>,
    pub confirmation_is_current: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScoreBatchOutcome {
    pub cells: Vec<ScoreCell>,
    pub workspace_revision: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct GradebookControl {
    pub id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub phase_code: String,
    pub score_entry_enabled: bool,
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UpdateControlInput {
    pub score_entry_enabled: bool,
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct GradebookSubject {
    pub learning_offering_id: Uuid,
    pub subject_id: Uuid,
    pub code: String,
    pub name: String,
    pub learning_group_id: Uuid,
    pub group_name: String,
    pub assigned: bool,
    #[sqlx(json)]
    pub phases: Vec<GradebookPhaseSummary>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GradebookPhaseSummary {
    pub id: Uuid,
    pub phase_code: String,
    pub max_score: String,
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RemoveItemInput {
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScoreBatchInput {
    pub cells: Vec<ScoreCellMutation>,
}
