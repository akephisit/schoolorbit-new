use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::FromRow;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::modules::academic::core::models::StudyProgramOption;
use crate::modules::lookup::models::GradeLevelLookupItem;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, sqlx::Type, ToSchema)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum LearningOfferingKind {
    Course,
    Activity,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, sqlx::Type, ToSchema)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum LearningOfferingStatus {
    Draft,
    Published,
    Closed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize, sqlx::Type, ToSchema)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum OfferingTargetKind {
    Homeroom,
    GradeProgram,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, sqlx::Type, ToSchema)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum ActivityRegistrationType {
    #[serde(rename = "self")]
    #[sqlx(rename = "self")]
    SelfRegistration,
    Assigned,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, sqlx::Type, ToSchema)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum ActivitySchedulingMode {
    Synchronized,
    Independent,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, sqlx::Type, ToSchema)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum LearningTeacherRole {
    Primary,
    Secondary,
    Assistant,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, sqlx::Type, ToSchema)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum RosterStatus {
    Draft,
    Published,
    Closed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, sqlx::Type, ToSchema)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum MembershipStatus {
    Active,
    Ended,
    Removed,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum RosterOverrideAction {
    Add,
    Remove,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CurriculumPreviewAction {
    Create,
    Retain,
    Conflict,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CourseGradingPolicy {
    pub policy_code: String,
    #[serde(default = "default_course_total_score")]
    #[schema(value_type = String, pattern = r"^(0|[1-9]\d*)(\.\d{1,2})?$")]
    pub total_score: String,
    #[schema(value_type = Option<String>, pattern = r"^-?(0|[1-9]\d*)(\.\d{1,2})?$")]
    pub passing_score: Option<String>,
}

fn default_course_total_score() -> String {
    "100.00".to_string()
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityAttendanceRequirement {
    #[schema(value_type = Option<String>, pattern = r"^(0|[1-9]\d*)(\.\d{1,2})?$")]
    pub minimum_percent: Option<String>,
    pub required_sessions: Option<i32>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityPassCriteria {
    pub require_attendance: bool,
    pub require_teacher_confirmation: bool,
    pub outcomes: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfferingTargetInput {
    pub target_kind: OfferingTargetKind,
    pub homeroom_id: Option<Uuid>,
    pub grade_level_id: Uuid,
    pub study_program_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateCourseOfferingRequest {
    pub academic_term_id: Uuid,
    pub subject_version_id: Uuid,
    pub curriculum_course_requirement_id: Option<Uuid>,
    pub owning_organization_unit_id: Uuid,
    pub targets: Vec<OfferingTargetInput>,
    pub grading_policy: CourseGradingPolicy,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateActivityOfferingRequest {
    pub academic_term_id: Uuid,
    pub activity_version_id: Uuid,
    pub curriculum_activity_requirement_id: Option<Uuid>,
    pub owning_organization_unit_id: Uuid,
    pub targets: Vec<OfferingTargetInput>,
    pub registration_type: ActivityRegistrationType,
    pub scheduling_mode: ActivitySchedulingMode,
    pub capacity: Option<i32>,
    pub attendance_requirement: ActivityAttendanceRequirement,
    pub pass_criteria: ActivityPassCriteria,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
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

    pub fn owning_organization_unit_id(&self) -> Uuid {
        match self {
            Self::Course(request) => request.owning_organization_unit_id,
            Self::Activity(request) => request.owning_organization_unit_id,
        }
    }

    pub fn targets(&self) -> &[OfferingTargetInput] {
        match self {
            Self::Course(request) => &request.targets,
            Self::Activity(request) => &request.targets,
        }
    }
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateLearningOfferingRequest {
    pub row_version: i64,
    pub owning_organization_unit_id: Uuid,
    pub targets: Vec<OfferingTargetInput>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishLearningOfferingRequest {
    pub row_version: i64,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LearningOfferingQuery {
    pub academic_term_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LearningGroupTermQuery {
    pub academic_term_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, IntoParams, ToSchema)]
#[into_params(parameter_in = Query)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StudentActivityRegistrationQuery {
    pub academic_term_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PreviewCurriculumOfferingsRequest {
    pub academic_term_id: Uuid,
    pub study_program_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyCurriculumOfferingsRequest {
    pub academic_term_id: Uuid,
    pub study_program_ids: Vec<Uuid>,
    pub owning_organization_unit_id: Uuid,
    pub source_hash: String,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LearningOfferingTarget {
    pub id: Uuid,
    pub target_kind: OfferingTargetKind,
    pub homeroom_id: Option<Uuid>,
    pub grade_level_id: Uuid,
    pub study_program_id: Uuid,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CourseOfferingSnapshot {
    pub subject_version_id: Uuid,
    pub subject_id: Uuid,
    pub curriculum_course_requirement_id: Option<Uuid>,
    pub credit: String,
    pub hours: Option<String>,
    pub grading_policy: CourseGradingPolicy,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
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

#[derive(Clone, Debug, Serialize, ToSchema)]
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

#[derive(Clone, Debug, Serialize, ToSchema)]
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

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StudentActivityRegistrationResult {
    pub learning_offering_id: Uuid,
    pub learning_group_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub enrolled: bool,
    pub revision: i64,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LearningOfferingSnapshot {
    Course(CourseOfferingSnapshot),
    Activity(ActivityOfferingSnapshot),
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LearningOffering {
    pub id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    pub kind: LearningOfferingKind,
    pub code_snapshot: String,
    pub name_snapshot: String,
    pub source_requirement_kind: Option<String>,
    pub source_requirement_id: Option<Uuid>,
    pub status: LearningOfferingStatus,
    pub published_at: Option<DateTime<Utc>>,
    pub owning_organization_unit_id: Option<Uuid>,
    pub row_version: i64,
    pub migrated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub snapshot: LearningOfferingSnapshot,
    pub targets: Vec<LearningOfferingTarget>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LearningOfferingOverviewItem {
    pub offering: LearningOffering,
    pub grade_levels: Vec<GradeLevelLookupItem>,
    pub study_programs: Vec<StudyProgramOption>,
    pub group_count: i64,
    pub teacher_assignment_count: i64,
    pub groups_without_primary_teacher: i64,
    pub published_roster_count: i64,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LearningDeliveryOverview {
    pub academic_term_id: Uuid,
    pub offerings: Vec<LearningOfferingOverviewItem>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CurriculumOfferingPreviewItem {
    pub action: CurriculumPreviewAction,
    pub resource_kind: LearningOfferingKind,
    pub catalog_version_id: Uuid,
    pub requirement_id: Uuid,
    pub study_program_id: Uuid,
    pub grade_level_id: Uuid,
    pub code: String,
    pub name: String,
    pub credit: Option<String>,
    pub hours: Option<String>,
    pub existing_offering_id: Option<Uuid>,
    pub conflict_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CurriculumOfferingPreview {
    pub academic_term_id: Uuid,
    pub source_hash: String,
    pub items: Vec<CurriculumOfferingPreviewItem>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApplyCurriculumOfferingsResult {
    pub academic_term_id: Uuid,
    pub source_hash: String,
    pub offering_ids: Vec<Uuid>,
    pub created_count: usize,
    pub retained_count: usize,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateLearningGroupRequest {
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub capacity: Option<i32>,
    pub preferred_room_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateLearningGroupRequest {
    pub row_version: i64,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub capacity: Option<i32>,
    pub preferred_room_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TeacherAssignmentInput {
    pub teacher_id: Uuid,
    pub role: LearningTeacherRole,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplaceLearningGroupTeachersRequest {
    pub row_version: i64,
    pub teachers: Vec<TeacherAssignmentInput>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReplaceLearningGroupHomeroomsRequest {
    pub row_version: i64,
    pub homeroom_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(transparent)]
pub struct LearningGroupHomeroomIds(pub Vec<Uuid>);

#[derive(Clone, Debug, Serialize, ToSchema)]
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
    pub status: LearningOfferingStatus,
    pub roster_status: RosterStatus,
    pub roster_published_at: Option<DateTime<Utc>>,
    pub row_version: i64,
    pub migrated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub teacher_assignments: Vec<TeacherAssignmentInput>,
    pub homeroom_ids: Vec<Uuid>,
    pub preferred_room_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RosterPreviewStudent {
    pub student_academic_year_id: Uuid,
    pub student_id: Uuid,
    pub proposed_active: bool,
    pub currently_active: bool,
    pub conflict_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
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

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RosterOverrideInput {
    pub student_academic_year_id: Uuid,
    pub action: RosterOverrideAction,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplyRosterRequest {
    pub row_version: i64,
    pub source_hash: String,
    pub overrides: Vec<RosterOverrideInput>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PublishRosterRequest {
    pub row_version: i64,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
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

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ActivityResult {
    pub learning_result_id: Uuid,
    pub learning_group_student_id: Uuid,
    pub outcome: Option<String>,
    pub attendance_percent: Option<String>,
    pub teacher_comment: Option<String>,
    pub finalized_at: Option<DateTime<Utc>>,
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
    pub status: LearningOfferingStatus,
    pub published_at: Option<DateTime<Utc>>,
    pub owning_organization_unit_id: Option<Uuid>,
    pub row_version: i64,
    pub migrated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
    pub status: LearningOfferingStatus,
    pub roster_status: RosterStatus,
    pub roster_published_at: Option<DateTime<Utc>>,
    pub row_version: i64,
    pub migrated: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
