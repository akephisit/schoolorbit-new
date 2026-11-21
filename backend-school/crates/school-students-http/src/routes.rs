pub mod parents;

use crate::handlers;
use crate::state::StudentHttpState;
use axum::{
    routing::{delete, get, post},
    Router,
};

#[inline(never)]
pub fn protected_routes() -> Router<StudentHttpState> {
    Router::new()
        .route(
            "/student/profile",
            get(handlers::get_own_profile).put(handlers::update_own_profile),
        )
        .route(
            "/students",
            get(handlers::list_students).post(handlers::create_student),
        )
        .route(
            "/students/{id}",
            get(handlers::get_student)
                .put(handlers::update_student)
                .delete(handlers::delete_student),
        )
        .route(
            "/students/{id}/parents",
            post(handlers::relationships::add_parent_to_student),
        )
        .route(
            "/students/{id}/parents/{parent_id}",
            delete(handlers::relationships::remove_parent_from_student),
        )
}
