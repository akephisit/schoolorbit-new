pub mod handlers;
pub mod models;
pub mod services;

use crate::AppState;
use axum::Router;

pub fn workflow_routes() -> Router<AppState> {
    handlers::routes()
}
