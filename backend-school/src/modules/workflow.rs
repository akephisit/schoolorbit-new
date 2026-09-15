pub mod handlers;

use crate::AppState;
use axum::Router;

pub fn workflow_routes() -> Router<AppState> {
    handlers::routes()
}
