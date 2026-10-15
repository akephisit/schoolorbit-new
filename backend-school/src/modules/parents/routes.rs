use super::handlers;
use crate::AppState;
use axum::{routing::get, Router};

pub fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/parent/profile", get(handlers::get_own_parent_profile))
        .route(
            "/parent/academic-context/options",
            get(handlers::get_parent_academic_context_options),
        )
        .route(
            "/parent/students/{student_id}",
            get(handlers::get_child_profile),
        )
        .route(
            "/parent/students/{student_id}/timetable",
            get(handlers::get_child_timetable),
        )
        .route(
            "/parent/students/{student_id}/academic-context/options",
            get(handlers::get_child_academic_context_options),
        )
        .route(
            "/parent/students/{student_id}/exam-schedules",
            get(handlers::get_child_exam_schedule),
        )
        .route(
            "/parent/students/{student_id}/calendar/events",
            get(handlers::get_child_calendar_events),
        )
}
