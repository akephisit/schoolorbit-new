use super::{handlers, state::BehaviorHttpState};
use axum::Router;
pub fn protected_routes() -> Router<BehaviorHttpState> {
    handlers::routes()
}
