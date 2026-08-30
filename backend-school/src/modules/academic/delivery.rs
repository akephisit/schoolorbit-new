pub mod handlers;
pub mod models;
pub mod services;

use axum::routing::{delete, get, post, put};
use axum::Router;

use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/delivery/workspace", get(handlers::get_delivery_overview))
        .route(
            "/delivery/homerooms",
            get(handlers::get_homeroom_delivery_workspace),
        )
        .route(
            "/delivery/management-options",
            get(handlers::get_delivery_management_options),
        )
        .route(
            "/offerings",
            get(handlers::list_offerings).post(handlers::create_offering),
        )
        .route(
            "/offerings/preview-from-curriculum",
            post(handlers::preview_offerings_from_curriculum),
        )
        .route(
            "/offerings/apply-from-curriculum",
            post(handlers::apply_offerings_from_curriculum),
        )
        .route(
            "/offerings/{id}",
            get(handlers::get_offering).patch(handlers::update_offering),
        )
        .route("/offerings/{id}/publish", post(handlers::publish_offering))
        .route(
            "/offerings/{id}/groups",
            get(handlers::list_groups).post(handlers::create_group),
        )
        .route("/learning-groups", get(handlers::list_groups_for_term))
        .route(
            "/learning-groups/{id}",
            get(handlers::get_group).patch(handlers::update_group),
        )
        .route(
            "/learning-groups/{id}/homerooms",
            get(handlers::list_group_homerooms).put(handlers::replace_group_homerooms),
        )
        .route(
            "/learning-groups/{id}/teachers",
            get(handlers::list_group_teachers).put(handlers::replace_group_teachers),
        )
        .route(
            "/learning-groups/{id}/roster",
            get(handlers::preview_group_roster).put(handlers::apply_group_roster),
        )
        .route(
            "/learning-groups/{id}/roster/publish",
            post(handlers::publish_group_roster),
        )
        .route(
            "/term-change-sets",
            get(handlers::list_term_change_sets).post(handlers::create_term_change_set),
        )
        .route(
            "/term-change-sets/{id}",
            get(handlers::get_term_change_set).patch(handlers::update_term_change_set),
        )
        .route(
            "/term-change-sets/{id}/cancel",
            post(handlers::cancel_term_change_set),
        )
        .route(
            "/term-change-sets/{id}/items",
            put(handlers::upsert_term_change_item),
        )
        .route(
            "/term-change-sets/{id}/items/{itemId}",
            delete(handlers::delete_term_change_item),
        )
        .route(
            "/term-change-sets/{id}/preview",
            get(handlers::preview_term_change_set),
        )
        .route(
            "/term-change-sets/{id}/publish",
            post(handlers::publish_term_change_set),
        )
        .route(
            "/learning-groups/{id}/memberships",
            get(handlers::list_group_memberships).post(handlers::add_group_membership),
        )
        .route(
            "/learning-groups/{id}/memberships/{membershipId}/end",
            post(handlers::end_group_membership),
        )
}

#[cfg(test)]
mod services_tests;
