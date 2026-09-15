use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TermActivationWorkspace {
    pub context: school_academic_core::models::TermLifecycleContext,
    pub opens_year: bool,
    pub predecessor: Option<school_academic_core::models::YearLifecycleContext>,
    pub policy: OpeningPolicy,
    pub planned_students: usize,
    pub eligible_placements: usize,
    pub findings: Vec<LifecycleFinding>,
    pub can_activate: bool,
    pub source_checksum: String,
}
