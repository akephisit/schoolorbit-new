use super::handlers;
use crate::http::state::NavigationHttpState;
use axum::{
    routing::{get, post, put},
    Router,
};

pub fn protected_routes() -> Router<NavigationHttpState> {
    Router::new()
        .route("/menu/user", get(handlers::public::get_user_menu))
        .route(
            "/admin/menu/workspaces",
            get(handlers::admin::list_menu_workspaces).post(handlers::admin::create_menu_workspace),
        )
        .route(
            "/admin/menu/templates/academic/recommended",
            get(handlers::admin::preview_recommended_academic_menu_template),
        )
        .route(
            "/admin/menu/templates/academic/recommended/apply",
            post(handlers::admin::apply_recommended_academic_menu_template),
        )
        .route(
            "/admin/menu/workspaces/reorder",
            post(handlers::admin::reorder_menu_workspaces),
        )
        .route(
            "/admin/menu/workspaces/{id}",
            put(handlers::admin::update_menu_workspace)
                .delete(handlers::admin::delete_menu_workspace),
        )
        .route(
            "/admin/menu/groups",
            get(handlers::admin::list_menu_groups).post(handlers::admin::create_menu_group),
        )
        .route(
            "/admin/menu/groups/{id}",
            put(handlers::admin::update_menu_group).delete(handlers::admin::delete_menu_group),
        )
        .route(
            "/admin/menu/groups/reorder",
            post(handlers::admin::reorder_menu_groups),
        )
        .route(
            "/admin/menu/items",
            get(handlers::admin::list_menu_items).post(handlers::admin::create_menu_item),
        )
        .route(
            "/admin/menu/items/{id}",
            put(handlers::admin::update_menu_item).delete(handlers::admin::delete_menu_item),
        )
        .route(
            "/admin/menu/items/{id}/group",
            put(handlers::admin::move_item_to_group),
        )
        .route(
            "/admin/menu/items/reorder",
            post(handlers::admin::reorder_menu_items),
        )
}

use crate::http::feature_toggles;
use crate::http::register_routes;

pub fn public_system_routes() -> Router<NavigationHttpState> {
    Router::new().route("/admin/routes/sync", post(register_routes::register_routes))
}

pub fn protected_system_routes() -> Router<NavigationHttpState> {
    Router::new()
        .route("/admin/features", get(feature_toggles::list_features))
        .route(
            "/admin/features/{id}",
            get(feature_toggles::get_feature).put(feature_toggles::update_feature),
        )
        .route(
            "/admin/features/{id}/toggle",
            post(feature_toggles::toggle_feature),
        )
}
