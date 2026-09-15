mod cycles;
mod evaluations;
mod lifecycle;
mod observations;
mod reviews_and_reports;
mod shared;
mod templates;
mod term_preparation;

#[cfg(any(test, feature = "integration-test-support"))]
pub use cycles::get_cycle;
pub use cycles::{create_cycle, list_cycles, update_cycle};
pub use evaluations::{replace_observation_evaluators, submit_my_evaluation};
pub use lifecycle::pending_term_work;
pub use observations::{
    approve_observation_request, cancel_observation, cancel_requested_observation,
    evaluator_availability, get_observation, list_observations, observation_timetable_options,
    request_observation, return_observation_request, update_observation,
    update_requested_observation,
};
pub use reviews_and_reports::{
    acknowledge_observation, approve_observation, certify_observation, cycle_progress,
    cycle_teacher_status, get_observation_review,
};
pub use shared::{
    can_view_observation_results, teacher_can_edit_requested_observation,
    SupervisionObservationListAccess,
};
pub use templates::{create_template, get_template, list_templates, update_template};
pub use term_preparation::apply as apply_term_preparation;

#[cfg(feature = "integration-test-support")]
pub use observations::{
    load_timetable_block_group_context_for_teacher_for_test, resolve_lesson_input_for_test,
    test_bangkok_observation_date, test_day_of_week_matches_observed_at, LessonResolutionCycle,
    LessonResolutionEvidence,
};
