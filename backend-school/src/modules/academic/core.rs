#[cfg(feature = "api-docs")]
pub mod handlers;

// The cutover suite remains application-owned because it exercises historical migrations and
// cross-domain orchestration. These test-only namespaces preserve its original module topology
// without restoring production compatibility exports.
#[cfg(test)]
pub(crate) mod models {
    pub use crate::modules::academic::lifecycle::models::{
        TermTransitionAction, TermTransitionRequest, YearReopeningRequest, YearTransitionAction,
        YearTransitionRequest,
    };
    pub use school_academic_core::models::*;
}

#[cfg(test)]
pub(crate) mod services {
    pub use crate::modules::academic::lifecycle::services::{
        term_transitions, year_reopening_command as year_reopening, year_transitions,
    };
    pub use school_academic_core::services::{
        activation_context, bell_schedules, lifecycle_guard, years_terms,
    };

    pub(crate) mod activation_context_tests;

    pub(crate) mod promotion_students {
        pub use school_academic_core::services::promotion_students::*;

        mod tests;
    }

    pub(crate) mod workspaces {
        pub use school_academic_core::services::workspaces::*;

        mod tests;
    }

    pub(crate) mod promotion_targets {
        pub use school_academic_core::services::promotion_targets::*;

        pub(super) mod tests;
    }

    pub(crate) mod promotion_execution {
        pub use school_academic_core::services::promotion_execution::*;

        mod tests;
    }

    pub(crate) mod promotion_reconciliation {
        pub use school_academic_core::services::promotion_reconciliation::*;

        mod tests;
    }
}

#[cfg(test)]
mod schema_tests;
#[cfg(test)]
pub(crate) mod services_tests;
