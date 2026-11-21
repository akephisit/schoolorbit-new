use super::{handlers, state::BehaviorHttpState};
use axum::{
    routing::{get, post},
    Router,
};

pub fn protected_routes() -> Router<BehaviorHttpState> {
    Router::new()
        .route("/api/behavior/overview", get(handlers::overview))
        .route(
            "/api/behavior/items",
            get(handlers::list_items).post(handlers::save_item),
        )
        .route(
            "/api/behavior/rules",
            get(handlers::list_rules).post(handlers::save_rule),
        )
        .route(
            "/api/behavior/settings",
            get(handlers::get_settings).put(handlers::save_settings),
        )
        .route("/api/behavior/students", get(handlers::students))
        .route("/api/behavior/options", get(handlers::selector_options))
        .route("/api/behavior/batches", post(handlers::create_batch))
        .route("/api/behavior/entries", get(handlers::report))
        .route(
            "/api/behavior/entries/cancel",
            post(handlers::cancel_entries),
        )
        .route("/api/behavior/cycles", get(handlers::cycles))
        .route(
            "/api/behavior/cleanup/preview",
            post(handlers::preview_cleanup),
        )
        .route("/api/behavior/cleanup", post(handlers::execute_cleanup))
        .route("/api/behavior/cleanup/{id}", get(handlers::cleanup_job))
        .route(
            "/api/behavior/history/adjustments",
            post(handlers::adjust_history),
        )
        .route("/api/behavior/reset/preview", post(handlers::preview_reset))
        .route("/api/behavior/reset", post(handlers::reset))
        .route(
            "/api/behavior/students/{id}/summary",
            get(handlers::student_summary),
        )
}
