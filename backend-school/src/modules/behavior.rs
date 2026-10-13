pub mod handlers;
use crate::AppState;
use axum::Router;
pub fn behavior_routes() -> Router<AppState> {
    handlers::routes()
}
