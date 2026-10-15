use super::handlers;
use crate::AppState;
use axum::{
    routing::{delete, get},
    Router,
};

pub fn public_routes() -> Router<AppState> {
    Router::new()
        .route("/school/public", get(handlers::get_public_info))
        .route(
            "/school/public/statistics",
            get(handlers::get_public_statistics),
        )
        .route(
            "/school/public/organization",
            get(handlers::get_public_organization),
        )
        .route(
            "/school/public/organization-members/{id}/avatar",
            get(handlers::get_public_organization_avatar),
        )
}

pub fn protected_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/school/settings",
            get(handlers::get_settings).patch(handlers::update_settings),
        )
        .route("/school/settings/logo", delete(handlers::delete_logo))
}
