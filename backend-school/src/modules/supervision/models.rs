use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisionCycleStatus {
    Draft,
    Open,
    Closed,
    Archived,
}

impl SupervisionCycleStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Open => "open",
            Self::Closed => "closed",
            Self::Archived => "archived",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "draft" => Some(Self::Draft),
            "open" => Some(Self::Open),
            "closed" => Some(Self::Closed),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisionTemplateStatus {
    Draft,
    Active,
    Archived,
}

impl SupervisionTemplateStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "draft" => Some(Self::Draft),
            "active" => Some(Self::Active),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisionTargetType {
    School,
    OrganizationUnit,
    SubjectGroup,
    Staff,
}

impl SupervisionTargetType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::School => "school",
            Self::OrganizationUnit => "organization_unit",
            Self::SubjectGroup => "subject_group",
            Self::Staff => "staff",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "school" => Some(Self::School),
            "organization_unit" => Some(Self::OrganizationUnit),
            "subject_group" => Some(Self::SubjectGroup),
            "staff" => Some(Self::Staff),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisionTemplateItemType {
    Rating,
    Text,
}

impl SupervisionTemplateItemType {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Rating => "rating",
            Self::Text => "text",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "rating" => Some(Self::Rating),
            "text" => Some(Self::Text),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisionTemplateStepActorKind {
    Supervisor,
    ObservedTeacher,
    Permission,
    OrganizationPosition,
}

impl SupervisionTemplateStepActorKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Supervisor => "supervisor",
            Self::ObservedTeacher => "observed_teacher",
            Self::Permission => "permission",
            Self::OrganizationPosition => "organization_position",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "supervisor" => Some(Self::Supervisor),
            "observed_teacher" => Some(Self::ObservedTeacher),
            "permission" => Some(Self::Permission),
            "organization_position" => Some(Self::OrganizationPosition),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisionTemplateStepActionKind {
    Submit,
    Approve,
    ReturnForRevision,
    Publish,
    Acknowledge,
    Sign,
}

impl SupervisionTemplateStepActionKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Submit => "submit",
            Self::Approve => "approve",
            Self::ReturnForRevision => "return_for_revision",
            Self::Publish => "publish",
            Self::Acknowledge => "acknowledge",
            Self::Sign => "sign",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "submit" => Some(Self::Submit),
            "approve" => Some(Self::Approve),
            "return_for_revision" => Some(Self::ReturnForRevision),
            "publish" => Some(Self::Publish),
            "acknowledge" => Some(Self::Acknowledge),
            "sign" => Some(Self::Sign),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisionObservationStatus {
    Requested,
    Planned,
    InProgress,
    EvaluatorsSubmitted,
    UnderReview,
    Returned,
    Approved,
    Published,
    Acknowledged,
    Completed,
    Cancelled,
}

impl SupervisionObservationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Requested => "requested",
            Self::Planned => "planned",
            Self::InProgress => "in_progress",
            Self::EvaluatorsSubmitted => "evaluators_submitted",
            Self::UnderReview => "under_review",
            Self::Returned => "returned",
            Self::Approved => "approved",
            Self::Published => "published",
            Self::Acknowledged => "acknowledged",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "requested" => Some(Self::Requested),
            "planned" => Some(Self::Planned),
            "in_progress" => Some(Self::InProgress),
            "evaluators_submitted" => Some(Self::EvaluatorsSubmitted),
            "under_review" => Some(Self::UnderReview),
            "returned" => Some(Self::Returned),
            "approved" => Some(Self::Approved),
            "published" => Some(Self::Published),
            "acknowledged" => Some(Self::Acknowledged),
            "completed" => Some(Self::Completed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SupervisionEvaluatorStatus {
    Assigned,
    Draft,
    Submitted,
}

impl SupervisionEvaluatorStatus {
    pub fn from_code(code: &str) -> Option<Self> {
        match code {
            "assigned" => Some(Self::Assigned),
            "draft" => Some(Self::Draft),
            "submitted" => Some(Self::Submitted),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonSnapshot {
    pub source: Option<String>,
    pub timetable_entry_id: Option<Uuid>,
    pub subject_name: Option<String>,
    pub classroom_label: Option<String>,
    pub room_label: Option<String>,
    pub observed_at: Option<DateTime<Utc>>,
    pub period_label: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisionCycleTarget {
    pub id: Uuid,
    pub cycle_id: Uuid,
    pub target_type: SupervisionTargetType,
    pub target_id: Option<Uuid>,
    pub required_observations: i32,
    pub priority: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisionCycle {
    pub id: Uuid,
    pub academic_year: i32,
    pub semester: String,
    pub academic_semester_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub template_id: Uuid,
    pub booking_opens_at: Option<DateTime<Utc>>,
    pub booking_closes_at: Option<DateTime<Utc>>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub status: SupervisionCycleStatus,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub targets: Vec<SupervisionCycleTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisionTemplateItem {
    pub id: Uuid,
    pub section_id: Uuid,
    pub label: String,
    pub description: Option<String>,
    pub item_type: SupervisionTemplateItemType,
    pub required: bool,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisionTemplateSection {
    pub id: Uuid,
    pub template_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub sort_order: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub items: Vec<SupervisionTemplateItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisionTemplateStep {
    pub id: Uuid,
    pub template_id: Uuid,
    pub step_order: i32,
    pub step_code: String,
    pub label: String,
    pub actor_kind: SupervisionTemplateStepActorKind,
    pub actor_permission: Option<String>,
    pub organization_position_code: Option<String>,
    pub action_kind: SupervisionTemplateStepActionKind,
    pub required: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisionTemplate {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: SupervisionTemplateStatus,
    pub rating_min: i32,
    pub rating_max: i32,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub sections: Vec<SupervisionTemplateSection>,
    pub steps: Vec<SupervisionTemplateStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisionEvaluator {
    pub id: Uuid,
    pub observation_id: Uuid,
    pub evaluator_user_id: Uuid,
    pub evaluator_display_name: Option<String>,
    pub role_label: Option<String>,
    pub is_required: bool,
    pub status: SupervisionEvaluatorStatus,
    pub submitted_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisionEvaluatorConflict {
    pub observation_id: Uuid,
    pub observed_display_name: Option<String>,
    pub observed_at: DateTime<Utc>,
    pub lesson_title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisionEvaluatorAvailability {
    pub id: Uuid,
    pub name: String,
    pub title: Option<String>,
    pub available: bool,
    pub conflict_reason: Option<String>,
    pub conflict: Option<SupervisionEvaluatorConflict>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisionAction {
    pub id: Uuid,
    pub observation_id: Uuid,
    pub actor_user_id: Option<Uuid>,
    pub actor_display_name: Option<String>,
    pub action_kind: String,
    pub from_status: Option<SupervisionObservationStatus>,
    pub to_status: Option<SupervisionObservationStatus>,
    pub comment: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisionObservation {
    pub id: Uuid,
    pub cycle_id: Uuid,
    pub observed_user_id: Uuid,
    pub observed_display_name: Option<String>,
    pub requested_by: Option<Uuid>,
    pub approved_by: Option<Uuid>,
    pub template_id: Uuid,
    pub timetable_entry_id: Option<Uuid>,
    pub observed_at: DateTime<Utc>,
    pub manual_lesson: Option<ManualLesson>,
    pub lesson_snapshot: LessonSnapshot,
    pub status: SupervisionObservationStatus,
    pub requested_at: DateTime<Utc>,
    pub approved_at: Option<DateTime<Utc>>,
    pub cancelled_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub evaluators: Vec<SupervisionEvaluator>,
    pub actions: Vec<SupervisionAction>,
    pub average_rating: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualLesson {
    pub subject_name: String,
    pub classroom_label: String,
    pub room_label: Option<String>,
    pub observed_at: DateTime<Utc>,
    pub period_label: String,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSupervisionCycleTargetRequest {
    pub target_type: SupervisionTargetType,
    pub target_id: Option<Uuid>,
    #[serde(default = "default_required_observations")]
    pub required_observations: i32,
    #[serde(default = "default_target_priority")]
    pub priority: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSupervisionCycleRequest {
    pub academic_year: i32,
    pub semester: String,
    pub academic_semester_id: Option<Uuid>,
    pub title: String,
    pub description: Option<String>,
    pub template_id: Uuid,
    pub booking_opens_at: Option<DateTime<Utc>>,
    pub booking_closes_at: Option<DateTime<Utc>>,
    pub starts_at: DateTime<Utc>,
    pub ends_at: DateTime<Utc>,
    pub status: Option<SupervisionCycleStatus>,
    #[serde(default)]
    pub targets: Vec<CreateSupervisionCycleTargetRequest>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSupervisionCycleRequest {
    pub academic_year: Option<i32>,
    pub semester: Option<String>,
    pub academic_semester_id: Option<Uuid>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub template_id: Option<Uuid>,
    pub booking_opens_at: Option<DateTime<Utc>>,
    pub booking_closes_at: Option<DateTime<Utc>>,
    pub starts_at: Option<DateTime<Utc>>,
    pub ends_at: Option<DateTime<Utc>>,
    pub status: Option<SupervisionCycleStatus>,
    pub targets: Option<Vec<CreateSupervisionCycleTargetRequest>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSupervisionTemplateItemRequest {
    pub label: String,
    pub description: Option<String>,
    pub item_type: SupervisionTemplateItemType,
    #[serde(default = "default_required")]
    pub required: bool,
    #[serde(default)]
    pub sort_order: i32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSupervisionTemplateSectionRequest {
    pub title: String,
    pub description: Option<String>,
    #[serde(default)]
    pub sort_order: i32,
    #[serde(default)]
    pub items: Vec<CreateSupervisionTemplateItemRequest>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSupervisionTemplateStepRequest {
    pub step_order: i32,
    pub step_code: String,
    pub label: String,
    pub actor_kind: SupervisionTemplateStepActorKind,
    pub actor_permission: Option<String>,
    pub organization_position_code: Option<String>,
    pub action_kind: SupervisionTemplateStepActionKind,
    #[serde(default = "default_required")]
    pub required: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateSupervisionTemplateRequest {
    pub title: String,
    pub description: Option<String>,
    pub status: Option<SupervisionTemplateStatus>,
    #[serde(default = "default_rating_min")]
    pub rating_min: i32,
    #[serde(default = "default_rating_max")]
    pub rating_max: i32,
    #[serde(default)]
    pub sections: Vec<CreateSupervisionTemplateSectionRequest>,
    #[serde(default)]
    pub steps: Vec<CreateSupervisionTemplateStepRequest>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSupervisionTemplateRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<SupervisionTemplateStatus>,
    pub rating_min: Option<i32>,
    pub rating_max: Option<i32>,
    pub sections: Option<Vec<CreateSupervisionTemplateSectionRequest>>,
    pub steps: Option<Vec<CreateSupervisionTemplateStepRequest>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualLessonInput {
    pub subject_name: String,
    pub classroom_label: String,
    pub room_label: Option<String>,
    pub observed_at: DateTime<Utc>,
    pub period_label: String,
    pub reason: String,
}

impl ManualLessonInput {
    pub fn snapshot(&self) -> LessonSnapshot {
        LessonSnapshot {
            source: Some("manual".to_string()),
            timetable_entry_id: None,
            subject_name: Some(self.subject_name.clone()),
            classroom_label: Some(self.classroom_label.clone()),
            room_label: self.room_label.clone(),
            observed_at: Some(self.observed_at),
            period_label: Some(self.period_label.clone()),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestSupervisionObservationRequest {
    pub cycle_id: Uuid,
    pub timetable_entry_id: Option<Uuid>,
    pub observed_at: Option<DateTime<Utc>>,
    pub manual_lesson: Option<ManualLessonInput>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateRequestedObservationRequest {
    pub timetable_entry_id: Option<Uuid>,
    pub observed_at: Option<DateTime<Utc>>,
    pub manual_lesson: Option<ManualLessonInput>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateSupervisionObservationRequest {
    pub template_id: Option<Uuid>,
    pub timetable_entry_id: Option<Uuid>,
    pub observed_at: Option<DateTime<Utc>>,
    pub manual_lesson: Option<ManualLessonInput>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceObservationEvaluatorsRequest {
    #[serde(default)]
    pub evaluators: Vec<EvaluatorAssignmentInput>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelObservationRequest {
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluatorAssignmentInput {
    pub evaluator_user_id: Uuid,
    pub role_label: Option<String>,
    pub is_required: Option<bool>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApproveObservationRequest {
    #[serde(default)]
    pub evaluators: Vec<EvaluatorAssignmentInput>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReturnObservationRequest {
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EvaluationResponseInput {
    pub template_item_id: Uuid,
    pub rating_score: Option<f64>,
    pub text_response: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveEvaluationRequest {
    #[serde(default)]
    pub responses: Vec<EvaluationResponseInput>,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AcknowledgeObservationRequest {
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct SupervisionObservationFilter {
    pub cycle_id: Option<Uuid>,
    pub status: Option<SupervisionObservationStatus>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisionCycleProgress {
    pub cycle_id: Uuid,
    pub total_observations: i64,
    pub requested_count: i64,
    pub planned_count: i64,
    pub under_review_count: i64,
    pub approved_count: i64,
    pub published_count: i64,
    pub completed_count: i64,
    pub cancelled_count: i64,
    pub average_rating: Option<f64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SupervisionTeacherStatusRow {
    pub teacher_id: Uuid,
    pub teacher_display_name: String,
    pub organization_unit_names: Vec<String>,
    pub observation_id: Option<Uuid>,
    pub status: Option<SupervisionObservationStatus>,
    pub observed_at: Option<DateTime<Utc>>,
    pub lesson_title: Option<String>,
    pub evaluator_names: Vec<String>,
    pub average_rating: Option<f64>,
    pub next_step_label: String,
}

fn default_required_observations() -> i32 {
    1
}

fn default_target_priority() -> i32 {
    100
}

fn default_required() -> bool {
    true
}

fn default_rating_min() -> i32 {
    1
}

fn default_rating_max() -> i32 {
    5
}
