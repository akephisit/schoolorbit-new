use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

macro_rules! text_enum {
    ($name:ident {$($variant:ident => $value:literal),+ $(,)?}) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema, sqlx::Type)]
        #[serde(rename_all="snake_case")]
        #[sqlx(type_name="text",rename_all="snake_case")]
        pub enum $name { $($variant),+ }
        impl $name { pub fn code(self)-> &'static str { match self { $(Self::$variant=>$value),+ } } }
    }
}
text_enum!(BehaviorSide { Merit=>"merit", Deduction=>"deduction" });
text_enum!(BehaviorSource { Manual=>"manual", Arrival=>"arrival", Lesson=>"lesson", Special=>"special" });
text_enum!(BehaviorScoreAction { None=>"none", Merit=>"merit", Deduction=>"deduction" });
text_enum!(BehaviorResetSchedule { Manual=>"manual", Monthly=>"monthly", Term=>"term", Year=>"year" });
text_enum!(BehaviorCleanupMode { DetailsOnly=>"details_only", Archive=>"archive", DeleteEffects=>"delete_effects" });

#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorItem {
    pub id: Uuid,
    pub name: String,
    pub side: BehaviorSide,
    pub enabled: bool,
    pub row_version: i64,
    #[schema(required = true)]
    pub version_id: Option<Uuid>,
    #[schema(required = true)]
    pub effective_from: Option<NaiveDate>,
    #[schema(required = true)]
    pub points: Option<i32>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveBehaviorItem {
    pub request_id: Uuid,
    pub id: Option<Uuid>,
    pub row_version: Option<i64>,
    pub name: String,
    pub side: BehaviorSide,
    pub enabled: bool,
    pub effective_from: NaiveDate,
    pub points: i32,
}
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in=Query)]
pub struct BehaviorItemsQuery {
    pub date: Option<NaiveDate>,
    pub include_disabled: Option<bool>,
}
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorAttendanceRule {
    pub id: Uuid,
    pub kind: BehaviorSource,
    pub status: String,
    #[schema(required = true)]
    pub grade_level_id: Option<Uuid>,
    pub effective_from: NaiveDate,
    pub action: BehaviorScoreAction,
    pub points: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveBehaviorAttendanceRule {
    pub kind: BehaviorSource,
    pub status: String,
    pub grade_level_id: Option<Uuid>,
    pub effective_from: NaiveDate,
    pub action: BehaviorScoreAction,
    pub points: i32,
}
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorSettings {
    pub enabled: bool,
    pub count_absent_lessons: bool,
    pub count_special: bool,
    #[schema(required = true)]
    pub daily_merit_cap: Option<i32>,
    #[schema(required = true)]
    pub daily_deduction_cap: Option<i32>,
    pub merit_reset: BehaviorResetSchedule,
    pub deduction_reset: BehaviorResetSchedule,
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveBehaviorSettings {
    pub settings: BehaviorSettings,
}
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorCycle {
    pub id: Uuid,
    pub side: BehaviorSide,
    pub starts_on: NaiveDate,
    pub starts_at: DateTime<Utc>,
    #[schema(required = true)]
    pub ends_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub ends_on: Option<NaiveDate>,
    pub state: String,
    pub label: String,
}
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in=Query)]
pub struct BehaviorStudentsQuery {
    pub academic_term_id: Uuid,
    pub date: NaiveDate,
    pub grade_level_id: Option<Uuid>,
    pub homeroom_id: Option<Uuid>,
    pub search: Option<String>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
}
#[derive(Debug, Clone, Serialize, Deserialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorStudent {
    pub id: Uuid,
    pub student_academic_year_id: Uuid,
    pub name: String,
    #[schema(required = true)]
    pub student_code: Option<String>,
    pub grade_level_id: Uuid,
    pub grade_level: String,
    #[schema(required = true)]
    pub homeroom_id: Option<Uuid>,
    pub homeroom: String,
    #[schema(required = true)]
    pub class_number: Option<i32>,
}
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorStudentPage {
    pub items: Vec<BehaviorStudent>,
    pub total: i64,
    pub roster_token: String,
    pub options: BehaviorSelectorOptions,
}
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorSelectorOption {
    pub id: Uuid,
    pub label: String,
}
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorSelectorOptions {
    pub grade_levels: Vec<BehaviorSelectorOption>,
    pub homerooms: Vec<BehaviorSelectorOption>,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateBehaviorBatch {
    pub request_id: Uuid,
    pub academic_term_id: Uuid,
    pub item_id: Uuid,
    pub item_row_version: i64,
    pub item_version_id: Uuid,
    pub occurred_at: DateTime<Utc>,
    pub detail: Option<String>,
    pub student_ids: Vec<Uuid>,
    pub roster_token: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorBatchReceipt {
    pub id: Uuid,
    pub affected_students: i64,
    pub side: BehaviorSide,
    pub points_per_student: i32,
    pub replayed: bool,
    pub details_available: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelBehaviorEntries {
    pub request_id: Uuid,
    pub entry_ids: Vec<Uuid>,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorCancellation {
    pub cancelled: i64,
    pub already_cancelled: i64,
}
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in=Query)]
pub struct BehaviorReportQuery {
    pub start: NaiveDate,
    pub end: NaiveDate,
    pub academic_term_id: Option<Uuid>,
    pub student_id: Option<Uuid>,
    pub homeroom_id: Option<Uuid>,
    pub grade_level_id: Option<Uuid>,
    pub side: Option<BehaviorSide>,
    pub source: Option<BehaviorSource>,
    pub search: Option<String>,
    pub batch_id: Option<Uuid>,
    pub created_by: Option<Uuid>,
    pub mine: Option<bool>,
    pub cancelled: Option<bool>,
    pub offset: Option<i64>,
    pub limit: Option<i64>,
}
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorReportEntry {
    pub id: Uuid,
    pub student_id: Uuid,
    pub student_name: String,
    #[schema(required = true)]
    pub student_code: Option<String>,
    #[schema(required = true)]
    pub homeroom_id: Option<Uuid>,
    pub homeroom: String,
    pub grade_level_id: Uuid,
    pub date: NaiveDate,
    pub academic_term_id: Uuid,
    pub side: BehaviorSide,
    pub source: BehaviorSource,
    pub points: i32,
    pub item_name: String,
    #[schema(required = true)]
    pub detail: Option<String>,
    #[schema(required = true)]
    pub occurred_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub batch_id: Option<Uuid>,
    #[schema(required = true)]
    pub source_key: Option<String>,
    #[schema(required = true)]
    pub source_session_id: Option<Uuid>,
    #[schema(required = true)]
    pub created_by: Option<Uuid>,
    #[schema(required = true)]
    pub creator_name: Option<String>,
    pub created_at: DateTime<Utc>,
    #[schema(required = true)]
    pub cancelled_at: Option<DateTime<Utc>>,
    #[schema(required = true)]
    pub cancellation_reason: Option<String>,
    pub can_cancel: bool,
}
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorReportPage {
    pub items: Vec<BehaviorReportEntry>,
    pub total: i64,
}
#[derive(Debug, Clone, Deserialize, IntoParams)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in=Query)]
pub struct BehaviorSummaryQuery {
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
    pub academic_term_id: Option<Uuid>,
}
#[derive(Debug, Clone, Default, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorScoreTotals {
    pub merit: i64,
    pub deduction: i64,
    pub net: i64,
}
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorCurrentTotals {
    pub merit: i64,
    pub deduction: i64,
    #[schema(required = true)]
    pub net: Option<i64>,
}
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorStudentSummary {
    pub student_id: Uuid,
    pub current: BehaviorCurrentTotals,
    pub history: BehaviorScoreTotals,
    pub lifetime: BehaviorScoreTotals,
    pub comparable_current: BehaviorCurrentTotals,
    pub comparison_exact: bool,
    pub comparison_start: NaiveDate,
    pub comparison_end: NaiveDate,
    pub merit_cycle: BehaviorCycle,
    pub deduction_cycle: BehaviorCycle,
    pub history_totals: Vec<BehaviorDailyTotal>,
}
#[derive(Debug, Clone, Serialize, FromRow, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorDailyTotal {
    pub id: Uuid,
    pub student_id: Uuid,
    pub date: NaiveDate,
    pub academic_term_id: Uuid,
    pub grade_level_id: Uuid,
    #[schema(required = true)]
    pub homeroom_id: Option<Uuid>,
    pub side: BehaviorSide,
    pub source: BehaviorSource,
    pub cycle_id: Uuid,
    pub points: i64,
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BehaviorCleanupScope {
    pub start: Option<NaiveDate>,
    pub end: Option<NaiveDate>,
    pub academic_year_id: Option<Uuid>,
    pub academic_term_id: Option<Uuid>,
    pub sides: Vec<BehaviorSide>,
    pub sources: Vec<BehaviorSource>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewBehaviorCleanup {
    pub scope: BehaviorCleanupScope,
    pub mode: BehaviorCleanupMode,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorCleanupPreview {
    pub token: Uuid,
    pub revision: i64,
    pub students: i64,
    pub entries: i64,
    pub merit: i64,
    pub deduction: i64,
    pub current: BehaviorScoreTotals,
    pub history: BehaviorScoreTotals,
    pub lifetime: BehaviorScoreTotals,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExecuteBehaviorCleanup {
    pub request_id: Uuid,
    pub token: Uuid,
    pub revision: i64,
    pub scope: BehaviorCleanupScope,
    pub mode: BehaviorCleanupMode,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorCleanupOutcome {
    pub id: Uuid,
    pub status: String,
    pub students: i64,
    pub entries_deleted: i64,
    pub merit: i64,
    pub deduction: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdjustBehaviorHistory {
    pub request_id: Uuid,
    pub total_id: Uuid,
    pub row_version: i64,
    pub delta: i64,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorHistoryAdjustment {
    pub id: Uuid,
    pub total_id: Uuid,
    pub points: i64,
    pub row_version: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewBehaviorReset {
    pub sides: Vec<BehaviorSide>,
    pub through: NaiveDate,
}
#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResetBehaviorScores {
    pub request_id: Uuid,
    pub token: Uuid,
    pub revision: i64,
    pub sides: Vec<BehaviorSide>,
    pub through: NaiveDate,
    pub reason: String,
}
/// Complete canonical attendance state for one school civil day. Empty facts reverse
/// all still-editable automatic contributions for that generation, term and day.
#[derive(Debug, Clone)]
pub struct BehaviorAttendanceDay {
    pub generation: Uuid,
    pub academic_term_id: Uuid,
    pub date: NaiveDate,
    pub revision: i64,
    pub facts: Vec<BehaviorAttendanceFact>,
}
#[derive(Debug, Clone)]
pub struct BehaviorAttendanceFact {
    pub source_key: String,
    pub source_session_id: Option<Uuid>,
    pub kind: BehaviorSource,
    pub student_id: Uuid,
    pub status: String,
    pub counted: bool,
    pub grade_level_id: Uuid,
    pub homeroom_id: Option<Uuid>,
    pub overrides: Option<Vec<BehaviorScoreOverride>>,
}
#[derive(Debug, Clone)]
pub struct BehaviorScoreOverride {
    pub status: String,
    pub action: BehaviorScoreAction,
    pub points: i32,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BehaviorDeliveryOutcome {
    Applied,
    AlreadyApplied,
    Superseded,
    Suppressed,
}
#[derive(Debug, Clone)]
pub struct BehaviorResetReadiness {
    pub finalized_through: NaiveDate,
    pub delivered_through: NaiveDate,
    pub term_ends: Vec<NaiveDate>,
    pub year_ends: Vec<NaiveDate>,
}
#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BehaviorOverview {
    pub cycles: Vec<BehaviorCycle>,
    pub activation_date: NaiveDate,
}
