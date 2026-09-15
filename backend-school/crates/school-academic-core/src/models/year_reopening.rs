use super::*;

#[derive(Debug, Clone, Serialize)]
pub struct YearRecoveryState {
    pub context: YearLifecycleContext,
    pub running_years: i64,
    pub successor_years: i64,
    pub running_terms: i64,
}
