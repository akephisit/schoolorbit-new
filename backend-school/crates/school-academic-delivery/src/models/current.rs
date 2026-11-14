use super::{
    LearningOfferingKind, LearningOfferingSnapshot, LearningOfferingTarget, LearningTeacherRole,
};
use serde::{Deserialize, Serialize};
#[cfg(feature = "api-docs")]
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeliveryWorkspace {
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub row_version: i64,
    pub offerings: Vec<DeliveryOffering>,
    pub course_metadata: Vec<DeliveryCourseMetadata>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeliveryCourseMetadata {
    pub subject_version_id: Uuid,
    pub subject_group_id: Uuid,
    pub subject_group_name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(IntoParams, ToSchema))]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Query))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliveryWorkspaceQuery {
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateWeeklyPeriodTargetRequest {
    pub row_version: i64,
    pub weekly_period_target: i32,
}

/// A live projection of normalized current Delivery rows; never persisted as a graph.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliveryGraph {
    pub offerings: Vec<DeliveryOffering>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliveryOffering {
    pub row_version: i64,
    /// Empty means the actual source publication was not recorded (legacy/manual).
    #[serde(default)]
    pub curriculum_sources: Vec<super::CurriculumPublicationSource>,
    pub id: Uuid,
    pub kind: LearningOfferingKind,
    pub code: String,
    pub name: String,
    pub owning_organization_unit_id: Uuid,
    pub source_requirement_kind: Option<String>,
    pub source_requirement_id: Option<Uuid>,
    pub weekly_period_target: i32,
    pub catalog: LearningOfferingSnapshot,
    pub targets: Vec<LearningOfferingTarget>,
    pub homeroom_ids: Vec<Uuid>,
    pub groups: Vec<DeliveryGroup>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliveryGroup {
    pub registration_open: bool,
    pub row_version: i64,
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub capacity: Option<i32>,
    pub homeroom_ids: Vec<Uuid>,
    pub preferred_room_ids: Vec<Uuid>,
    pub teachers: Vec<DeliveryTeacher>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliveryTeacher {
    pub is_active: bool,
    pub row_version: i64,
    pub assignment_id: Uuid,
    pub teacher_id: Uuid,
    pub display_name: String,
    pub role: LearningTeacherRole,
}

/// Current teacher membership with the exact academic context and stable assignment identity.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct DeliveryTeacherAllocation {
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub learning_offering_id: Uuid,
    pub learning_group_id: Uuid,
    pub assignment_id: Uuid,
    pub teacher_id: Uuid,
    pub role: LearningTeacherRole,
}

/// Active teacher IDs in one current group.
/// Snake-case serialization is an internal typed SQL-binding contract, not an HTTP DTO.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GroupTeacherAssignments {
    pub learning_group_id: Uuid,
    pub teacher_ids: Vec<Uuid>,
    pub primary_teacher_ids: Vec<Uuid>,
}

/// Current memberships; an empty collection grants no assigned access.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurrentTeacherAssignments {
    pub groups: Vec<GroupTeacherAssignments>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CurrentTeacherGroupIds {
    pub learning_group_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteDeliveryResourceRequest {
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub row_version: i64,
    pub source_digest: String,
}

/// Counts supplied by each dependent owner through application composition.
#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OfferingDeletionImpact {
    pub learning_groups: i64,
    pub teacher_assignments: i64,
    pub memberships: i64,
    pub timetable_blocks: i64,
    pub exam_schedule_items: i64,
    pub assessment_scores: i64,
    pub attendance_sessions: i64,
    pub attendance_summaries: i64,
    pub official_results: i64,
    pub result_corrections: i64,
    pub term_aggregates: i64,
    pub annual_aggregates: i64,
    pub supervision_links: i64,
    pub derived_behavior_events: i64,
    pub evidence_files: i64,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeliveryDeletionPreview {
    pub learning_group_id: Option<Uuid>,
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub learning_offering_id: Uuid,
    pub row_version: i64,
    pub source_digest: String,
    pub impact: OfferingDeletionImpact,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeletedDeliveryResource {
    pub learning_group_id: Option<Uuid>,
    pub learning_offering_id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub deleted: bool,
    pub delivery_row_version: i64,
}

/// Exact locked graph identity lent to dependency owners; no owner may broaden it.
#[derive(Clone, Debug, Serialize)]
pub struct DeliveryDeletionScope {
    pub delete_offering: bool,
    pub learning_offering_id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub learning_group_ids: Vec<Uuid>,
}

#[derive(Clone, Debug, Serialize)]
pub struct DeliveryDeletionDependencies {
    pub impact: OfferingDeletionImpact,
    pub source_digest: String,
}
