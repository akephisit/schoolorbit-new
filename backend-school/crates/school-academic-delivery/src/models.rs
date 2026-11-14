use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
#[cfg(feature = "api-docs")]
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

pub mod current;

use school_academic_core::models::HomeroomLookupItem;
use school_academic_core::models::{GradeLevelLookupItem, RequirementKind, StudyProgramOption};

/// Staff lookup item with title
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct StaffLookupItem {
    pub id: Uuid,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "api-docs", schema(value_type = String))]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
pub struct Room {
    pub id: Uuid,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub building_id: Option<Uuid>,
    pub name_th: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub name_en: Option<String>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub code: Option<String>,
    pub room_type: String,
    pub capacity: i32,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub floor: Option<i32>,
    pub status: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub description: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[sqlx(default)]
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub building_name: Option<String>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, sqlx::Type)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum LearningOfferingKind {
    Course,
    Activity,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, sqlx::Type)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum OfferingTargetKind {
    Homeroom,
    GradeProgram,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, sqlx::Type)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum ActivityRegistrationType {
    #[serde(rename = "self")]
    #[sqlx(rename = "self")]
    SelfRegistration,
    Assigned,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, sqlx::Type)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum ActivitySchedulingMode {
    Synchronized,
    Independent,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, sqlx::Type)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum LearningTeacherRole {
    Primary,
    Secondary,
    Assistant,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, sqlx::Type)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum MembershipStatus {
    Active,
    Ended,
    Removed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum RosterOverrideAction {
    Add,
    Remove,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum CurriculumPreviewAction {
    Create,
    Retain,
    Conflict,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum PreparationAction {
    Apply,
    Skip,
    DeferGroups,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum PreparationGroupingState {
    Proposed,
    Deferred,
    Conflict,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityAttendanceRequirement {
    #[cfg_attr(feature = "api-docs", schema(value_type = Option<String>, pattern = r"^(0|[1-9]\d*)(\.\d{1,2})?$"))]
    pub minimum_percent: Option<String>,
    pub required_sessions: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityPassCriteria {
    pub require_attendance: bool,
    pub require_teacher_confirmation: bool,
    pub outcomes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfferingTargetInput {
    pub target_kind: OfferingTargetKind,
    pub homeroom_id: Option<Uuid>,
    pub grade_level_id: Uuid,
    pub study_program_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCourseOfferingRequest {
    pub academic_term_id: Uuid,
    pub subject_version_id: Uuid,
    pub curriculum_course_requirement_id: Option<Uuid>,
    pub targets: Vec<OfferingTargetInput>,
    #[cfg_attr(feature = "api-docs", schema(value_type = String, pattern = r"^(0|[1-9]\d*)(\.\d{1,2})?$"))]
    pub assessment_total_score: String,
    pub weekly_period_target: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateActivityOfferingRequest {
    #[serde(default)]
    pub create_homeroom_groups: bool,
    pub academic_term_id: Uuid,
    pub activity_version_id: Uuid,
    pub curriculum_activity_requirement_id: Option<Uuid>,
    pub targets: Vec<OfferingTargetInput>,
    pub registration_type: ActivityRegistrationType,
    pub scheduling_mode: ActivitySchedulingMode,
    pub capacity: Option<i32>,
    pub attendance_requirement: ActivityAttendanceRequirement,
    pub pass_criteria: ActivityPassCriteria,
    pub weekly_period_target: i32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CreateLearningOfferingRequest {
    Course(CreateCourseOfferingRequest),
    Activity(CreateActivityOfferingRequest),
}

impl CreateLearningOfferingRequest {
    pub fn academic_term_id(&self) -> Uuid {
        match self {
            Self::Course(request) => request.academic_term_id,
            Self::Activity(request) => request.academic_term_id,
        }
    }

    pub fn targets(&self) -> &[OfferingTargetInput] {
        match self {
            Self::Course(request) => &request.targets,
            Self::Activity(request) => &request.targets,
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateLearningOfferingRequest {
    pub row_version: i64,
    pub targets: Vec<OfferingTargetInput>,
    pub weekly_period_target: i32,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(IntoParams, ToSchema))]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Query))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LearningOfferingQuery {
    pub academic_term_id: Uuid,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(IntoParams, ToSchema))]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Query))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HomeroomDeliveryQuery {
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(IntoParams, ToSchema))]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Query))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LearningGroupTermQuery {
    pub academic_term_id: Uuid,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(IntoParams, ToSchema))]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Query))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudentActivityRegistrationQuery {
    pub academic_term_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewCurriculumOfferingsRequest {
    pub academic_term_id: Uuid,
    pub study_program_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyCurriculumOfferingsRequest {
    pub academic_term_id: Uuid,
    pub study_program_ids: Vec<Uuid>,
    pub source_hash: String,
    pub idempotency_key: Uuid,
    pub choices: Vec<CurriculumPreparationChoice>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LearningOfferingTarget {
    pub id: Uuid,
    pub target_kind: OfferingTargetKind,
    pub homeroom_id: Option<Uuid>,
    pub grade_level_id: Uuid,
    pub study_program_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CourseOfferingSnapshot {
    pub subject_version_id: Uuid,
    pub subject_id: Uuid,
    pub curriculum_course_requirement_id: Option<Uuid>,
    pub credit: String,
    pub hours: Option<String>,
    pub standard_periods_per_week: i32,
    #[cfg_attr(feature = "api-docs", schema(value_type = String, pattern = r"^(0|[1-9]\d*)(\.\d{1,2})?$"))]
    pub assessment_total_score: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ActivityOfferingSnapshot {
    pub activity_version_id: Uuid,
    pub activity_id: Uuid,
    pub curriculum_activity_requirement_id: Option<Uuid>,
    pub registration_type: ActivityRegistrationType,
    pub scheduling_mode: ActivitySchedulingMode,
    pub hours: String,
    pub capacity: Option<i32>,
    pub attendance_requirement: ActivityAttendanceRequirement,
    pub pass_criteria: ActivityPassCriteria,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct StudentActivityGroupOption {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub capacity: Option<i32>,
    pub member_count: i64,
    pub teacher_names: Vec<String>,
    pub enrolled: bool,
    pub registration_open: bool,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct StudentActivityOfferingOption {
    pub id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub code: String,
    pub name: String,
    pub activity_type: String,
    pub enrolled_group_id: Option<Uuid>,
    pub groups: Vec<StudentActivityGroupOption>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct StudentActivityRegistrationResult {
    pub learning_offering_id: Uuid,
    pub learning_group_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub enrolled: bool,
    pub revision: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LearningOfferingSnapshot {
    Course(CourseOfferingSnapshot),
    Activity(ActivityOfferingSnapshot),
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LearningOffering {
    pub can_manage: bool,
    pub id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub kind: LearningOfferingKind,
    pub code_snapshot: String,
    pub name_snapshot: String,
    pub source_requirement_kind: Option<String>,
    pub source_requirement_id: Option<Uuid>,
    pub owning_organization_unit_id: Uuid,
    pub row_version: i64,
    pub migrated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub snapshot: LearningOfferingSnapshot,
    pub targets: Vec<LearningOfferingTarget>,
    pub weekly_period_target: i32,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LearningOfferingOverviewItem {
    pub offering: LearningOffering,
    pub grade_levels: Vec<GradeLevelLookupItem>,
    pub study_programs: Vec<StudyProgramOption>,
    pub group_count: i64,
    pub teacher_assignment_count: i64,
    pub groups_without_primary_teacher: i64,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LearningDeliveryOverview {
    pub academic_term_id: Uuid,
    pub offerings: Vec<LearningOfferingOverviewItem>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum HomeroomOfferingState {
    Missing,
    Present,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum HomeroomGroupMode {
    Central,
    Missing,
    Normal,
    Combined,
    Split,
    Deferred,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum HomeroomTeacherState {
    Deferred,
    MissingPrimary,
    Assigned,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum CurriculumDeliveryAlignmentState {
    MatchesCurriculum,
    CurriculumRequirementNotOffered,
    ExtraOffering,
    OperationalPeriodsDiffer,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeliveryPrerequisite {
    pub code: String,
    pub message: String,
    pub recovery_path: String,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct UnlinkedDeliveryItem {
    pub offering_id: Uuid,
    pub resource_kind: LearningOfferingKind,
    pub group_id: Option<Uuid>,
    pub code: String,
    pub name: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct HomeroomDeliveryGroupSummary {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub homeroom_ids: Vec<Uuid>,
    pub homeroom_names: Vec<String>,
    pub primary_teacher_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct HomeroomDeliveryItem {
    pub requirement_id: Uuid,
    pub resource_kind: LearningOfferingKind,
    pub catalog_version_id: Uuid,
    pub code: String,
    pub name: String,
    pub requirement_kind: RequirementKind,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub subject_type: Option<String>,
    pub standard_periods_per_week: Option<i32>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub weekly_period_target: Option<i32>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub scheduling_mode: Option<ActivitySchedulingMode>,
    pub offering_id: Option<Uuid>,
    pub offering_state: HomeroomOfferingState,
    pub group_mode: HomeroomGroupMode,
    pub teacher_state: HomeroomTeacherState,
    pub alignment_states: Vec<CurriculumDeliveryAlignmentState>,
    pub groups: Vec<HomeroomDeliveryGroupSummary>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CurriculumDeliveryExtraOffering {
    pub offering_id: Uuid,
    pub resource_kind: LearningOfferingKind,
    pub catalog_version_id: Uuid,
    pub code: String,
    pub name: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub weekly_period_target: Option<i32>,
    pub alignment_states: Vec<CurriculumDeliveryAlignmentState>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct HomeroomDeliveryRoom {
    pub homeroom: HomeroomLookupItem,
    pub grade_level: GradeLevelLookupItem,
    pub study_program: StudyProgramOption,
    pub curriculum_level_id: Uuid,
    pub expected_count: usize,
    pub ready_count: usize,
    pub items: Vec<HomeroomDeliveryItem>,
    pub extra_offerings: Vec<CurriculumDeliveryExtraOffering>,
    pub blockers: Vec<DeliveryPrerequisite>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct HomeroomDeliveryWorkspace {
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub homerooms: Vec<HomeroomDeliveryRoom>,
    pub unlinked: Vec<UnlinkedDeliveryItem>,
    pub row_version: i64,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeliveryCatalogVersionOption {
    pub id: Uuid,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub subject_type: Option<String>,
    pub kind: LearningOfferingKind,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub scheduling_mode: Option<ActivitySchedulingMode>,
    pub code: String,
    pub name: String,
    pub version_no: i32,
    pub label: String,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub standard_periods_per_week: Option<i32>,
}

#[derive(Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeliveryManagementOptions {
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub catalog_versions: Vec<DeliveryCatalogVersionOption>,
    pub grade_levels: Vec<GradeLevelLookupItem>,
    pub study_programs: Vec<StudyProgramOption>,
    pub homerooms: Vec<HomeroomLookupItem>,
    pub learning_groups: Vec<LearningGroup>,
    pub teachers: Vec<StaffLookupItem>,
    pub rooms: Vec<Room>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CurriculumGroupProposal {
    pub group_key: String,
    pub name: String,
    pub homeroom_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PreparationConflict {
    pub code: String,
    pub message: String,
    pub offering_id: Option<Uuid>,
    pub group_id: Option<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CurriculumPreparationProposal {
    pub proposal_id: String,
    pub offering_action: CurriculumPreviewAction,
    pub resource_kind: LearningOfferingKind,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub scheduling_mode: Option<ActivitySchedulingMode>,
    pub catalog_version_id: Uuid,
    pub requirement_ids: Vec<Uuid>,
    pub target_homeroom_ids: Vec<Uuid>,
    pub code: String,
    pub name: String,
    pub credit: Option<String>,
    pub hours: Option<String>,
    pub existing_offering_id: Option<Uuid>,
    pub grouping_state: PreparationGroupingState,
    pub default_groups: Vec<CurriculumGroupProposal>,
    pub conflicts: Vec<PreparationConflict>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CurriculumPreparationChoice {
    pub proposal_id: String,
    pub action: PreparationAction,
    pub groups: Vec<CurriculumGroupProposal>,
}

#[derive(Clone, Debug, Deserialize, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CurriculumPublicationSource {
    pub edition_id: Uuid,
    pub publication_id: Uuid,
    pub publication_no: i32,
    pub revision_year: Option<i32>,
    pub edition_name: String,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CurriculumOfferingPreview {
    pub curriculum_sources: Vec<CurriculumPublicationSource>,
    pub academic_term_id: Uuid,
    pub source_hash: String,
    pub proposals: Vec<CurriculumPreparationProposal>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ApplyCurriculumOfferingsResult {
    pub academic_term_id: Uuid,
    pub source_hash: String,
    pub offering_ids: Vec<Uuid>,
    pub group_ids: Vec<Uuid>,
    pub created_offering_count: usize,
    pub retained_offering_count: usize,
    pub created_group_count: usize,
    pub retained_group_count: usize,
    pub skipped_count: usize,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateLearningGroupRequest {
    pub registration_open: bool,
    pub name: String,
    pub description: Option<String>,
    pub capacity: Option<i32>,
    pub preferred_room_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateLearningGroupRequest {
    pub registration_open: bool,
    pub row_version: i64,
    pub name: String,
    pub description: Option<String>,
    pub capacity: Option<i32>,
    pub preferred_room_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeacherAssignmentInput {
    pub teacher_id: Uuid,
    pub role: LearningTeacherRole,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LearningGroupTeacherAssignment {
    pub is_active: bool,
    pub id: Uuid,
    pub teacher_id: Uuid,
    pub display_name: String,
    pub role: LearningTeacherRole,
    pub row_version: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplaceLearningGroupTeachersRequest {
    pub row_version: i64,
    pub teachers: Vec<TeacherAssignmentInput>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplaceLearningGroupHomeroomsRequest {
    pub row_version: i64,
    pub homeroom_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(transparent)]
pub struct LearningGroupHomeroomIds(pub Vec<Uuid>);

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LearningGroup {
    pub id: Uuid,
    pub learning_offering_id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub capacity: Option<i32>,
    pub row_version: i64,
    pub migrated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub teacher_assignments: Vec<LearningGroupTeacherAssignment>,
    pub homeroom_ids: Vec<Uuid>,
    pub preferred_room_ids: Vec<Uuid>,
    pub registration_open: bool,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct RosterPreviewStudent {
    pub student_academic_year_id: Uuid,
    pub student_id: Uuid,
    pub student_code: Option<String>,
    pub display_name: String,
    pub grade_level_name: String,
    pub homeroom_name: Option<String>,
    pub proposed_active: bool,
    pub currently_active: bool,
    pub conflict_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct RosterPreview {
    pub learning_group_id: Uuid,
    pub source_hash: String,
    pub added: usize,
    pub removed: usize,
    pub retained: usize,
    pub conflicts: usize,
    pub students: Vec<RosterPreviewStudent>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RosterOverrideInput {
    pub student_academic_year_id: Uuid,
    pub action: RosterOverrideAction,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyRosterRequest {
    pub row_version: i64,
    pub source_hash: String,
    pub overrides: Vec<RosterOverrideInput>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LearningGroupStudent {
    pub id: Uuid,
    pub learning_group_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub student_id: Uuid,
    pub membership_status: MembershipStatus,
    pub roster_source: String,
    pub joined_at: NaiveDate,
    pub left_at: Option<NaiveDate>,
    pub published_at: Option<DateTime<Utc>>,
    pub row_version: i64,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DatedRosterMembership {
    pub id: Uuid,
    pub learning_group_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub student_id: Uuid,
    pub student_code: Option<String>,
    pub display_name: String,
    pub membership_status: MembershipStatus,
    pub roster_source: String,
    pub joined_at: NaiveDate,
    pub left_at: Option<NaiveDate>,
    pub published_at: Option<DateTime<Utc>>,
    pub row_version: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AddDatedRosterMembershipRequest {
    pub group_row_version: i64,
    pub student_academic_year_id: Uuid,
    pub joined_at: NaiveDate,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RemoveDatedRosterMembershipRequest {
    pub group_row_version: i64,
    pub membership_row_version: i64,
    pub left_at: NaiveDate,
}

#[derive(Debug, FromRow)]
pub(super) struct LearningOfferingRow {
    pub id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub kind: LearningOfferingKind,
    pub code_snapshot: String,
    pub name_snapshot: String,
    pub source_requirement_kind: Option<String>,
    pub source_requirement_id: Option<Uuid>,
    pub owning_organization_unit_id: Uuid,
    pub row_version: i64,
    pub migrated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub weekly_period_target: i32,
}

#[derive(Debug, FromRow)]
pub(super) struct LearningGroupRow {
    pub id: Uuid,
    pub learning_offering_id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub capacity: Option<i32>,
    pub row_version: i64,
    pub migrated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub registration_open: bool,
}

#[derive(Debug, FromRow)]
pub(super) struct LearningGroupStudentRow {
    pub id: Uuid,
    pub learning_group_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub student_id: Uuid,
    pub membership_status: MembershipStatus,
    pub roster_source: String,
    pub joined_at: NaiveDate,
    pub left_at: Option<NaiveDate>,
    pub published_at: Option<DateTime<Utc>>,
    pub row_version: i64,
}

impl From<LearningGroupStudentRow> for LearningGroupStudent {
    fn from(row: LearningGroupStudentRow) -> Self {
        Self {
            id: row.id,
            learning_group_id: row.learning_group_id,
            student_academic_year_id: row.student_academic_year_id,
            student_id: row.student_id,
            membership_status: row.membership_status,
            roster_source: row.roster_source,
            joined_at: row.joined_at,
            left_at: row.left_at,
            published_at: row.published_at,
            row_version: row.row_version,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq, sqlx::Type)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum RosterTrackingMode {
    Manual,
    Homeroom,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LearningGroupRosterTracking {
    pub mode: RosterTrackingMode,
    pub effective_from: Option<NaiveDate>,
    pub group_row_version: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateRosterTrackingRequest {
    pub mode: RosterTrackingMode,
    pub effective_from: Option<NaiveDate>,
    pub row_version: i64,
}

/// Explicit, optimistic target list for one atomic offering setup.
#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BulkGroupSetupRequest {
    pub groups: Vec<BulkGroupSetupTarget>,
    pub primary_teacher: Option<BulkGroupSetupPrimaryTeacher>,
    pub roster_tracking: Option<BulkGroupSetupTrackingInput>,
    pub delivery_row_version: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BulkGroupSetupTarget {
    pub learning_group_id: Uuid,
    pub row_version: i64,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BulkGroupSetupPrimaryTeacher {
    pub teacher_id: Uuid,
    #[serde(default)]
    pub replace_existing_primary: bool,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BulkGroupSetupTrackingInput {
    pub mode: RosterTrackingMode,
    pub effective_from: Option<NaiveDate>,
}

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum BulkGroupSetupTeacherAction {
    Unchanged,
    Set,
    Replace,
    PreserveExisting,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BulkGroupSetupPreviewTarget {
    pub group: LearningGroup,
    pub roster_tracking: LearningGroupRosterTracking,
    pub primary_teacher_action: BulkGroupSetupTeacherAction,
    pub roster_tracking_changed: bool,
    pub blocked_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BulkGroupSetupPreview {
    pub targets: Vec<BulkGroupSetupPreviewTarget>,
    pub changed_count: usize,
    pub skipped_count: usize,
    pub blocked_count: usize,
    pub delivery_row_version: i64,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BulkGroupSetupRosterTracking {
    pub learning_group_id: Uuid,
    pub tracking: LearningGroupRosterTracking,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BulkGroupSetupResponse {
    pub groups: Vec<LearningGroup>,
    pub roster_tracking: Vec<BulkGroupSetupRosterTracking>,
    pub delivery_row_version: i64,
}
