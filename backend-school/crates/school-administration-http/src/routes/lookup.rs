use crate::handlers;
use crate::state::AdministrationHttpState;
use axum::{routing::get, Router};

#[inline(never)]
pub fn protected_routes() -> Router<AdministrationHttpState> {
    Router::new()
        .route("/lookup/staff", get(handlers::lookup::lookup_staff))
        .route("/lookup/students", get(handlers::lookup::lookup_students))
        .route("/lookup/rooms", get(handlers::lookup::lookup_rooms))
        .route("/lookup/roles", get(handlers::lookup::lookup_roles))
        .route(
            "/lookup/organization-units",
            get(handlers::lookup::lookup_organization_units),
        )
        .route(
            "/lookup/organization-units/{id}",
            get(handlers::lookup::lookup_organization_unit_by_id),
        )
        .route(
            "/lookup/grade-levels",
            get(handlers::lookup::lookup_grade_levels),
        )
        .route("/lookup/homerooms", get(handlers::lookup::lookup_homerooms))
        .route(
            "/lookup/academic-years",
            get(handlers::lookup::lookup_academic_years),
        )
        .route("/lookup/subjects", get(handlers::lookup::lookup_subjects))
}
