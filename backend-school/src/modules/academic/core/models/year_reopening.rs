use super::*;

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

#[derive(Debug, Clone, Serialize)]
pub(crate) struct YearRecoveryState {
    pub context: YearLifecycleContext,
    pub running_years: i64,
    pub successor_years: i64,
    pub running_terms: i64,
}
