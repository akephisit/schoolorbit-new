use super::handlers;
use crate::AppState;
use axum::{
    routing::{get, post},
    Router,
};

pub fn public_routes() -> Router<AppState> {
    Router::new().route(
        "/admin/routes/sync",
        post(handlers::register_routes::register_routes),
    )
}

pub fn protected_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/admin/features",
            get(handlers::feature_toggles::list_features),
        )
        .route(
            "/admin/features/{id}",
            get(handlers::feature_toggles::get_feature)
                .put(handlers::feature_toggles::update_feature),
        )
        .route(
            "/admin/features/{id}/toggle",
            post(handlers::feature_toggles::toggle_feature),
        )
}
