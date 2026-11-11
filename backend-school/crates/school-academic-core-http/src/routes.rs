use crate::state::AcademicCoreHttpState;
use axum::Router;

#[inline(never)]
pub fn academic_routes() -> Router<AcademicCoreHttpState> {
    crate::core::routes()
}

#[inline(never)]
pub fn public_routes() -> Router<AcademicCoreHttpState> {
    Router::new().route(
        "/public/academic-context/options",
        axum::routing::get(crate::core::handlers::list_public_context_options),
    )
}

#[inline(never)]
pub fn personal_routes() -> Router<AcademicCoreHttpState> {
    Router::new().route(
        "/me/academic-context/options",
        axum::routing::get(crate::core::handlers::list_my_context_options),
    )
}
