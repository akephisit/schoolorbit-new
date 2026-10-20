//! Typed wire contracts. Every monetary field is an integer-satang string.
use chrono::{DateTime, NaiveDate, Utc};
use school_workflow::process_models::{ProcessAction, ProcessInstance, ProcessTemplateStep};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum BudgetStatus {
    Draft,
    Submitted,
    Returned,
    Approved,
    Rejected,
    Closed,
}
impl BudgetStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Submitted => "submitted",
            Self::Returned => "returned",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
            Self::Closed => "closed",
        }
    }
    pub fn editable(self) -> bool {
        matches!(self, Self::Draft | Self::Returned)
    }
}
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum BudgetAdjustmentKind {
    Add,
    Reduce,
    Transfer,
    Loan,
    Repayment,
}
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum BudgetResourceKind {
    Project,
    Request,
    Adjustment,
}
impl BudgetResourceKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Project => "project",
            Self::Request => "request",
            Self::Adjustment => "adjustment",
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetCatalogItem {
    pub id: Uuid,
    pub name: String,
    pub active: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetFiscalYear {
    pub id: Uuid,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub active: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetCatalogs {
    pub fiscal_years: Vec<BudgetFiscalYear>,
    pub funds: Vec<BudgetCatalogItem>,
    pub expense_categories: Vec<BudgetCatalogItem>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetSettings {
    pub revision: i64,
    pub fiscal_years: Vec<BudgetFiscalYear>,
    pub funds: Vec<BudgetCatalogItem>,
    pub expense_categories: Vec<BudgetCatalogItem>,
    pub plan_steps: Vec<ProcessTemplateStep>,
    pub request_steps: Vec<ProcessTemplateStep>,
    pub adjustment_steps: Vec<ProcessTemplateStep>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SaveBudgetSettingsRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub fiscal_years: Vec<BudgetFiscalYearInput>,
    pub funds: Vec<BudgetCatalogItemInput>,
    pub expense_categories: Vec<BudgetCatalogItemInput>,
    pub plan_steps: Vec<ProcessTemplateStep>,
    pub request_steps: Vec<ProcessTemplateStep>,
    pub adjustment_steps: Vec<ProcessTemplateStep>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetProjectInput {
    pub name: String,
    pub fiscal_year_id: Uuid,
    pub organization_unit_id: Uuid,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CreateBudgetProjectRequest {
    pub request_id: Uuid,
    pub name: String,
    pub fiscal_year_id: Uuid,
    pub organization_unit_id: Uuid,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SaveBudgetProjectRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub name: String,
    pub fiscal_year_id: Uuid,
    pub organization_unit_id: Uuid,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub activities: Vec<BudgetActivityInput>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetActivityInput {
    pub id: Option<Uuid>,
    pub title: String,
    pub default_fund_id: Option<Uuid>,
    pub default_expense_category_id: Option<Uuid>,
    pub lines: Vec<BudgetPlanLineInput>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetPlanLineInput {
    pub id: Option<Uuid>,
    pub description: String,
    pub quantity: String,
    pub unit: String,
    pub unit_price_satang: String,
    pub fund_id: Uuid,
    pub expense_category_id: Uuid,
    pub supply_item_id: Option<Uuid>,
    pub supply_version_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetActivity {
    pub id: Uuid,
    pub title: String,
    pub default_fund_id: Option<Uuid>,
    pub default_expense_category_id: Option<Uuid>,
    pub total_satang: String,
    pub lines: Vec<BudgetPlanLine>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetPlanLine {
    pub id: Uuid,
    pub description: String,
    pub quantity: String,
    pub unit: String,
    pub unit_price_satang: String,
    pub total_satang: String,
    pub fund_id: Uuid,
    pub expense_category_id: Uuid,
    pub supply_item_id: Option<Uuid>,
    pub supply_version_id: Option<Uuid>,
    pub supply_version_revision: Option<i64>,
    pub supply_name_snapshot: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetPlanSnapshot {
    pub id: Uuid,
    pub version: i64,
    pub project: BudgetProjectInput,
    pub activities: Vec<BudgetActivity>,
    pub total_satang: String,
    pub accepted_at: DateTime<Utc>,
    pub accepted_by: Uuid,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetProjectSummary {
    pub id: Uuid,
    pub revision: i64,
    pub name: String,
    pub fiscal_year_id: Uuid,
    pub organization_unit_id: Uuid,
    pub organization_unit_name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub owner_id: Uuid,
    pub status: BudgetStatus,
    pub accepted_version: Option<i64>,
    pub total_satang: String,
    pub reserved_satang: String,
    pub paid_satang: String,
    pub available_satang: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetCapabilities {
    pub can_edit: bool,
    pub can_submit: bool,
    pub can_act: bool,
    pub can_amend: bool,
    pub can_create_request: bool,
    pub can_pay: bool,
    pub can_close: bool,
    pub can_adjust: bool,
    pub can_manage_settings: bool,
    pub can_export: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetAllocationRef {
    pub project_id: Uuid,
    pub activity_id: Uuid,
    pub fund_id: Uuid,
    pub expense_category_id: Uuid,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetBalance {
    pub project_id: Uuid,
    pub activity_id: Uuid,
    pub fund_id: Uuid,
    pub expense_category_id: Uuid,
    pub planned_satang: String,
    pub adjusted_satang: String,
    pub current_satang: String,
    pub reserved_satang: String,
    pub paid_satang: String,
    pub available_satang: String,
    pub loan_outstanding_satang: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetProjectWorkspace {
    pub project: BudgetProjectSummary,
    pub activities: Vec<BudgetActivity>,
    pub accepted_plan: Option<BudgetPlanSnapshot>,
    pub balances: Vec<BudgetBalance>,
    pub capabilities: BudgetCapabilities,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetProjectPage {
    pub items: Vec<BudgetProjectSummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetMutationRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetActionRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub expected_process_revision: i64,
    pub action: ProcessAction,
    pub comment: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct StartBudgetAmendmentRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetRequestLineInput {
    pub plan_line_id: Uuid,
    pub amount_satang: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetSpendingRequestInput {
    pub activity_id: Uuid,
    pub title: String,
    pub lines: Vec<BudgetRequestLineInput>,
    pub evidence_file_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SaveBudgetSpendingRequestRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub activity_id: Uuid,
    pub title: String,
    pub lines: Vec<BudgetRequestLineInput>,
    pub evidence_file_ids: Vec<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetRequestSummary {
    pub id: Uuid,
    pub project_id: Uuid,
    pub activity_id: Uuid,
    pub owner_id: Uuid,
    pub title: String,
    pub revision: i64,
    pub status: BudgetStatus,
    pub total_satang: String,
    pub reserved_satang: String,
    pub paid_satang: String,
    pub remaining_satang: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetSpendingRequestDetail {
    pub request: BudgetRequestSummary,
    pub lines: Vec<BudgetRequestLine>,
    pub evidence_file_ids: Vec<Uuid>,
    pub capabilities: BudgetCapabilities,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetRequestPage {
    pub items: Vec<BudgetRequestSummary>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetPaymentAllocationInput {
    pub plan_line_id: Uuid,
    pub amount_satang: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CreateBudgetPaymentRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub paid_on: NaiveDate,
    pub voucher_number: String,
    pub note: Option<String>,
    pub evidence_file_ids: Vec<Uuid>,
    pub allocations: Vec<BudgetPaymentAllocationInput>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct RefundBudgetPaymentRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub refunded_on: NaiveDate,
    pub reason: String,
    pub evidence_file_ids: Vec<Uuid>,
    pub allocations: Vec<BudgetPaymentAllocationInput>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetFileSnapshot {
    pub file_id: Uuid,
    pub version_id: Uuid,
    pub checksum: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetRefund {
    pub id: Uuid,
    pub payment_id: Uuid,
    pub refunded_on: NaiveDate,
    pub reason: String,
    pub total_satang: String,
    pub allocations: Vec<BudgetPaymentAllocationInput>,
    pub evidence: Vec<BudgetFileSnapshot>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetPayment {
    pub id: Uuid,
    pub request_id: Uuid,
    pub paid_on: NaiveDate,
    pub voucher_number: String,
    pub note: Option<String>,
    pub total_satang: String,
    pub allocations: Vec<BudgetPaymentAllocationInput>,
    pub evidence: Vec<BudgetFileSnapshot>,
    pub created_by: Uuid,
    pub created_at: DateTime<Utc>,
    pub refunds: Vec<BudgetRefund>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetPaymentPage {
    pub items: Vec<BudgetPayment>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct CloseBudgetRequestRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub reason: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetAdjustmentInput {
    pub kind: BudgetAdjustmentKind,
    pub fiscal_year_id: Uuid,
    pub from: Option<BudgetAllocationRef>,
    pub to: Option<BudgetAllocationRef>,
    pub amount_satang: String,
    pub reason: String,
    pub loan_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct SaveBudgetAdjustmentRequest {
    pub request_id: Uuid,
    pub kind: BudgetAdjustmentKind,
    pub fiscal_year_id: Uuid,
    pub from: Option<BudgetAllocationRef>,
    pub to: Option<BudgetAllocationRef>,
    pub amount_satang: String,
    pub reason: String,
    pub loan_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetAdjustmentDetail {
    pub id: Uuid,
    pub revision: i64,
    pub owner_id: Uuid,
    pub status: BudgetStatus,
    pub kind: BudgetAdjustmentKind,
    pub fiscal_year_id: Uuid,
    pub from: Option<BudgetAllocationRef>,
    pub to: Option<BudgetAllocationRef>,
    pub amount_satang: String,
    pub reason: String,
    pub loan_id: Option<Uuid>,
    pub loan_outstanding_satang: String,
    pub created_at: DateTime<Utc>,
    pub capabilities: BudgetCapabilities,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetAdjustmentPage {
    pub items: Vec<BudgetAdjustmentDetail>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetHistoryEntry {
    pub id: Uuid,
    pub resource_kind: BudgetResourceKind,
    pub resource_id: Uuid,
    pub revision: i64,
    pub action: String,
    pub actor_id: Uuid,
    pub comment: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetHistoryPage {
    pub items: Vec<BudgetHistoryEntry>,
    pub total: i64,
    pub limit: i64,
    pub offset: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetProcessDetail {
    pub process: Option<ProcessInstance>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetDashboardRow {
    pub project_id: Uuid,
    pub project_name: String,
    pub activity_id: Uuid,
    pub activity_title: String,
    pub fund_id: Uuid,
    pub fund_name: String,
    pub expense_category_id: Uuid,
    pub expense_category_name: String,
    pub planned_satang: String,
    pub adjusted_satang: String,
    pub current_satang: String,
    pub reserved_satang: String,
    pub paid_satang: String,
    pub available_satang: String,
    pub loan_outstanding_satang: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetDashboardTotals {
    pub planned_satang: String,
    pub adjusted_satang: String,
    pub current_satang: String,
    pub reserved_satang: String,
    pub paid_satang: String,
    pub available_satang: String,
    pub loan_outstanding_satang: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetDashboard {
    pub totals: BudgetDashboardTotals,
    pub rows: Vec<BudgetDashboardRow>,
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema, utoipa::IntoParams))]
#[serde(rename_all = "camelCase")]
pub struct BudgetListQuery {
    pub fiscal_year_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub activity_id: Option<Uuid>,
    pub organization_unit_id: Option<Uuid>,
    pub status: Option<BudgetStatus>,
    pub search: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
}
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema, utoipa::IntoParams))]
#[serde(rename_all = "camelCase")]
pub struct BudgetReportQuery {
    pub export_scope: Option<bool>,
    pub metric: Option<BudgetReportMetric>,
    pub fiscal_year_id: Option<Uuid>,
    pub project_id: Option<Uuid>,
    pub organization_unit_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetRequestLine {
    pub plan_line_id: Uuid,
    pub description: String,
    pub unit: String,
    pub fund_id: Uuid,
    pub fund_name: String,
    pub expense_category_id: Uuid,
    pub expense_category_name: String,
    pub amount_satang: String,
    pub reserved_satang: String,
    pub paid_satang: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct UpdateBudgetAdjustmentRequest {
    pub request_id: Uuid,
    pub expected_revision: i64,
    pub kind: BudgetAdjustmentKind,
    pub fiscal_year_id: Uuid,
    pub from: Option<BudgetAllocationRef>,
    pub to: Option<BudgetAllocationRef>,
    pub amount_satang: String,
    pub reason: String,
    pub loan_id: Option<Uuid>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetCatalogItemInput {
    pub id: Option<Uuid>,
    pub name: String,
    pub active: bool,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "camelCase")]
pub struct BudgetFiscalYearInput {
    pub id: Option<Uuid>,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub active: bool,
}
#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize)]
#[cfg_attr(feature = "api-docs", derive(utoipa::ToSchema))]
#[serde(rename_all = "snake_case")]
pub enum BudgetReportMetric {
    #[default]
    Planned,
    Current,
    Paid,
}
