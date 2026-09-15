use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
mod opening;
pub use opening::*;
mod activation;
pub use activation::*;
mod promotion_policy;
pub use promotion_policy::*;
mod promotion_run;
pub use promotion_run::*;
mod promotion_item;
pub use promotion_item::*;
mod promotion_execution;
pub use promotion_execution::*;
mod promotion_workspace;
pub use promotion_workspace::*;
mod year_reopening;
pub use year_reopening::*;
mod promotion_impacts;
pub use promotion_impacts::*;
mod promotion_impact_resolutions;
pub use promotion_impact_resolutions::*;
mod term_preparation;
pub use term_preparation::*;
mod transitions;
pub use transitions::*;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct YearLifecycleWorkspace {
    pub context: school_academic_core::models::YearLifecycleContext,
    pub terms: Vec<school_academic_core::models::YearTermLifecycleState>,
    pub coverage: school_academic_results::models::AnnualClosureCoverage,
    pub findings: Vec<LifecycleFinding>,
    pub available_actions: Vec<YearTransitionAction>,
    pub can_close: bool,
    pub source_checksum: String,
}

#[derive(Debug, Clone, Deserialize, utoipa::IntoParams, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[into_params(parameter_in = Query)]
pub struct TermLifecycleQuery {
    pub academic_year_id: Uuid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleSeverity {
    Ready,
    Warning,
    Blocking,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LifecycleFinding {
    pub code: String,
    pub severity: LifecycleSeverity,
    pub count: usize,
    pub message: String,
    pub resolution_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TermLifecycleWorkspace {
    pub context: school_academic_core::models::TermLifecycleContext,
    pub findings: Vec<LifecycleFinding>,
    pub coverage: school_academic_results::models::TermClosureCoverage,
    pub available_actions: Vec<TermTransitionAction>,
    pub source_checksum: String,
}
