use super::handlers;
use crate::AppState;
use axum::{routing::get, Router};

pub fn public_routes() -> Router<AppState> {
    Router::new().route(
        "/public/calendar/events",
        get(handlers::list_public_calendar_events),
    )
}

pub fn protected_routes() -> Router<AppState> {
    Router::new().route(
        "/me/calendar/events",
        get(handlers::list_my_calendar_events),
    )
}
