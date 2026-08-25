pub mod handlers;
pub mod models;
pub mod services;

use axum::routing::{get, post};
use axum::Router;

use crate::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
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
}

#[cfg(test)]
mod services_tests;
