use chrono::{DateTime, NaiveTime, Utc};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::modules::academic::core::models::BellSchedulePeriod;
use crate::modules::academic::delivery::models::ActivitySchedulingMode;
use crate::modules::academic::models::timetable_version::TimetableVersion;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimetableBlockKind {
    Course,
    Activity,
    Structural,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimetableStructuralKind {
    Break,
    Homeroom,
    FlagCeremony,
    TeacherMeeting,
    Academic,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimetableTargetKind {
    Group,
    Homeroom,
    Teacher,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimetableBlockSyncStatus {
    Linked,
    WaitingForData,
    Conflict,
    OutsideScope,
    Excluded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimetableBlockConflictType {
    LearningGroup,
    Homeroom,
    Teacher,
    Room,
    Version,
    StaleBlock,
    MissingInstructor,
    OutsideScope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimetableBlockPlacementState {
    Source,
    Move,
    Swap,
    Blocked,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimetableBlockMutationKind {
    Create,
    Update,
    Move,
    Swap,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableBlockInstructor {
    pub teacher_id: Uuid,
    pub display_name: String,
    pub role: String,
    pub order_index: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableBlockGroup {
    pub id: Uuid,
    pub learning_group_id: Uuid,
    pub learning_offering_id: Uuid,
    pub code: String,
    pub name: String,
    pub homeroom_ids: Vec<Uuid>,
    #[schema(required = true)]
    pub room_id: Option<Uuid>,
    #[schema(required = true)]
    pub room_code: Option<String>,
    pub instructors: Vec<TimetableBlockInstructor>,
    #[schema(required = true)]
    pub sync_status: Option<TimetableBlockSyncStatus>,
    pub row_version: i64,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableBlockHomeroom {
    pub id: Uuid,
    pub homeroom_id: Uuid,
    pub code: String,
    pub name: String,
    #[schema(required = true)]
    pub room_id: Option<Uuid>,
    #[schema(required = true)]
    pub room_code: Option<String>,
    pub row_version: i64,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableBlockTeacher {
    pub id: Uuid,
    pub teacher_id: Uuid,
    pub display_name: String,
    pub row_version: i64,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableBlockSyncState {
    pub id: Uuid,
    pub learning_group_id: Uuid,
    pub learning_offering_id: Uuid,
    pub status: TimetableBlockSyncStatus,
    #[schema(required = true)]
    pub linked_block_group_id: Option<Uuid>,
    #[schema(required = true)]
    pub conflict_code: Option<String>,
    #[schema(required = true)]
    pub conflict_message: Option<String>,
    #[schema(required = true)]
    pub attempted_group_row_version: Option<i64>,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableBlock {
    pub id: Uuid,
    pub timetable_version_id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub bell_schedule_id: Uuid,
    pub bell_schedule_period_id: Uuid,
    pub period_name: String,
    #[schema(value_type = String)]
    pub start_time: NaiveTime,
    #[schema(value_type = String)]
    pub end_time: NaiveTime,
    pub day_of_week: String,
    pub block_kind: TimetableBlockKind,
    #[schema(required = true)]
    pub scheduling_mode: Option<ActivitySchedulingMode>,
    #[schema(required = true)]
    pub learning_offering_id: Option<Uuid>,
    #[schema(required = true)]
    pub offering_code: Option<String>,
    #[schema(required = true)]
    pub offering_name: Option<String>,
    #[schema(required = true)]
    pub structural_kind: Option<TimetableStructuralKind>,
    #[schema(required = true)]
    pub title: Option<String>,
    #[schema(required = true)]
    pub note: Option<String>,
    #[schema(required = true)]
    pub series_id: Option<Uuid>,
    pub groups: Vec<TimetableBlockGroup>,
    pub homerooms: Vec<TimetableBlockHomeroom>,
    pub teachers: Vec<TimetableBlockTeacher>,
    pub sync_states: Vec<TimetableBlockSyncState>,
    pub row_version: i64,
    pub is_active: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableBlockWorkspaceLearningGroup {
    pub id: Uuid,
    pub learning_offering_id: Uuid,
    pub code: String,
    pub name: String,
    pub status: String,
    pub roster_status: String,
    pub offering_kind: String,
    pub offering_code: String,
    pub offering_name: String,
    pub homeroom_ids: Vec<Uuid>,
    pub eligible_instructors: Vec<TimetableBlockInstructor>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableBlockWorkspaceHomeroom {
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
pub struct TimetableBlockWorkspaceRoom {
    pub id: Uuid,
    #[schema(required = true)]
    pub code: Option<String>,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableBlockWorkspaceStaff {
    pub id: Uuid,
    pub display_name: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableOrdinaryDemand {
    pub learning_group_id: Uuid,
    pub learning_offering_id: Uuid,
    pub offering_code: String,
    pub offering_name: String,
    pub required_periods: i32,
    pub scheduled_periods: i32,
    pub remaining_periods: i32,
    pub homeroom_ids: Vec<Uuid>,
    pub eligible_instructors: Vec<TimetableBlockInstructor>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableSynchronizedDemand {
    pub learning_offering_id: Uuid,
    pub offering_code: String,
    pub offering_name: String,
    pub required_periods: i32,
    pub scheduled_periods: i32,
    pub intended_homeroom_ids: Vec<Uuid>,
    pub linked_group_count: i32,
    pub pending_group_count: i32,
    pub conflict_group_count: i32,
    pub excluded_group_count: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableBlockSummary {
    pub block_count: i32,
    pub ordinary_demand_count: i32,
    pub synchronized_demand_count: i32,
    pub linked_group_count: i32,
    pub waiting_group_count: i32,
    pub conflict_group_count: i32,
    pub excluded_group_count: i32,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableBlockWorkspace {
    pub version: TimetableVersion,
    pub bell_periods: Vec<BellSchedulePeriod>,
    pub blocks: Vec<TimetableBlock>,
    pub ordinary_demands: Vec<TimetableOrdinaryDemand>,
    pub synchronized_demands: Vec<TimetableSynchronizedDemand>,
    pub learning_groups: Vec<TimetableBlockWorkspaceLearningGroup>,
    pub homerooms: Vec<TimetableBlockWorkspaceHomeroom>,
    pub rooms: Vec<TimetableBlockWorkspaceRoom>,
    pub staff: Vec<TimetableBlockWorkspaceStaff>,
    pub summary: TimetableBlockSummary,
}

#[derive(Debug, Clone, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableBlockWorkspaceQuery {
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub timetable_version_id: Uuid,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateOrdinaryTimetableBlockRequest {
    pub timetable_version_id: Uuid,
    pub academic_term_id: Uuid,
    pub learning_group_id: Uuid,
    pub day_of_week: String,
    pub bell_schedule_period_id: Uuid,
    #[serde(deserialize_with = "deserialize_required_nullable_uuid")]
    #[schema(required = true)]
    pub room_id: Option<Uuid>,
    pub instructor_ids: Vec<Uuid>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateSynchronizedTimetableBlockRequest {
    pub timetable_version_id: Uuid,
    pub academic_term_id: Uuid,
    pub learning_offering_id: Uuid,
    pub day_of_week: String,
    pub bell_schedule_period_id: Uuid,
    pub intended_homeroom_ids: Vec<Uuid>,
    #[serde(deserialize_with = "deserialize_required_nullable_uuid")]
    #[schema(required = true)]
    pub room_id: Option<Uuid>,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableStructuralSlotInput {
    pub day_of_week: String,
    pub bell_schedule_period_id: Uuid,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateStructuralTimetableBlocksRequest {
    pub timetable_version_id: Uuid,
    pub academic_term_id: Uuid,
    pub structural_kind: TimetableStructuralKind,
    pub title: String,
    #[serde(default)]
    pub note: Option<String>,
    pub slots: Vec<TimetableStructuralSlotInput>,
    #[serde(default)]
    pub homeroom_ids: Vec<Uuid>,
    #[serde(default)]
    pub teacher_ids: Vec<Uuid>,
    #[serde(default)]
    pub all_homerooms: bool,
    #[serde(default)]
    pub all_teachers: bool,
    #[serde(deserialize_with = "deserialize_required_nullable_uuid")]
    #[schema(required = true)]
    pub room_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateTimetableBlockRequest {
    pub timetable_version_id: Uuid,
    pub row_version: i64,
    #[serde(default)]
    pub day_of_week: Option<String>,
    #[serde(default)]
    pub bell_schedule_period_id: Option<Uuid>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub clear_title: bool,
    #[serde(default)]
    pub note: Option<String>,
    #[serde(default)]
    pub clear_note: bool,
    #[serde(default)]
    pub room_id: Option<Uuid>,
    #[serde(default)]
    pub clear_room: bool,
    #[serde(default)]
    pub instructor_ids: Option<Vec<Uuid>>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoveTimetableBlockTargetRequest {
    pub timetable_version_id: Uuid,
    pub block_row_version: i64,
    pub target_kind: TimetableTargetKind,
    pub target_id: Uuid,
    pub target_row_version: i64,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RetryTimetableBlockSyncRequest {
    pub timetable_version_id: Uuid,
    pub block_row_version: i64,
    #[serde(default)]
    pub learning_group_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RestoreTimetableBlockGroupRequest {
    pub timetable_version_id: Uuid,
    pub block_row_version: i64,
    pub learning_group_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum TimetableBlockPlacementSource {
    ExistingBlock {
        #[serde(rename = "blockId")]
        block_id: Uuid,
        #[serde(rename = "rowVersion")]
        row_version: i64,
    },
    OrdinaryDemand {
        #[serde(rename = "learningGroupId")]
        learning_group_id: Uuid,
        #[serde(rename = "learningOfferingId")]
        learning_offering_id: Uuid,
    },
    SynchronizedOffering {
        #[serde(rename = "learningOfferingId")]
        learning_offering_id: Uuid,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableBlockPlacementCandidate {
    pub block_kind: TimetableBlockKind,
    #[serde(deserialize_with = "deserialize_required_nullable_uuid")]
    #[schema(required = true)]
    pub learning_group_id: Option<Uuid>,
    #[serde(deserialize_with = "deserialize_required_nullable_uuid")]
    #[schema(required = true)]
    pub learning_offering_id: Option<Uuid>,
    #[serde(deserialize_with = "deserialize_required_nullable_uuid")]
    #[schema(required = true)]
    pub room_id: Option<Uuid>,
    #[serde(default)]
    pub homeroom_ids: Vec<Uuid>,
    #[serde(default)]
    pub teacher_ids: Vec<Uuid>,
    #[serde(default)]
    pub instructor_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimetableBlockPlacementPreviewRequest {
    pub timetable_version_id: Uuid,
    pub academic_term_id: Uuid,
    pub source: TimetableBlockPlacementSource,
    pub candidate: TimetableBlockPlacementCandidate,
    pub target_day_of_week: String,
    pub target_bell_schedule_period_id: Uuid,
    #[serde(default)]
    pub expected_target_block_id: Option<Uuid>,
    #[serde(default)]
    pub expected_target_row_version: Option<i64>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableBlockConflict {
    pub conflict_type: TimetableBlockConflictType,
    pub code: String,
    pub message: String,
    #[schema(required = true)]
    pub existing_block_id: Option<Uuid>,
    #[schema(required = true)]
    pub target_kind: Option<TimetableTargetKind>,
    #[schema(required = true)]
    pub target_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TimetableBlockPlacementPreview {
    pub state: TimetableBlockPlacementState,
    #[schema(required = true)]
    pub source_block_id: Option<Uuid>,
    #[schema(required = true)]
    pub target_block_id: Option<Uuid>,
    pub target_day_of_week: String,
    pub target_bell_schedule_period_id: Uuid,
    pub normalized_candidate: TimetableBlockPlacementCandidate,
    pub conflicts: Vec<TimetableBlockConflict>,
    #[schema(required = true)]
    pub mutation: Option<TimetableBlockMutationKind>,
}

#[derive(Debug, Clone, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SwapTimetableBlocksRequest {
    pub timetable_version_id: Uuid,
    pub block_a_id: Uuid,
    pub block_a_row_version: i64,
    pub block_b_id: Uuid,
    pub block_b_row_version: i64,
}

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SwapTimetableBlocksResponse {
    pub block_a: TimetableBlock,
    pub block_b: TimetableBlock,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteTimetableBlockQuery {
    pub timetable_version_id: Uuid,
    pub row_version: i64,
}

#[derive(Debug, Clone, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteTimetableBlockSeriesQuery {
    pub timetable_version_id: Uuid,
}

fn deserialize_required_nullable_uuid<'de, D>(deserializer: D) -> Result<Option<Uuid>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Option::<Uuid>::deserialize(deserializer)
}
