//! Typed phase-one process and correspondence wire contracts.
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ProcessStepKind {
    Acknowledge,
    Review,
    Approve,
    Execute,
    Sign,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ProcessCompletionPolicy {
    All,
    Any,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ProcessRecipientKind {
    User,
    OrganizationUnit,
    OrganizationPosition,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ProcessStatus {
    Active,
    RevisionRequested,
    Completed,
    Rejected,
    Closed,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum ProcessAction {
    Read,
    Acknowledge,
    Review,
    Approve,
    Execute,
    Sign,
    Reject,
    ReturnForRevision,
    Report,
    Close,
    Reassign,
    Route,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum CorrespondenceRegister {
    Incoming,
    Outgoing,
    Internal,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum CorrespondenceUrgency {
    Normal,
    Urgent,
    VeryUrgent,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum CorrespondenceStatus {
    Draft,
    InProgress,
    RevisionRequested,
    Completed,
    Rejected,
    Closed,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum CorrespondenceListView {
    ActionRequired,
    Sent,
    Drafts,
    #[default]
    All,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum DocumentAnnotationKind {
    Text,
    Ink,
    Highlight,
    Circle,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum SignatureMethod {
    Drawn,
    Image,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ProcessRecipient {
    pub kind: ProcessRecipientKind,
    pub user_id: Option<Uuid>,
    pub organization_unit_id: Option<Uuid>,
    pub position_code: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ProcessTemplateStep {
    pub title: String,
    pub kind: ProcessStepKind,
    pub completion_policy: ProcessCompletionPolicy,
    #[serde(default)]
    pub recipients: Vec<ProcessRecipient>,
    pub due_after_days: Option<i32>,
    #[serde(default)]
    pub requires_signature: bool,
    #[serde(default)]
    pub requires_comment: bool,
    #[serde(default)]
    pub requires_evidence: bool,
    #[serde(default)]
    pub requires_delegated_authority: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SaveProcessTemplateRequest {
    pub request_id: Uuid,
    pub expected_revision: Option<i64>,
    pub code: String,
    pub title: String,
    pub steps: Vec<ProcessTemplateStep>,
    pub active: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ProcessTemplate {
    pub id: Uuid,
    pub code: String,
    pub title: String,
    pub version: i64,
    pub steps: Vec<ProcessTemplateStep>,
    pub active: bool,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ProcessRecipientSnapshot {
    pub user_id: Uuid,
    pub display_name: String,
    pub completed_at: Option<DateTime<Utc>>,
    pub read_at: Option<DateTime<Utc>>,
    pub reassigned_from: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ProcessRuntimeStep {
    pub step_index: usize,
    pub definition: ProcessTemplateStep,
    pub recipients: Vec<ProcessRecipientSnapshot>,
    pub due_at: Option<DateTime<Utc>>,
    pub work_item_id: Option<Uuid>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ProcessSource {
    pub module_code: String,
    pub resource_type: String,
    pub resource_id: Uuid,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ProcessInstance {
    pub id: Uuid,
    pub source: ProcessSource,
    pub title: String,
    pub action_path: String,
    pub template_id: Option<Uuid>,
    pub template_version: Option<i64>,
    pub revision: i64,
    pub status: ProcessStatus,
    pub current_step: usize,
    pub steps: Vec<ProcessRuntimeStep>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub due_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub directed: Option<DirectedProcess>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ProcessStepRecipients {
    pub step_index: usize,
    pub recipients: Vec<ProcessRecipient>,
    pub due_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct InkPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct InkStroke {
    pub points: Vec<InkPoint>,
    pub width: f64,
    pub color: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DocumentAnnotation {
    pub id: Uuid,
    pub kind: DocumentAnnotationKind,
    pub page: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub text: Option<String>,
    pub color: String,
    #[serde(default)]
    pub strokes: Vec<InkStroke>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SignatureInput {
    pub method: SignatureMethod,
    pub asset_id: Option<Uuid>,
    #[serde(default)]
    pub strokes: Vec<InkStroke>,
    pub page: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub document_revision: i64,
    pub content_hash: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ProcessActionRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub action: ProcessAction,
    #[serde(default)]
    pub annotations: Vec<DocumentAnnotation>,
    pub comment: Option<String>,
    #[serde(default)]
    pub evidence_file_ids: Vec<Uuid>,
    pub signature: Option<SignatureInput>,
    pub reassign_user_id: Option<Uuid>,
    pub reassign_from_user_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignment_id: Option<Uuid>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub routing: Option<DirectedRoutingRequest>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CorrespondenceSourceLink {
    pub label: String,
    pub url: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CreateCorrespondenceRequest {
    pub request_id: Uuid,
    pub register: CorrespondenceRegister,
    pub title: String,
    pub sender: String,
    pub document_number: Option<String>,
    pub document_date: Option<NaiveDate>,
    pub urgency: CorrespondenceUrgency,
    #[serde(default)]
    pub source_links: Vec<CorrespondenceSourceLink>,
    pub organization_unit_id: Option<Uuid>,
    pub original_file_id: Option<Uuid>,
    #[serde(default)]
    pub attachment_file_ids: Vec<Uuid>,
    pub content: String,
    #[serde(default)]
    pub annotations: Vec<DocumentAnnotation>,
    pub template_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ReviseCorrespondenceRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub content: String,
    pub original_file_id: Option<Uuid>,
    #[serde(default)]
    pub attachment_file_ids: Vec<Uuid>,
    #[serde(default)]
    pub annotations: Vec<DocumentAnnotation>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub metadata: Option<CorrespondenceDraftMetadata>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CorrespondenceDraftMetadata {
    pub title: String,
    pub sender: String,
    pub document_number: Option<String>,
    pub document_date: Option<NaiveDate>,
    pub urgency: CorrespondenceUrgency,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SendCorrespondenceRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub template_id: Uuid,
    #[serde(default)]
    pub step_recipients: Vec<ProcessStepRecipients>,
    pub due_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CorrespondenceDocumentRevision {
    pub revision: i64,
    pub content: String,
    pub original_file_id: Option<Uuid>,
    pub attachment_file_ids: Vec<Uuid>,
    pub annotations: Vec<DocumentAnnotation>,
    pub content_hash: String,
    pub file_snapshots: Vec<CorrespondenceFileSnapshot>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CorrespondenceSummary {
    pub id: Uuid,
    pub register_number: i64,
    pub register_year: i32,
    pub register: CorrespondenceRegister,
    pub title: String,
    pub sender: String,
    pub document_number: Option<String>,
    pub document_date: Option<NaiveDate>,
    pub urgency: CorrespondenceUrgency,
    pub organization_unit_id: Option<Uuid>,
    pub revision: i64,
    pub status: CorrespondenceStatus,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct ProcessHistoryEntry {
    pub id: Uuid,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub reassigned_from_user_id: Option<Uuid>,
    pub reassigned_to_user_id: Option<Uuid>,
    pub action: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assignment_id: Option<Uuid>,
    pub document_revision: i64,
    pub annotations: Vec<DocumentAnnotation>,
    pub comment: Option<String>,
    pub evidence_file_ids: Vec<Uuid>,
    pub created_at: DateTime<Utc>,
    pub revision: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CorrespondenceSigningEvidence {
    pub id: Uuid,
    pub signer_id: Uuid,
    pub signer_name: String,
    pub signer_role: Option<String>,
    pub signed_at: DateTime<Utc>,
    pub signed_content_hash: String,
    pub signature_file_snapshot: Option<CorrespondenceFileSnapshot>,
    pub signature: SignatureInput,
    pub file_id: Option<Uuid>,
    pub action_id: Uuid,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CorrespondenceDetail {
    pub record: CorrespondenceSummary,
    pub source_links: Vec<CorrespondenceSourceLink>,
    pub document: CorrespondenceDocumentRevision,
    pub revisions: Vec<CorrespondenceDocumentRevision>,
    pub instance: Option<ProcessInstance>,
    pub history: Vec<ProcessHistoryEntry>,
    pub signatures: Vec<CorrespondenceSigningEvidence>,
    pub can_manage: bool,
    pub can_act: bool,
    pub available_actions: Vec<ProcessAction>,
    pub assignment_actions: Vec<DirectedAssignmentActions>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CorrespondenceList {
    pub items: Vec<CorrespondenceSummary>,
    pub next_cursor: Option<Uuid>,
    pub counts: CorrespondenceListCounts,
    pub progress: Vec<CorrespondenceListProgress>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CorrespondenceListCounts {
    pub action_required: i64,
    pub sent: i64,
    pub drafts: i64,
    pub all: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CorrespondenceListProgress {
    pub correspondence_id: Uuid,
    pub current_step_index: Option<usize>,
    pub step_count: usize,
    pub current_step_title: Option<String>,
    pub pending_recipients: Vec<CorrespondenceUserOption>,
    pub due_at: Option<DateTime<Utc>>,
    pub my_action: Option<ProcessAction>,
    pub can_revise: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SignatureAsset {
    pub id: Uuid,
    pub label: String,
    pub file_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CreateSignatureAssetRequest {
    pub request_id: Uuid,
    pub label: String,
    pub file_id: Uuid,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CorrespondenceUserOption {
    pub id: Uuid,
    pub display_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CorrespondenceOrganizationOption {
    pub id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CorrespondencePositionOption {
    pub organization_unit_id: Uuid,
    pub code: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CorrespondenceOptions {
    pub users: Vec<CorrespondenceUserOption>,
    pub organizations: Vec<CorrespondenceOrganizationOption>,
    pub positions: Vec<CorrespondencePositionOption>,
    pub templates: Vec<ProcessTemplate>,
    pub decision_makers: Vec<CorrespondenceUserOption>,
    pub reviewers: Vec<CorrespondenceUserOption>,
    pub routing_settings: Vec<CorrespondenceRoutingSettings>,
    pub can_manage_routing_settings: bool,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum DirectedPurpose {
    Decide,
    Acknowledge,
    Execute,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum DirectedAssignmentStatus {
    Pending,
    WaitingChildren,
    WaitingReview,
    Completed,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DirectedSendCorrespondenceRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub first_recipient: ProcessRecipient,
    pub due_at: Option<DateTime<Utc>>,
}

fn required_default() -> bool {
    true
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DirectedRouteRecipient {
    pub recipient: ProcessRecipient,
    pub purpose: DirectedPurpose,
    #[serde(default = "required_default")]
    pub required: bool,
    #[serde(default)]
    pub allow_onward_assignment: bool,
    pub due_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DirectedRoutingRequest {
    pub recipients: Vec<DirectedRouteRecipient>,
    #[serde(default)]
    pub require_review: bool,
    pub reviewer: Option<ProcessRecipient>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DirectedAssignment {
    pub id: Uuid,
    pub assigned_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub parent_id: Option<Uuid>,
    pub selector: ProcessRecipient,
    pub recipient: ProcessRecipientSnapshot,
    pub purpose: DirectedPurpose,
    pub status: DirectedAssignmentStatus,
    pub required: bool,
    pub allow_onward_assignment: bool,
    pub reviewer: Option<ProcessRecipientSnapshot>,
    pub due_at: Option<DateTime<Utc>>,
    pub work_item_id: Option<Uuid>,
    pub review_work_item_id: Option<Uuid>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DirectedProcess {
    pub assignments: Vec<DirectedAssignment>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct DirectedAssignmentActions {
    pub assignment_id: Uuid,
    pub actions: Vec<ProcessAction>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CorrespondenceRoutingSettings {
    pub register: CorrespondenceRegister,
    pub revision: i64,
    pub eligible_decision_makers: Vec<ProcessRecipient>,
    pub default_first_recipient: Option<ProcessRecipient>,
    pub default_purpose: DirectedPurpose,
    pub default_require_review: bool,
    pub default_reviewer: Option<ProcessRecipient>,
    pub default_allow_onward_assignment: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SaveCorrespondenceRoutingSettingsRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub register: CorrespondenceRegister,
    pub eligible_decision_makers: Vec<ProcessRecipient>,
    pub default_first_recipient: Option<ProcessRecipient>,
    pub default_purpose: DirectedPurpose,
    #[serde(default)]
    pub default_require_review: bool,
    pub default_reviewer: Option<ProcessRecipient>,
    #[serde(default)]
    pub default_allow_onward_assignment: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CorrespondenceFileSnapshot {
    pub file_id: Uuid,
    pub version_id: Uuid,
    pub checksum: String,
}
