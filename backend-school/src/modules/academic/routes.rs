use super::websockets;
use crate::AppState;
use axum::{routing::get, Router};

// WebSocket authentication runs in the handler; query selects academic term only.
pub fn websocket_routes() -> Router<AppState> {
    Router::new().route("/timetable", get(websockets::timetable_websocket_handler))
}
