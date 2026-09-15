pub mod handlers;

use crate::AppState;
use axum::Router;

pub fn work_routes() -> Router<AppState> {
    handlers::routes()
}
