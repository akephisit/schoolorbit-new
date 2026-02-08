pub mod models;
pub mod handlers;
pub mod websockets;
pub mod services;

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
        .route("/classrooms/{id}", put(handlers::update_classroom))

        // Enrollments
        .route("/enrollments", post(handlers::enroll_students))
        .route("/enrollments/class/{id}", get(handlers::get_class_enrollments))
        .route("/enrollments/{id}", axum::routing::delete(handlers::remove_enrollment))
        .route("/enrollments/{id}/number", put(handlers::update_enrollment_number))
        .route("/enrollments/class/{id}/auto-number", post(handlers::auto_assign_class_numbers))

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

        // Study Plans (หลักสูตรสถานศึกษา)
        .route("/study-plans", get(handlers::study_plans::list_study_plans).post(handlers::study_plans::create_study_plan))
        .route("/study-plans/{id}", get(handlers::study_plans::get_study_plan).put(handlers::study_plans::update_study_plan).delete(handlers::study_plans::delete_study_plan))
        
        // Study Plan Versions
        .route("/study-plan-versions", get(handlers::study_plans::list_study_plan_versions).post(handlers::study_plans::create_study_plan_version))
        .route("/study-plan-versions/{id}", get(handlers::study_plans::get_study_plan_version).put(handlers::study_plans::update_study_plan_version).delete(handlers::study_plans::delete_study_plan_version))
        
        // Study Plan Subjects
        .route("/study-plan-versions/{id}/subjects", get(handlers::study_plans::list_study_plan_subjects).post(handlers::study_plans::add_subjects_to_version))
        .route("/study-plan-subjects/{id}", axum::routing::delete(handlers::study_plans::delete_study_plan_subject))
        
        // Bulk: Generate Courses from Study Plan
        .route("/planning/generate-from-plan", post(handlers::study_plans::generate_courses_from_plan))
        
        // Auto-Scheduling
        .route("/scheduling/auto-schedule", post(handlers::scheduling::auto_schedule_timetable))
        .route("/scheduling/jobs", get(handlers::scheduling::list_scheduling_jobs))
        .route("/scheduling/jobs/:id", get(handlers::scheduling::get_scheduling_job))
        
        // Instructor Preferences
        .route("/instructor-preferences", post(handlers::scheduling::create_instructor_preference))
        
        // Instructor Room Assignments
        .route("/instructor-rooms", post(handlers::scheduling::create_instructor_room_assignment))
        
        // Locked Slots
        .route("/timetable/locked-slots", post(handlers::scheduling::create_locked_slot).get(handlers::scheduling::list_locked_slots))
        .route("/timetable/locked-slots/:id", axum::routing::delete(handlers::scheduling::delete_locked_slot))
}
