use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use school_academic_core::models::{TermLifecycleContext, YearLifecycleContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TermTransitionAction {
    MarkReady,
    BeginClosing,
    CancelClosing,
    Close,
    Reopen,
    Cancel,
    Activate,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TermTransitionRequest {
    pub academic_year_id: Uuid,
    pub request_id: Uuid,
    pub action: TermTransitionAction,
    pub expected_year_version: i64,
    pub expected_term_version: i64,
    pub readiness_checksum: String,
    pub acknowledged_warning_codes: Vec<String>,
    pub closed_on: Option<NaiveDate>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TermTransitionOutcome {
    pub request_id: Uuid,
    pub action: TermTransitionAction,
    pub context: TermLifecycleContext,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum YearTransitionAction {
    BeginClosing,
    CancelClosing,
    Close,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct YearTransitionRequest {
    pub request_id: Uuid,
    pub action: YearTransitionAction,
    pub expected_year_version: i64,
    pub readiness_checksum: String,
    pub acknowledged_warning_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct YearTransitionOutcome {
    pub request_id: Uuid,
    pub action: YearTransitionAction,
    pub context: YearLifecycleContext,
    pub completed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct YearReopeningRequest {
    pub request_id: Uuid,
    pub expected_year_version: i64,
    pub source_checksum: String,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct YearReopeningOutcome {
    pub request_id: Uuid,
    pub context: YearLifecycleContext,
    pub completed_at: DateTime<Utc>,
}
