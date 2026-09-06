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
        "/results",
        Router::new()
            .route(
                "/policies",
                get(handlers::list_policies).post(handlers::create_policy),
            )
            .route(
                "/policies/{policy_id}/activate",
                post(handlers::activate_policy),
            )
            .route("/readiness", get(handlers::readiness))
            .route(
                "/groups/{group_id}/course/selection",
                put(handlers::save_selection),
            )
            .route(
                "/groups/{group_id}/course/confirm",
                post(handlers::confirm_group_results),
            )
            .route(
                "/groups/{group_id}/course",
                get(handlers::get_course_workspace),
            )
            .route(
                "/groups/{group_id}/activity/outcomes",
                put(handlers::save_activity_outcomes),
            )
            .route(
                "/groups/{group_id}/activity/confirm",
                post(handlers::confirm_activity),
            )
            .route(
                "/groups/{group_id}/activity",
                get(handlers::get_activity_workspace),
            ),
    )
}
