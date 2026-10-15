use super::handlers;
use crate::http::state::AdministrationHttpState;
use axum::{
    routing::{get, post, put},
    Router,
};
pub fn supply_catalog_routes() -> Router<AdministrationHttpState> {
    Router::new()
        .route(
            "/categories",
            get(handlers::list_categories).post(handlers::create_category),
        )
        .route("/categories/{id}", put(handlers::update_category))
        .route(
            "/items",
            get(handlers::list_items).post(handlers::create_item),
        )
        .route("/duplicates", get(handlers::duplicates))
        .route(
            "/items/{id}",
            get(handlers::get_item).put(handlers::update_item),
        )
        .route(
            "/items/{id}/versions",
            get(handlers::list_versions).post(handlers::append_version),
        )
        .route(
            "/items/{id}/versions/{version_id}",
            put(handlers::update_version),
        )
        .route("/items/{id}/favorite", put(handlers::set_favorite))
        .route(
            "/bundles",
            get(handlers::list_bundles).post(handlers::create_bundle),
        )
        .route(
            "/bundles/{id}",
            put(handlers::update_bundle).delete(handlers::delete_bundle),
        )
        .route(
            "/proposals",
            get(handlers::list_proposals).post(handlers::create_proposal),
        )
        .route("/proposals/{id}", get(handlers::get_proposal))
        .route("/proposals/{id}/approve", post(handlers::approve_proposal))
        .route("/proposals/{id}/reject", post(handlers::reject_proposal))
}
