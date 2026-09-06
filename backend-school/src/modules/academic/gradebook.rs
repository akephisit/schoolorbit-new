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
        "/gradebook",
        Router::new()
            .route("/subjects", get(handlers::list_subjects))
            .route("/controls", get(handlers::list_controls))
            .route("/controls/{control_id}", put(handlers::update_control))
            .route(
                "/groups/{group_id}/phases/{phase_code}/scores",
                put(handlers::save_scores_batch),
            )
            .route(
                "/groups/{group_id}/phases/{phase_code}/confirm",
                post(handlers::confirm_phase),
            )
            .route(
                "/groups/{group_id}/phases/{phase_code}/items",
                post(handlers::create_item),
            )
            .route(
                "/groups/{group_id}/phases/{phase_code}/items/{item_id}",
                put(handlers::update_item).delete(handlers::remove_item),
            )
            .route(
                "/groups/{group_id}/phases/{phase_code}",
                get(handlers::get_group_phase_workspace),
            ),
    )
}
