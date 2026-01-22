pub mod models;
pub mod handlers;

// Export routes
use axum::{Router, middleware as axum_middleware};
use crate::middleware;
use crate::AppState;

pub fn facility_routes() -> Router<AppState> {
    handlers::routes()
}
