use super::handlers;
use crate::state::CalendarHttpState;
use axum::{routing::get, Router};

pub fn public_routes() -> Router<CalendarHttpState> {
    Router::new().route(
        "/public/calendar/events",
        get(handlers::list_public_calendar_events),
    )
}

pub fn protected_routes() -> Router<CalendarHttpState> {
    Router::new().route(
        "/me/calendar/events",
        get(handlers::list_my_calendar_events),
    )
}

use axum::routing::put;

pub fn management_routes() -> Router<CalendarHttpState> {
    Router::new()
        .route(
            "/requests",
            get(handlers::requests::list_requests).post(handlers::requests::create_request),
        )
        .route(
            "/requests/calendar",
            get(handlers::requests::list_pending_calendar),
        )
        .route(
            "/requests/pending-count",
            get(handlers::requests::get_pending_count),
        )
        .route(
            "/requests/{id}",
            get(handlers::requests::get_request_for_review),
        )
        .route(
            "/requests/{id}/approve",
            axum::routing::post(handlers::requests::approve_request),
        )
        .route(
            "/requests/{id}/reject",
            axum::routing::post(handlers::requests::reject_request),
        )
        .route(
            "/target-options",
            get(handlers::requests::list_target_options),
        )
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
