use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::modules::academic::core::models::BellSchedulePeriod;
use crate::modules::academic::delivery::models::ActivitySchedulingMode;
use crate::modules::academic::models::timetable_version::TimetableVersion;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableInstructor {
    pub user_id: Uuid,
    pub display_name: String,
    pub role: String,
    pub subject_group_id: Option<Uuid>,
    pub subject_group_name: Option<String>,
    pub subject_group_display_order: Option<i32>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableEntry {
    pub id: Uuid,
    pub timetable_version_id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub bell_schedule_id: Uuid,
    pub bell_schedule_period_id: Uuid,
    pub day_of_week: String,
    pub entry_type: String,
    pub learning_group_id: Option<Uuid>,
    pub offering_id: Option<Uuid>,
    pub homeroom_id: Option<Uuid>,
    pub room_id: Option<Uuid>,
    pub note: Option<String>,
    pub title: Option<String>,
    pub batch_id: Option<Uuid>,
    pub row_version: i64,
    pub is_active: bool,
    pub offering_code: Option<String>,
    pub offering_name: Option<String>,
    pub learning_group_code: Option<String>,
    pub learning_group_name: Option<String>,
    /// Stable catalog identity. A selected version is represented by its display label only.
    pub subject_id: Option<Uuid>,
    pub subject_group_id: Option<Uuid>,
    pub subject_group_name: Option<String>,
    pub subject_group_display_order: Option<i32>,
    pub subject_version_display_label: Option<String>,
    pub activity_id: Option<Uuid>,
    pub activity_version_display_label: Option<String>,
    pub activity_scheduling_mode: Option<ActivitySchedulingMode>,
    pub homeroom_name: Option<String>,
    pub room_code: Option<String>,
    pub period_name: Option<String>,
    #[schema(value_type = String)]
    pub start_time: NaiveTime,
    #[schema(value_type = String)]
    pub end_time: NaiveTime,
    pub instructors: Vec<TimetableInstructor>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateTimetableEntryRequest {
    pub timetable_version_id: Uuid,
    pub academic_term_id: Uuid,
    pub learning_group_id: Option<Uuid>,
    pub homeroom_id: Option<Uuid>,
    pub day_of_week: String,
    pub bell_schedule_period_id: Uuid,
    pub room_id: Option<Uuid>,
    pub note: Option<String>,
    pub entry_type: String,
    pub title: Option<String>,
    #[serde(default)]
    pub instructor_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateTimetableEntryRequest {
    pub timetable_version_id: Uuid,
    pub row_version: i64,
    pub day_of_week: Option<String>,
    pub bell_schedule_period_id: Option<Uuid>,
    pub room_id: Option<Uuid>,
    pub clear_room: Option<bool>,
    pub note: Option<String>,
    pub clear_note: Option<bool>,
    pub title: Option<String>,
    pub instructor_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteTimetableEntryQuery {
    pub timetable_version_id: Uuid,
    pub row_version: i64,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableBatchMutationQuery {
    pub timetable_version_id: Uuid,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableQuery {
    pub timetable_version_id: Uuid,
    pub academic_term_id: Uuid,
    pub learning_group_id: Option<Uuid>,
    pub homeroom_id: Option<Uuid>,
    pub instructor_id: Option<Uuid>,
    pub room_id: Option<Uuid>,
    pub day_of_week: Option<String>,
    pub entry_type: Option<String>,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableWorkspaceQuery {
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub timetable_version_id: Uuid,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableWorkspace {
    pub version: TimetableVersion,
    pub bell_periods: Vec<BellSchedulePeriod>,
    pub entries: Vec<TimetableEntry>,
    pub learning_groups: Vec<TimetableWorkspaceLearningGroup>,
    pub homerooms: Vec<TimetableWorkspaceHomeroom>,
    pub rooms: Vec<TimetableWorkspaceRoom>,
    pub staff: Vec<TimetableWorkspaceStaff>,
    pub unscheduled_demands: Vec<TimetableUnscheduledDemand>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableWorkspaceLearningGroup {
    pub id: Uuid,
    pub learning_offering_id: Uuid,
    pub code: String,
    pub name: String,
    pub status: String,
    pub roster_status: String,
    pub offering_code: String,
    pub offering_name: String,
    pub homeroom_ids: Vec<Uuid>,
    pub eligible_instructor_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableWorkspaceHomeroom {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub grade_level_id: Uuid,
    pub grade_level_type: String,
    pub grade_level_year: i32,
    #[schema(required = true)]
    pub room_number: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableWorkspaceRoom {
    pub id: Uuid,
    #[schema(required = true)]
    pub code: Option<String>,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableWorkspaceStaff {
    pub id: Uuid,
    pub display_name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableUnscheduledDemand {
    pub learning_group_id: Uuid,
    pub learning_offering_id: Uuid,
    pub offering_code: String,
    pub offering_name: String,
    pub required_periods: i32,
    pub scheduled_periods: i32,
    pub remaining_periods: i32,
    pub homeroom_ids: Vec<Uuid>,
    pub eligible_instructor_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PersonalTimetableQuery {
    pub academic_term_id: Uuid,
    pub date: NaiveDate,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateBatchTimetableEntriesRequest {
    pub timetable_version_id: Uuid,
    pub academic_term_id: Uuid,
    #[serde(default)]
    pub learning_group_ids: Vec<Uuid>,
    #[serde(default)]
    pub homeroom_ids: Vec<Uuid>,
    pub days_of_week: Vec<String>,
    pub bell_schedule_period_ids: Vec<Uuid>,
    pub entry_type: String,
    pub title: Option<String>,
    pub room_id: Option<Uuid>,
    pub note: Option<String>,
    #[serde(default)]
    pub instructor_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct BatchTimetableResult {
    pub batch_id: Uuid,
    pub entries: Vec<TimetableEntry>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SwapTimetableEntriesRequest {
    pub timetable_version_id: Uuid,
    pub entry_a_id: Uuid,
    pub entry_a_row_version: i64,
    pub entry_b_id: Uuid,
    pub entry_b_row_version: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SwapTimetableEntriesResponse {
    pub entry_a: TimetableEntry,
    pub entry_b: TimetableEntry,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableOccupancyQuery {
    pub timetable_version_id: Uuid,
    pub academic_term_id: Uuid,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableOccupancyCell {
    pub entry_id: Uuid,
    pub learning_group_id: Option<Uuid>,
    pub homeroom_ids: Vec<Uuid>,
    pub room_id: Option<Uuid>,
    pub instructor_ids: Vec<Uuid>,
    pub day_of_week: String,
    pub bell_schedule_period_id: Uuid,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ValidateMovesRequest {
    pub timetable_version_id: Uuid,
    pub academic_term_id: Uuid,
    pub entry_id: Uuid,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MoveValidityCell {
    pub day_of_week: String,
    pub bell_schedule_period_id: Uuid,
    pub state: String,
    pub target_entry_id: Option<Uuid>,
    pub valid: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ConflictInfo {
    pub conflict_type: String,
    pub message: String,
    pub existing_entry_id: Uuid,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableValidationResponse {
    pub is_valid: bool,
    pub conflicts: Vec<ConflictInfo>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableTemplate {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableTemplateEntry {
    pub id: Uuid,
    pub template_id: Uuid,
    pub day_of_week: String,
    pub bell_period_order_index: i32,
    pub entry_type: String,
    pub title: Option<String>,
    pub resource_kind: String,
    pub stable_resource_id: Option<Uuid>,
    pub learning_group_code: Option<String>,
    pub target_selector: TimetableTemplateTargetSelector,
    pub instructor_ids: Vec<Uuid>,
    pub room_id: Option<Uuid>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableTemplateTargetSelector {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grade_level_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub study_program_id: Option<Uuid>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_number: Option<String>,
    #[serde(default)]
    pub instructor_only: bool,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateTemplateRequest {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateTemplateRequest {
    pub name: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FromCurrentRequest {
    pub timetable_version_id: Uuid,
    pub academic_term_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub entry_types: Option<Vec<String>>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyTemplateRequest {
    pub timetable_version_id: Uuid,
    pub academic_term_id: Uuid,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClearTimetableRequest {
    pub timetable_version_id: Uuid,
    pub academic_term_id: Uuid,
    pub entry_types: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TemplateWithEntries {
    pub template: TimetableTemplate,
    pub entries: Vec<TimetableTemplateEntry>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TemplateApplyResult {
    pub applied: usize,
    pub entry_ids: Vec<Uuid>,
}
