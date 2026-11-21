use crate::handlers;
use crate::state::AdministrationHttpState;
use axum::{
    routing::{delete, get},
    Router,
};

#[inline(never)]
pub fn public_routes() -> Router<AdministrationHttpState> {
    Router::new()
        .route("/school/public", get(handlers::settings::get_public_info))
        .route(
            "/school/public/appearance",
            get(handlers::settings::get_public_appearance),
        )
        .route(
            "/school/public/favicon",
            get(handlers::settings::get_public_favicon),
        )
        .route(
            "/school/public/share-image",
            get(handlers::settings::get_public_share_image),
        )
        .route(
            "/school/public/statistics",
            get(handlers::settings::get_public_statistics),
        )
        .route(
            "/school/public/organization",
            get(handlers::settings::get_public_organization),
        )
        .route(
            "/school/public/organization-members/{id}/avatar",
            get(handlers::settings::get_public_organization_avatar),
        )
}

#[inline(never)]
pub fn protected_routes() -> Router<AdministrationHttpState> {
    Router::new()
        .route(
            "/school/settings",
            get(handlers::settings::get_settings).patch(handlers::settings::update_settings),
        )
        .route(
            "/school/settings/logo",
            delete(handlers::settings::delete_logo),
        )
        .route(
            "/school/settings/appearance",
            get(handlers::settings::get_appearance).patch(handlers::settings::update_appearance),
        )
}
