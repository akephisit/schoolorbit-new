pub mod handlers;
pub mod models;
pub mod services;

#[cfg(test)]
mod services_tests;

use crate::AppState;
use axum::routing::{get, put};
use axum::Router;

pub fn calendar_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/events",
            get(handlers::list_calendar_events).post(handlers::create_calendar_event),
        )
        .route(
            "/events/{id}",
            put(handlers::update_calendar_event).delete(handlers::delete_calendar_event),
        )
        .route(
            "/categories",
            get(handlers::list_calendar_categories).post(handlers::create_calendar_category),
        )
        .route(
            "/categories/{id}",
            put(handlers::update_calendar_category).delete(handlers::delete_calendar_category),
        )
        .route(
            "/tags",
            get(handlers::list_calendar_tags).post(handlers::create_calendar_tag),
        )
        .route(
            "/tags/{id}",
            put(handlers::update_calendar_tag).delete(handlers::delete_calendar_tag),
        )
}
