pub mod handlers;
pub mod models;
pub mod services;

#[cfg(test)]
mod services_tests;

use crate::AppState;
use axum::Router;

pub fn supervision_routes() -> Router<AppState> {
    handlers::routes()
}
