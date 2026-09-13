use super::*;

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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct TermLifecycleContext {
    pub academic_year_id: Uuid,
    pub academic_term_id: Uuid,
    pub year_name: String,
    pub term_name: String,
    pub year_status: AcademicYearStatus,
    pub term_status: AcademicTermStatus,
    pub year_row_version: i64,
    pub term_row_version: i64,
    pub year_start_date: NaiveDate,
    pub year_end_date: NaiveDate,
    pub term_start_date: NaiveDate,
    pub planned_end_date: Option<NaiveDate>,
    pub closed_on: Option<NaiveDate>,
    pub sequence: i32,
    pub bell_schedule_id: Uuid,
    pub included_in_year_result: bool,
    pub blocks_year_closure: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TermTransitionOutcome {
    pub request_id: Uuid,
    pub action: TermTransitionAction,
    pub context: TermLifecycleContext,
    pub completed_at: DateTime<Utc>,
}
