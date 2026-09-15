pub mod handlers;

#[cfg(test)]
mod observation_integration_tests;
#[cfg(test)]
mod services_tests;

use crate::AppState;
use axum::Router;

pub fn supervision_routes() -> Router<AppState> {
    handlers::routes()
}
