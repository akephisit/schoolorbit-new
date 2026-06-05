pub mod handlers;
pub mod models;
pub mod services;

// Export routes
use crate::AppState;
use axum::Router;

pub fn facility_routes() -> Router<AppState> {
    handlers::routes()
}
