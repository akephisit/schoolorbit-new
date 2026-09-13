use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct YearReopeningWorkspace {
    pub context: crate::modules::academic::core::models::YearLifecycleContext,
    pub findings: Vec<LifecycleFinding>,
    pub can_reopen: bool,
    pub source_checksum: String,
}
