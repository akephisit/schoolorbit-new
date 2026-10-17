use super::{handlers, state::StudentCareHttpState};
use axum::{
    routing::{get, post, put},
    Router,
};
pub fn protected_routes() -> Router<StudentCareHttpState> {
    Router::new()
        .route("/api/student-care/overview", get(handlers::overview))
        .route(
            "/api/student-care/instruments",
            get(handlers::list_instruments).post(handlers::create_instrument),
        )
        .route(
            "/api/student-care/instruments/{id}",
            get(handlers::get_instrument),
        )
        .route(
            "/api/student-care/rounds/{id}",
            get(handlers::get_round).put(handlers::update_round),
        )
        .route(
            "/api/student-care/rounds",
            get(handlers::list_rounds).post(handlers::create_round),
        )
        .route("/api/student-care/students", get(handlers::list_students))
        .route("/api/student-care/staff", get(handlers::list_staff))
        .route(
            "/api/student-care/assignments",
            get(handlers::list_assignments).post(handlers::create_assignments),
        )
        .route(
            "/api/student-care/assignments/{id}",
            get(handlers::get_assignment),
        )
        .route(
            "/api/student-care/assignments/{id}/draft",
            put(handlers::save_draft),
        )
        .route(
            "/api/student-care/assignments/{id}/submit",
            post(handlers::submit_response),
        )
        .route(
            "/api/student-care/assignments/{id}/cancel",
            post(handlers::cancel_assignment),
        )
        .route(
            "/api/student-care/assignments/{id}/acknowledge",
            post(handlers::acknowledge_assignment),
        )
        .route(
            "/api/student-care/visits",
            get(handlers::list_visits).post(handlers::create_visit),
        )
        .route(
            "/api/student-care/visits/{id}",
            get(handlers::get_visit).put(handlers::update_visit),
        )
        .route(
            "/api/student-care/cases/{id}/events",
            get(handlers::list_case_events).post(handlers::add_case_event),
        )
        .route(
            "/api/student-care/cases",
            get(handlers::list_cases).post(handlers::create_case),
        )
        .route(
            "/api/student-care/cases/{id}",
            get(handlers::get_case).put(handlers::update_case),
        )
}
