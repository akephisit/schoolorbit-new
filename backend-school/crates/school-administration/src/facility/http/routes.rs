use super::handlers;
use crate::http::state::AdministrationHttpState;
use axum::Router;
pub fn facility_routes() -> Router<AdministrationHttpState> {
    handlers::routes()
}
