use super::{handlers, state::AttendanceHttpState};
use axum::{
    routing::{get, post, put},
    Router,
};

pub fn protected_routes() -> Router<AttendanceHttpState> {
    Router::new()
        .route(
            "/api/attendance/reset",
            get(handlers::reset_impact).post(handlers::reset_module),
        )
        .route(
            "/api/attendance/settings/{term}",
            get(handlers::get_settings).put(handlers::save_settings),
        )
        .route("/api/attendance/days", get(handlers::list_days))
        .route("/api/attendance/days/{term}", put(handlers::save_days))
        .route("/api/attendance/workspace", get(handlers::workspace))
        .route("/api/attendance/preview", get(handlers::open_session))
        .route(
            "/api/attendance/sessions/{id}",
            get(handlers::get_session).put(handlers::save_results),
        )
        .route(
            "/api/attendance/sessions/{id}/cancellation",
            put(handlers::cancel_session),
        )
        .route("/api/attendance/options", get(handlers::get_options))
        .route(
            "/api/attendance/audiences/{term}",
            post(handlers::create_audience),
        )
        .route(
            "/api/attendance/specials/{term}",
            post(handlers::create_special),
        )
        .route("/api/attendance/devices/{id}", put(handlers::save_device))
        .route("/api/attendance/faces", get(handlers::face_enrollments))
        .route(
            "/api/attendance/faces/{student}",
            put(handlers::enroll_face).delete(handlers::delete_face),
        )
        .route("/api/attendance/kiosk/open", post(handlers::open_kiosk))
        .route("/api/attendance/scans", post(handlers::scan))
        .route("/api/attendance/report", get(handlers::report))
        .route("/api/attendance/history", get(handlers::history))
        .route(
            "/api/attendance/terms/{term}/purge",
            get(handlers::purge_impact).post(handlers::purge_term),
        )
}
