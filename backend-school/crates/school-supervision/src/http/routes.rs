use super::{handlers, state::SupervisionHttpState};
use axum::Router;
pub fn protected_routes() -> Router<SupervisionHttpState> {
    handlers::routes()
}
