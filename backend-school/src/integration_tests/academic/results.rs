#[cfg(test)]
pub(crate) mod aggregate_revision_tests;
#[cfg(test)]
mod aggregate_roster_owner_tests;
#[cfg(test)]
mod annual_correction_owner_tests;
#[cfg(test)]
mod annual_revisions_owner_tests;
#[cfg(test)]
mod annual_roster_owner_tests;
#[cfg(test)]
mod closure_coverage_owner_tests;
#[cfg(test)]
mod promotion_sources_owner_tests;
#[cfg(test)]
pub mod models {
    pub use school_academic_results::models::*;
}
#[cfg(test)]
pub mod services {
    pub use school_academic_results::services::*;
}
#[cfg(test)]
mod services_tests;
