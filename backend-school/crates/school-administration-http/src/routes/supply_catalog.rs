use crate::handlers;
use crate::state::AdministrationHttpState;
use axum::{
    routing::{get, post, put},
    Router,
};
#[inline(never)]
pub fn supply_catalog_routes() -> Router<AdministrationHttpState> {
    Router::new()
        .route(
            "/categories",
            get(handlers::supply_catalog::list_categories)
                .post(handlers::supply_catalog::create_category),
        )
        .route(
            "/categories/{id}",
            put(handlers::supply_catalog::update_category),
        )
        .route(
            "/items",
            get(handlers::supply_catalog::list_items).post(handlers::supply_catalog::create_item),
        )
        .route("/duplicates", get(handlers::supply_catalog::duplicates))
        .route(
            "/items/{id}",
            get(handlers::supply_catalog::get_item).put(handlers::supply_catalog::update_item),
        )
        .route(
            "/items/{id}/versions",
            get(handlers::supply_catalog::list_versions)
                .post(handlers::supply_catalog::append_version),
        )
        .route(
            "/items/{id}/versions/{version_id}",
            put(handlers::supply_catalog::update_version),
        )
        .route(
            "/items/{id}/favorite",
            put(handlers::supply_catalog::set_favorite),
        )
        .route(
            "/bundles",
            get(handlers::supply_catalog::list_bundles)
                .post(handlers::supply_catalog::create_bundle),
        )
        .route(
            "/bundles/{id}",
            put(handlers::supply_catalog::update_bundle)
                .delete(handlers::supply_catalog::delete_bundle),
        )
        .route(
            "/proposals",
            get(handlers::supply_catalog::list_proposals)
                .post(handlers::supply_catalog::create_proposal),
        )
        .route(
            "/proposals/{id}",
            get(handlers::supply_catalog::get_proposal),
        )
        .route(
            "/proposals/{id}/approve",
            post(handlers::supply_catalog::approve_proposal),
        )
        .route(
            "/proposals/{id}/reject",
            post(handlers::supply_catalog::reject_proposal),
        )
}
