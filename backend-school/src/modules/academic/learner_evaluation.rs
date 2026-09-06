pub mod handlers;
pub mod models;
pub mod services;
#[cfg(test)]
mod services_tests;

pub fn routes() -> axum::Router<crate::AppState> {
    use axum::{
        routing::{get, post, put},
        Router,
    };
    Router::new().nest(
        "/learner-evaluations",
        Router::new()
            .route(
                "/policies",
                get(handlers::list_policies).post(handlers::create_policy),
            )
            .route(
                "/policies/{policy_id}/activate",
                post(handlers::activate_policy),
            )
            .route("/subjects", get(handlers::list_subjects))
            .route(
                "/catalog",
                get(handlers::list_catalog).post(handlers::create_catalog),
            )
            .route(
                "/catalog/{criterion_id}",
                put(handlers::update_catalog).delete(handlers::remove_catalog),
            )
            .route("/controls", get(handlers::list_controls))
            .route("/controls/{domain}", put(handlers::update_control))
            .route(
                "/subjects/{subject_id}/domains/{domain}/configuration",
                get(handlers::get_configuration),
            )
            .route(
                "/subjects/{subject_id}/domains/{domain}/criteria",
                post(handlers::create_criterion),
            )
            .route(
                "/subjects/{subject_id}/domains/{domain}/criteria/{criterion_id}",
                put(handlers::update_criterion).delete(handlers::remove_criterion),
            )
            .route(
                "/subjects/{subject_id}/domains/{domain}/lock",
                post(handlers::lock_subject),
            )
            .route(
                "/groups/{group_id}/domains/{domain}",
                get(handlers::get_workspace),
            )
            .route(
                "/groups/{group_id}/domains/{domain}/responses",
                put(handlers::save_responses),
            )
            .route(
                "/groups/{group_id}/domains/{domain}/confirm",
                post(handlers::confirm_group),
            )
            .route(
                "/students/{student_academic_year_id}/summary",
                get(handlers::student_summary),
            ),
    )
}
