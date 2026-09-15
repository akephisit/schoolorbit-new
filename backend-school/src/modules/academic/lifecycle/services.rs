pub use school_academic_lifecycle::services::{
    approve_run, calculate_run, create_promotion_policy, create_run, execute_run,
    get_activation_workspace, get_opening_policy, get_promotion_impacts,
    get_promotion_policy_options, get_run_workspace, get_year_reopening_workspace,
    get_year_workspace, list_promotion_policies, list_runs, preview_term_preparation,
    resolve_promotion_impact, review_item, update_opening_policy, year_reopening_command,
    year_transitions,
};

#[cfg(test)]
pub(crate) use school_academic_lifecycle::models::*;
#[cfg(not(test))]
use school_academic_lifecycle::models::{
    ApplyTermPreparationInput, TermLifecycleWorkspace, TermPreparationOutcome,
    TermTransitionOutcome, TermTransitionRequest,
};
use school_authorization::ActorContext;
use school_errors::AppError;
use sqlx::PgPool;
use uuid::Uuid;

use crate::modules::academic::lifecycle_provider_adapter::LIFECYCLE_PROVIDERS;

#[cfg(test)]
pub(crate) use school_academic_core::services::lifecycle_guard;
#[cfg(test)]
pub(crate) use school_academic_lifecycle::services::checksum;

pub async fn get_workspace(
    pool: &PgPool,
    actor: &ActorContext,
    year: Uuid,
    term: Uuid,
) -> Result<TermLifecycleWorkspace, AppError> {
    school_academic_lifecycle::services::get_workspace(
        &LIFECYCLE_PROVIDERS,
        pool,
        actor,
        year,
        term,
    )
    .await
}

pub async fn apply_term_preparation(
    pool: &PgPool,
    actor: &ActorContext,
    input: ApplyTermPreparationInput,
) -> Result<TermPreparationOutcome, AppError> {
    school_academic_lifecycle::services::apply_term_preparation(
        &LIFECYCLE_PROVIDERS,
        pool,
        actor,
        input,
    )
    .await
}

pub mod term_transitions {
    use super::*;

    pub async fn transition_term(
        pool: &PgPool,
        actor: &ActorContext,
        term: Uuid,
        request: TermTransitionRequest,
    ) -> Result<TermTransitionOutcome, AppError> {
        school_academic_lifecycle::services::term_transitions::transition_term(
            &LIFECYCLE_PROVIDERS,
            pool,
            actor,
            term,
            request,
        )
        .await
    }
}

#[cfg(test)]
pub(crate) async fn workspace_in_transaction(
    transaction: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    actor: &ActorContext,
    year: Uuid,
    term: Uuid,
) -> Result<TermLifecycleWorkspace, AppError> {
    school_academic_lifecycle::services::readiness::workspace_in_transaction(
        &LIFECYCLE_PROVIDERS,
        transaction,
        actor,
        year,
        term,
    )
    .await
}

#[cfg(test)]
pub(crate) mod promotion_approval {
    pub(crate) use school_academic_lifecycle::services::promotion_approval::*;
}

#[cfg(test)]
pub(crate) mod promotion_calculation {
    pub(crate) use school_academic_lifecycle::services::promotion_calculation::*;
}

#[cfg(test)]
pub(crate) mod promotion_execution {
    pub(crate) use school_academic_lifecycle::services::promotion_execution::*;

    pub(crate) mod tests {
        pub(crate) use super::super::promotion_execution_tests::{
            approved_fixture, started_run_fixture,
        };
    }
}

#[cfg(test)]
pub(crate) mod promotion_opening {
    pub(crate) use school_academic_lifecycle::services::promotion_opening::*;
}

#[cfg(test)]
pub(crate) mod promotion_run_review {
    pub(crate) use school_academic_lifecycle::services::promotion_run_review::*;

    pub(crate) mod tests {
        pub(crate) use super::super::promotion_review_tests::{
            hold, ready_run, ready_run_with_extra_student,
        };
    }
}

#[cfg(test)]
pub(crate) mod promotion_runs {
    pub(crate) use school_academic_lifecycle::services::promotion_runs::*;

    pub(crate) mod tests {
        pub(crate) use super::super::promotion_run_tests::fixture;
    }
}

#[cfg(test)]
pub(crate) mod term_preparation {
    pub(crate) use school_academic_lifecycle::services::term_preparation::{
        normalize_input, preview, validate_checksum,
    };

    use super::*;

    pub(crate) async fn apply(
        pool: &PgPool,
        actor: &ActorContext,
        input: ApplyTermPreparationInput,
    ) -> Result<TermPreparationOutcome, AppError> {
        school_academic_lifecycle::services::term_preparation::apply(
            &LIFECYCLE_PROVIDERS,
            pool,
            actor,
            input,
        )
        .await
    }
}

#[cfg(test)]
mod opening_policy_tests;
#[cfg(test)]
mod opening_readiness_tests;
#[cfg(test)]
mod promotion_approval_tests;
#[cfg(test)]
mod promotion_calculation_tests;
#[cfg(test)]
pub(crate) mod promotion_execution_tests;
#[cfg(test)]
mod promotion_impact_resolution_tests;
#[cfg(test)]
mod promotion_impact_tests;
#[cfg(test)]
mod promotion_policy_tests;
#[cfg(test)]
pub(crate) mod promotion_review_tests;
#[cfg(test)]
pub(crate) mod promotion_run_tests;
#[cfg(test)]
mod promotion_workspace_tests;
#[cfg(test)]
mod provider_tests;
#[cfg(test)]
mod term_preparation_tests;
#[cfg(test)]
mod transition_tests;
#[cfg(test)]
mod year_readiness_owner_tests;
#[cfg(test)]
mod year_reopening_core_evidence_tests;
#[cfg(test)]
mod year_reopening_dependency_tests;
#[cfg(test)]
pub(crate) mod year_reopening_tests;
#[cfg(test)]
mod year_transition_tests;
