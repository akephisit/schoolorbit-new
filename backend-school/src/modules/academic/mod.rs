pub mod models;
pub mod handlers;
pub mod websockets;

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
        .route("/years/{id}/levels", get(handlers::get_year_levels).put(handlers::update_year_levels))

        // Semesters
        .route("/semesters", post(handlers::create_semester))
        .route("/semesters/{id}", put(handlers::update_semester).delete(handlers::delete_semester))

        // Classrooms
        .route("/classrooms", get(handlers::list_classrooms).post(handlers::create_classroom))

        // Enrollments
        .route("/enrollments", post(handlers::enroll_students))
        .route("/enrollments/class/{id}", get(handlers::get_class_enrollments))
        .route("/enrollments/{id}", axum::routing::delete(handlers::remove_enrollment))
        .route("/enrollments/{id}/number", put(handlers::update_enrollment_number))

        // Curriculum: Subjects
        .route("/subjects/groups", get(handlers::subjects::list_subject_groups))
        .route("/subjects/bulk-copy", post(handlers::subjects::bulk_copy_subjects))
        .route("/subjects", get(handlers::subjects::list_subjects).post(handlers::subjects::create_subject))
        .route("/subjects/{id}", put(handlers::subjects::update_subject).delete(handlers::subjects::delete_subject))

        // Course Planning
        .route("/planning/courses", get(handlers::course_planning::list_classroom_courses).post(handlers::course_planning::assign_courses))
        .route("/planning/courses/{id}", put(handlers::course_planning::update_course).delete(handlers::course_planning::remove_course))

        // Timetable: Periods
        .route("/periods", get(handlers::timetable::list_periods).post(handlers::timetable::create_period))
        .route("/periods/{id}", put(handlers::timetable::update_period).delete(handlers::timetable::delete_period))

        // Timetable: Entries
        .route("/timetable", get(handlers::timetable::list_timetable_entries).post(handlers::timetable::create_timetable_entry))
        .route("/timetable/batch", post(handlers::timetable::create_batch_timetable_entries))
        .route("/timetable/{id}", axum::routing::put(handlers::timetable::update_timetable_entry).delete(handlers::timetable::delete_timetable_entry))
}
