use super::handlers;
use crate::http::state::NotificationHttpState;
use axum::{
    routing::{get, post},
    Router,
};

pub fn protected_routes() -> Router<NotificationHttpState> {
    Router::new()
        .route("/notifications/stream", get(handlers::stream_notifications))
        .route(
            "/notifications",
            get(handlers::list_notifications).post(handlers::create_notification),
        )
        .route("/notifications/read-all", post(handlers::mark_all_as_read))
        .route("/notifications/{id}/read", post(handlers::mark_as_read))
        .route("/notifications/subscribe", post(handlers::subscribe_push))
}
