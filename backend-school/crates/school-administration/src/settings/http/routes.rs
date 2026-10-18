use super::handlers;
use crate::http::state::AdministrationHttpState;
use axum::{
    routing::{delete, get},
    Router,
};

pub fn public_routes() -> Router<AdministrationHttpState> {
    Router::new()
        .route("/school/public", get(handlers::get_public_info))
        .route("/school/public/favicon", get(handlers::get_public_favicon))
        .route(
            "/school/public/share-image",
            get(handlers::get_public_share_image),
        )
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

pub fn protected_routes() -> Router<AdministrationHttpState> {
    Router::new()
        .route(
            "/school/settings",
            get(handlers::get_settings).patch(handlers::update_settings),
        )
        .route("/school/settings/logo", delete(handlers::delete_logo))
}
