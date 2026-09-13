pub mod handlers;
pub mod models;
pub mod services;

pub fn routes() -> axum::Router<crate::AppState> {
    use axum::{
        routing::{get, post, put},
        Router,
    };
    Router::new()
        .route(
            "/lifecycle/opening-policy",
            get(handlers::get_opening_policy).put(handlers::update_opening_policy),
        )
        .route(
            "/lifecycle/promotion-runs",
            get(handlers::list_promotion_runs).post(handlers::create_promotion_run),
        )
        .route(
            "/lifecycle/promotion-runs/{run_id}",
            get(handlers::get_promotion_run_workspace),
        )
        .route(
            "/lifecycle/promotion-runs/{run_id}/calculate",
            post(handlers::calculate_promotion_run),
        )
        .route(
            "/lifecycle/promotion-runs/{run_id}/impacts",
            get(handlers::promotion_runs::get_promotion_run_impacts),
        )
        .route(
            "/lifecycle/promotion-runs/{run_id}/impacts/{impact_id}/resolve",
            post(handlers::promotion_runs::resolve_promotion_run_impact),
        )
        .route(
            "/lifecycle/promotion-runs/{run_id}/items/{item_id}",
            put(handlers::review_promotion_run_item),
        )
        .route(
            "/lifecycle/promotion-runs/{run_id}/approve",
            post(handlers::approve_promotion_run),
        )
        .route(
            "/lifecycle/promotion-runs/{run_id}/execute",
            post(handlers::execute_promotion_run),
        )
        .route(
            "/lifecycle/promotion-policies/options",
            get(handlers::get_promotion_policy_options),
        )
        .route(
            "/lifecycle/promotion-policies",
            get(handlers::list_promotion_policies).post(handlers::create_promotion_policy),
        )
        .route(
            "/lifecycle/years/{year_id}",
            get(handlers::get_year_workspace),
        )
        .route(
            "/lifecycle/years/{year_id}/reopening",
            get(handlers::year_reopening::get_year_reopening_workspace)
                .post(handlers::year_reopening::reopen_year),
        )
        .route(
            "/lifecycle/years/{year_id}/transitions",
            post(handlers::transition_year),
        )
        .route("/lifecycle/terms/{term_id}", get(handlers::get_workspace))
        .route(
            "/lifecycle/terms/{term_id}/activation",
            get(handlers::get_activation_workspace),
        )
        .route(
            "/lifecycle/terms/{term_id}/transitions",
            post(handlers::transition_term),
        )
        .route(
            "/lifecycle/term-preparations/preview",
            post(handlers::preview_term_preparation),
        )
        .route(
            "/lifecycle/term-preparations/apply",
            post(handlers::apply_term_preparation),
        )
}
