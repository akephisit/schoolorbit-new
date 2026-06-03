pub mod models;
pub mod handlers;

// Export routes
use axum::Router;
use crate::AppState;

pub fn facility_routes() -> Router<AppState> {
    handlers::routes()
}
