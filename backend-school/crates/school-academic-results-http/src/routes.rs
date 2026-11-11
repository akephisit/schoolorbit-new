use crate::state::AcademicResultsHttpState;
use axum::Router;

#[inline(never)]
pub fn academic_routes() -> Router<AcademicResultsHttpState> {
    crate::results::routes()
}
