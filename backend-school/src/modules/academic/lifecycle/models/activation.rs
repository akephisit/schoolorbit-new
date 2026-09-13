use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TermActivationWorkspace {
    pub context: crate::modules::academic::core::models::TermLifecycleContext,
    pub opens_year: bool,
    pub predecessor: Option<crate::modules::academic::core::models::YearLifecycleContext>,
    pub policy: OpeningPolicy,
    pub planned_students: usize,
    pub eligible_placements: usize,
    pub findings: Vec<LifecycleFinding>,
    pub can_activate: bool,
    pub source_checksum: String,
}
