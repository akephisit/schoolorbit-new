pub mod models;
pub mod handlers;

use axum::{
    routing::{get, post, put, delete, patch},
    Router,
};
use crate::AppState;

pub fn admission_routes() -> Router<AppState> {
    Router::new()
        // === Rounds CRUD ===
        .route("/rounds", get(handlers::rounds::list_rounds)
                          .post(handlers::rounds::create_round))
        .route("/rounds/{id}", get(handlers::rounds::get_round)
                               .put(handlers::rounds::update_round)
                               .delete(handlers::rounds::delete_round))
        .route("/rounds/{id}/status", put(handlers::rounds::update_round_status))

        // === Exam Subjects ===
        .route("/rounds/{id}/subjects", get(handlers::rounds::list_subjects)
                                        .post(handlers::rounds::create_subject))
        .route("/subjects/{id}", put(handlers::rounds::update_subject)
                                 .delete(handlers::rounds::delete_subject))

        // === Tracks (สายการเรียน) ===
        .route("/rounds/{id}/tracks", get(handlers::rounds::list_tracks)
                                      .post(handlers::rounds::create_track))
        .route("/tracks/{id}", put(handlers::rounds::update_track)
                               .delete(handlers::rounds::delete_track))
        .route("/tracks/{id}/capacity", get(handlers::rounds::get_track_capacity))

        // === Applications (Public: submit ไม่ต้อง auth) ===
        .route("/apply/rounds", get(handlers::rounds::list_public_rounds))
        .route("/apply/round/{id}", get(handlers::rounds::get_public_round_info))
        .route("/apply/{round_id}", post(handlers::applications::submit_application))
        .route("/rounds/{id}/applications", get(handlers::applications::list_applications))
        .route("/applications/{id}", get(handlers::applications::get_application))
        .route("/applications/{id}/verify", put(handlers::applications::verify_application))
        .route("/applications/{id}/reject", put(handlers::applications::reject_application))
        .route("/applications/{id}", delete(handlers::applications::delete_application))
        .route("/applications/{id}/track", patch(handlers::applications::change_application_track))

        // === Scores ===
        .route("/rounds/{id}/scores", get(handlers::scores::get_all_scores))
        .route("/applications/{id}/scores", get(handlers::scores::get_application_scores)
                                            .put(handlers::scores::update_scores))
        .route("/rounds/{id}/scores/bulk", put(handlers::scores::bulk_update_scores))

        // === Selections (เรียงคะแนน + จัดห้อง) ===
        .route("/rounds/{id}/ranking", get(handlers::selections::get_ranking))
        .route("/tracks/{id}/ranking", get(handlers::selections::get_track_ranking))
        .route("/rounds/{id}/assign-rooms", post(handlers::selections::assign_rooms))

        // === Enrollment (มอบตัว) ===
        .route("/rounds/{id}/enrollment", get(handlers::applications::list_enrollment_pending))
        .route("/applications/{id}/enroll", post(handlers::applications::complete_enrollment))

        // === Portal (Applicant Stateless — ส่ง credentials ทุก request) ===
        .route("/portal/check", post(handlers::portal::check_application))
        .route("/portal/status", post(handlers::portal::get_status))
        .route("/portal/confirm", post(handlers::portal::confirm_enrollment))
        .route("/portal/form", post(handlers::portal::get_enrollment_form)
                               .put(handlers::portal::submit_enrollment_form))
        .route("/portal/application", put(handlers::portal::update_application))
        .route("/portal/upload", post(handlers::portal::portal_upload_document))
        .route("/portal/documents/{doc_type}", delete(handlers::portal::portal_delete_document))
}
