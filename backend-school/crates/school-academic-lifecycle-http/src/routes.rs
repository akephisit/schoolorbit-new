use crate::state::AcademicLifecycleHttpState;
use axum::Router;

#[inline(never)]
pub fn academic_routes() -> Router<AcademicLifecycleHttpState> {
    crate::lifecycle::routes()
}
