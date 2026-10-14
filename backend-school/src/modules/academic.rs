#[cfg(test)]
pub(crate) mod assessment_adapter;
pub mod core;
#[cfg(test)]
pub mod cutover_test_preflight;
#[cfg(test)]
mod cutover_test_preflight_database_tests;
#[cfg(test)]
pub mod cutover_test_support;
pub mod delivery;
pub mod gradebook;
pub mod handlers;
pub mod learner_evaluation;
pub mod lifecycle;
pub(crate) mod lifecycle_provider_adapter;
pub mod models;
pub mod reconciliation;
pub(crate) mod result_lock_adapter;
pub mod results;
pub mod services;
pub mod websockets;

use crate::AppState;
use axum::Router;
use school_academic_http::state::AcademicHttpState;

pub fn academic_routes(state: AcademicHttpState) -> Router<AppState> {
    school_academic_http::routes::academic_routes()
        .with_state(state)
        .nest(
            "/question-bank",
            crate::modules::question_bank::question_bank_routes(),
        )
}
