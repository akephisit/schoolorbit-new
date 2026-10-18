//! Staff-owned leave and planned exit contracts. Amounts are always computed by the server.
use crate::career::StaffPersonnelType;
use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use school_workflow::process_models::{
    ProcessAction, ProcessInstance, ProcessTemplateStep, SignatureInput,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! leave_enum {
    ($name:ident { $($variant:ident),+ $(,)? }) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
        #[cfg_attr(feature="api-docs", derive(utoipa::ToSchema))]
        #[serde(rename_all="snake_case")]
        pub enum $name { $($variant),+ }
    };
}
leave_enum!(LeaveKind { Leave, Exit });
leave_enum!(LeaveUnit {
    Day,
    HalfDay,
    Minute
});
leave_enum!(LeaveCountingMode {
    CalendarDays,
    WorkingDays
});
leave_enum!(LeaveDayPortion {
    FullDay,
    Morning,
    Afternoon
});
leave_enum!(LeaveStatus {
    Draft,
    Submitted,
    Returned,
    Approved,
    Rejected,
    Cancelled,
    CancellationPending
});
leave_enum!(LeaveProcessPurpose {
    Request,
    Cancellation
});

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveLeaveTypeRequest {
    pub request_id: Uuid,
    pub expected_revision: Option<i64>,
    pub code: String,
    pub name: String,
    pub kind: LeaveKind,
    pub archived: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LeaveType {
    pub id: Uuid,
    pub code: String,
    pub name: String,
    pub kind: LeaveKind,
    pub archived: bool,
    pub revision: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SaveLeavePolicyRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub effective_from: NaiveDate,
    pub source_reference: String,
    pub enabled: bool,
    pub personnel_types: Vec<StaffPersonnelType>,
    pub unit: LeaveUnit,
    pub counting_mode: LeaveCountingMode,
    /// ISO weekdays 1=Monday through 7=Sunday. Required for working_days.
    pub working_weekdays: Vec<u32>,
    pub excluded_dates: Vec<NaiveDate>,
    pub quota_year_start_month: u32,
    pub max_per_request: Option<i64>,
    pub annual_limit: Option<i64>,
    pub evidence_required: bool,
    pub evidence_from_amount: Option<i64>,
    pub minimum_notice_days: Option<i32>,
    pub steps: Vec<ProcessTemplateStep>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LeavePolicyVersion {
    pub id: Uuid,
    pub type_id: Uuid,
    pub version: i64,
    pub type_revision: i64,
    pub effective_from: NaiveDate,
    pub source_reference: String,
    pub enabled: bool,
    pub personnel_types: Vec<StaffPersonnelType>,
    pub unit: LeaveUnit,
    pub counting_mode: LeaveCountingMode,
    pub working_weekdays: Vec<u32>,
    pub excluded_dates: Vec<NaiveDate>,
    pub quota_year_start_month: u32,
    pub max_per_request: Option<i64>,
    pub annual_limit: Option<i64>,
    pub evidence_required: bool,
    pub evidence_from_amount: Option<i64>,
    pub minimum_notice_days: Option<i32>,
    pub steps: Vec<ProcessTemplateStep>,
    pub template_id: Uuid,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LeavePeriodInput {
    pub type_id: Uuid,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    pub day_portion: LeaveDayPortion,
    pub reason: String,
    #[serde(default)]
    pub evidence_file_ids: Vec<Uuid>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreateLeaveRequest {
    pub request_id: Uuid,
    pub period: LeavePeriodInput,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UpdateLeaveRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub period: LeavePeriodInput,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LeaveMutationRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CancelLeaveRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LeaveSignatureConsent {
    pub consent: bool,
    pub input: SignatureInput,
    pub image_file_id: Option<Uuid>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LeaveActionRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub expected_process_revision: i64,
    pub purpose: LeaveProcessPurpose,
    pub action: ProcessAction,
    pub comment: Option<String>,
    pub signature: Option<LeaveSignatureConsent>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LeaveRequestSummary {
    pub id: Uuid,
    pub type_id: Uuid,
    pub type_name: String,
    pub kind: LeaveKind,
    pub owner_id: Uuid,
    pub owner_name: String,
    pub status: LeaveStatus,
    pub revision: i64,
    pub policy_version_id: Option<Uuid>,
    pub personnel_type: Option<StaffPersonnelType>,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub start_time: Option<NaiveTime>,
    pub end_time: Option<NaiveTime>,
    pub day_portion: LeaveDayPortion,
    pub amount: i64,
    pub unit: LeaveUnit,
    pub reason: String,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LeaveHistoryEntry {
    pub id: Uuid,
    pub actor_id: Uuid,
    pub actor_name: String,
    pub action: String,
    pub revision: i64,
    pub comment: Option<String>,
    pub signature: Option<LeaveSignatureConsent>,
    pub content_hash: String,
    pub created_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LeaveRequestDetail {
    pub request: LeaveRequestSummary,
    pub policy: Option<LeavePolicyVersion>,
    pub instance: Option<ProcessInstance>,
    pub process_purpose: LeaveProcessPurpose,
    pub history: Vec<LeaveHistoryEntry>,
    pub evidence_file_ids: Vec<Uuid>,
    pub content_hash: String,
    pub can_edit: bool,
    pub can_submit: bool,
    pub can_act: bool,
    pub can_cancel: bool,
    pub available_actions: Vec<ProcessAction>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LeaveRequestPage {
    pub items: Vec<LeaveRequestSummary>,
    pub has_more: bool,
}
#[derive(Debug, Clone, Default, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::IntoParams))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature="api-docs", into_params(parameter_in=Query))]
pub struct LeaveListQuery {
    pub kind: Option<LeaveKind>,
    pub status: Option<LeaveStatus>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub include_archived: Option<bool>,
}
#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::IntoParams))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature="api-docs", into_params(parameter_in=Query))]
pub struct LeaveUsageQuery {
    pub from: NaiveDate,
    pub to: NaiveDate,
    pub kind: Option<LeaveKind>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LeaveUsageItem {
    pub user_id: Uuid,
    pub display_name: String,
    pub type_id: Uuid,
    pub type_name: String,
    pub unit: LeaveUnit,
    pub quota_year_start: NaiveDate,
    pub approved_amount: i64,
    pub reserved_amount: i64,
    pub annual_limit: Option<i64>,
    pub policy_version_id: Uuid,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LeaveUsageReport {
    pub items: Vec<LeaveUsageItem>,
    pub from: NaiveDate,
    pub to: NaiveDate,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LeavePersonalOptions {
    pub types: Vec<LeaveType>,
    pub personnel_type: Option<StaffPersonnelType>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LeaveRoutingUser {
    pub id: Uuid,
    pub display_name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LeaveRoutingOrganization {
    pub id: Uuid,
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LeaveRoutingPosition {
    pub organization_unit_id: Uuid,
    pub code: String,
    pub name: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LeaveRoutingOptions {
    pub users: Vec<LeaveRoutingUser>,
    pub organizations: Vec<LeaveRoutingOrganization>,
    pub positions: Vec<LeaveRoutingPosition>,
}

#[derive(Debug, Clone, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::IntoParams))]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[cfg_attr(feature="api-docs", into_params(parameter_in=Query))]
pub struct LeaveEffectivePolicyQuery {
    pub start_date: NaiveDate,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct LeaveFileSnapshot {
    pub file_id: Uuid,
    pub version_id: Uuid,
    pub checksum: String,
}
