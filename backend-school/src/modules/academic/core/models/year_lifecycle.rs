use super::*;

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

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct YearLifecycleContext {
    pub academic_year_id: Uuid,
    pub year: i32,
    pub name: String,
    pub start_date: NaiveDate,
    pub end_date: NaiveDate,
    pub status: AcademicYearStatus,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema, sqlx::FromRow)]
#[serde(rename_all = "camelCase")]
pub struct YearTermLifecycleState {
    pub academic_term_id: Uuid,
    pub name: String,
    pub sequence: i32,
    pub status: AcademicTermStatus,
    pub included_in_year_result: bool,
    pub blocks_year_closure: bool,
    pub row_version: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct YearTransitionOutcome {
    pub request_id: Uuid,
    pub action: YearTransitionAction,
    pub context: YearLifecycleContext,
    pub completed_at: DateTime<Utc>,
}
