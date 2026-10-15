use super::handlers;
use crate::AppState;
use axum::{routing::get, Router};

pub fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/lookup/staff", get(handlers::lookup_staff))
        .route("/lookup/students", get(handlers::lookup_students))
        .route("/lookup/rooms", get(handlers::lookup_rooms))
        .route("/lookup/roles", get(handlers::lookup_roles))
        .route(
            "/lookup/organization-units",
            get(handlers::lookup_organization_units),
        )
        .route(
            "/lookup/organization-units/{id}",
            get(handlers::lookup_organization_unit_by_id),
        )
        .route("/lookup/grade-levels", get(handlers::lookup_grade_levels))
        .route("/lookup/homerooms", get(handlers::lookup_homerooms))
        .route(
            "/lookup/academic-years",
            get(handlers::lookup_academic_years),
        )
        .route("/lookup/subjects", get(handlers::lookup_subjects))
}
