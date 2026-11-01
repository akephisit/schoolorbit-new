use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
#[cfg(feature = "api-docs")]
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::{
    LearningOfferingKind, LearningOfferingSnapshot, LearningOfferingTarget, LearningTeacherRole,
};

/// Published delivery graphs contain academic resource identities, never student rosters.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliverySnapshot {
    pub offerings: Vec<DeliveryVersionOffering>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliveryVersionOffering {
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
    pub groups: Vec<DeliveryVersionGroup>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliveryVersionGroup {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub description: Option<String>,
    pub capacity: Option<i32>,
    pub homeroom_ids: Vec<Uuid>,
    pub preferred_room_ids: Vec<Uuid>,
    pub teachers: Vec<DeliveryVersionTeacher>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliveryVersionTeacher {
    pub assignment_id: Uuid,
    pub teacher_id: Uuid,
    pub display_name: String,
    pub role: LearningTeacherRole,
}

/// Teacher membership in the latest published allocation of an offering's context.
/// Episode dates remain Delivery history; the selected graph owns membership here.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct DeliveryTeacherAllocation {
    pub delivery_version_id: Uuid,
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub learning_offering_id: Uuid,
    pub learning_group_id: Uuid,
    pub assignment_id: Uuid,
    pub teacher_id: Uuid,
    pub role: LearningTeacherRole,
}

/// Active teacher IDs in one group of the selected published allocation.
/// Snake-case serialization is an internal typed SQL-binding contract, not an HTTP DTO.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct GroupTeacherAssignments {
    pub learning_group_id: Uuid,
    pub teacher_ids: Vec<Uuid>,
    pub primary_teacher_ids: Vec<Uuid>,
}

/// A source ID remains present when a published graph has no matching memberships.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishedTeacherAssignments {
    pub delivery_version_id: Option<Uuid>,
    pub groups: Vec<GroupTeacherAssignments>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublishedTeacherGroupIds {
    pub delivery_version_id: Option<Uuid>,
    pub learning_group_ids: Vec<Uuid>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, sqlx::Type)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum DeliveryVersionStatus {
    Draft,
    Published,
    Cancelled,
}

/// Civil-date policy for deliberate publication; academic contexts have no inferred dates.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct PublicationDateRules {
    pub today: NaiveDate,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub minimum_effective_from: Option<NaiveDate>,
}

/// Bounded immutable source evidence, independent of resource counts and access projections.
#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeliveryPublicationContext {
    pub id: Uuid,
    pub publication_no: i64,
    pub effective_from: NaiveDate,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub effective_until: Option<NaiveDate>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub published_at: Option<DateTime<Utc>>,
    pub is_superseded: bool,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeliveryVersion {
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub publication_no: Option<i64>,
    pub is_superseded: bool,
    pub id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub source_version_id: Option<Uuid>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub effective_from: Option<NaiveDate>,
    pub reference_date: NaiveDate,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub effective_until: Option<NaiveDate>,
    pub status: DeliveryVersionStatus,
    pub row_version: i64,
    pub created_by: Option<Uuid>,
    pub published_by: Option<Uuid>,
    pub published_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub snapshot: DeliverySnapshot,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeliveryVersionDetails {
    #[serde(flatten)]
    pub version: DeliveryVersion,
    /// Current registry groups for visible, pinned course versions; outside publication facts.
    pub course_metadata: Vec<DeliveryVersionCourseMetadata>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeliveryVersionCourseMetadata {
    pub subject_version_id: Uuid,
    pub subject_group_id: Uuid,
    pub subject_group_name: String,
}

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeliveryVersionSummary {
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub publication_no: Option<i64>,
    pub is_superseded: bool,
    pub id: Uuid,
    pub academic_term_id: Uuid,
    pub academic_year_id: Uuid,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub source_version_id: Option<Uuid>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub change_set_id: Option<Uuid>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub effective_from: Option<NaiveDate>,
    pub reference_date: NaiveDate,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub effective_until: Option<NaiveDate>,
    pub status: DeliveryVersionStatus,
    pub row_version: i64,
    pub offering_count: i64,
    pub group_count: i64,
    pub teacher_assignment_count: i64,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub published_at: Option<DateTime<Utc>>,
    #[cfg_attr(feature = "api-docs", schema(required = true))]
    pub published_by: Option<Uuid>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(IntoParams, ToSchema))]
#[cfg_attr(feature = "api-docs", into_params(parameter_in = Query))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeliveryVersionQuery {
    pub academic_term_id: Uuid,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum DeliveryReadinessCode {
    DuplicateOffering,
    DuplicateGroup,
    DuplicateTeacher,
    InvalidWeeklyTarget,
    CatalogKindMismatch,
    MissingTargets,
    MissingGroups,
    MissingPrimaryTeacher,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeliveryReadinessFinding {
    pub code: DeliveryReadinessCode,
    pub learning_offering_id: Uuid,
    pub learning_group_id: Option<Uuid>,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum DeliveryChangeKind {
    Added,
    Removed,
    Changed,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeliveryVersionChange {
    pub kind: DeliveryChangeKind,
    pub learning_offering_id: Uuid,
    pub resource_id: Uuid,
    pub label: String,
    pub field: String,
    pub before: Option<String>,
    pub after: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeliveryResourceLabel {
    pub id: Uuid,
    pub code: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeleteDeliveryVersionRequest {
    pub row_version: i64,
    pub change_set_row_version: i64,
    pub expected_item_count: i64,
    pub expected_offering_count: i64,
    pub expected_group_count: i64,
}

#[derive(Clone, Debug, Serialize)]
#[cfg_attr(feature = "api-docs", derive(ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DeletedDeliveryVersion {
    pub id: Uuid,
    pub change_set_id: Uuid,
    pub source_version_id: Option<Uuid>,
    pub deleted_item_count: i64,
    pub deleted_offering_count: i64,
    pub deleted_group_count: i64,
}
