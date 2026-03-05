pub mod handlers;
pub mod models;

use axum::{
    Router,
    routing::{get, post, put, delete},
};
use crate::AppState;

pub fn admission_routes() -> Router<AppState> {
    Router::new()
        // ── Periods ─────────────────────────────────────────────────
        .route("/periods",                get(handlers::list_periods).post(handlers::create_period))
        .route("/periods/{id}",           get(handlers::get_period).put(handlers::update_period).delete(handlers::delete_period))
        .route("/periods/{id}/stats",     get(handlers::get_period_stats))

        // ── Exam Subjects (per period) ───────────────────────────────
        .route("/periods/{id}/subjects",         get(handlers::list_exam_subjects).post(handlers::create_exam_subject))
        .route("/periods/{id}/subjects/{sid}",   put(handlers::update_exam_subject).delete(handlers::delete_exam_subject))

        // ── Scores ──────────────────────────────────────────────────
        .route("/periods/{id}/scores",           get(handlers::list_scores_by_period))
        .route("/scores/batch",                  post(handlers::batch_upsert_scores))

        // ── Selections ──────────────────────────────────────────────
        .route("/periods/{id}/selections",       get(handlers::list_selections).post(handlers::create_selections))
        .route("/selections/{id}",               put(handlers::update_selection))
        .route("/selections/{id}/confirm",       post(handlers::confirm_selection))

        // ── Check-in ────────────────────────────────────────────────
        .route("/periods/{id}/checkin",          get(handlers::list_checkins))
        .route("/periods/{id}/checkin/stats",    get(handlers::get_checkin_stats))
        .route("/selections/{id}/checkin",       post(handlers::confirm_checkin))
        .route("/selections/{id}/absent",        post(handlers::mark_absent))

        // ── Applications ────────────────────────────────────────────
        .route("/applications",                  get(handlers::list_applications).post(handlers::create_application))
        .route("/applications/{id}",             get(handlers::get_application).put(handlers::update_application_status))
        .route("/applications/{id}/logs",        get(handlers::get_application_logs))

        // ── Interviews ──────────────────────────────────────────────
        .route("/interviews",                    post(handlers::create_interview))
        .route("/interviews/{id}",               put(handlers::update_interview))

        // ── Generate Students (legacy) ───────────────────────────────
        .route("/periods/{id}/generate-students", post(handlers::generate_students))
}
