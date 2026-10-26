use super::handlers;
use crate::http::state::NotificationHttpState;
use axum::{
    routing::{delete, get, post},
    Router,
};

pub fn protected_routes() -> Router<NotificationHttpState> {
    Router::new()
        .route("/notifications/stream", get(handlers::stream_notifications))
        .route("/notifications", get(handlers::list_notifications))
        .route("/notifications/read-all", post(handlers::mark_all_as_read))
        .route("/notifications/{id}/read", post(handlers::mark_as_read))
        .route("/notifications/push/config", get(handlers::push_config))
        .route(
            "/notifications/push/devices",
            get(handlers::list_push_devices).post(handlers::register_push_device),
        )
        .route(
            "/notifications/push/devices/{id}",
            delete(handlers::unregister_push_device),
        )
}
