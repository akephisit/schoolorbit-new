use super::{handlers, state::AttendanceHttpState};
use axum::Router;
pub fn protected_routes() -> Router<AttendanceHttpState> {
    handlers::routes()
}
