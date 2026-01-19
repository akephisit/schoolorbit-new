pub mod models;
pub mod handlers;

use axum::routing::{get, post, put};
use axum::Router;
use crate::AppState;

pub fn academic_routes() -> Router<AppState> {
    Router::new()
        // Structure (Years, Levels, Semesters)
        .route("/structure", get(handlers::list_academic_structure))
        .route("/levels", post(handlers::create_grade_level))
        .route("/levels/{id}", axum::routing::delete(handlers::delete_grade_level))
        
        // Academic Years
        .route("/years", post(handlers::create_academic_year))
        .route("/years/{id}/active", put(handlers::toggle_active_year))

        // Classrooms
        .route("/classrooms", get(handlers::list_classrooms).post(handlers::create_classroom))

        // Enrollments
        .route("/enrollments", post(handlers::enroll_students))
        .route("/enrollments/class/{id}", get(handlers::get_class_enrollments))
        .route("/enrollments/{id}", axum::routing::delete(handlers::remove_enrollment))

        // Curriculum: Subjects
        .route("/subjects/groups", get(handlers::subjects::list_subject_groups))
        .route("/subjects", get(handlers::subjects::list_subjects).post(handlers::subjects::create_subject))
        .route("/subjects/{id}", put(handlers::subjects::update_subject).delete(handlers::subjects::delete_subject))
}
