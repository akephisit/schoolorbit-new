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
        .route("/years/{id}", put(handlers::update_academic_year))
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
        // Batch endpoint — MUST be registered BEFORE `/subjects/{id}/...`
        // so Axum doesn't match `default-instructors` as `id`.
        .route("/subjects/default-instructors", get(handlers::subjects::batch_list_subject_default_instructors))
        .route("/subjects", get(handlers::subjects::list_subjects).post(handlers::subjects::create_subject))
        .route("/subjects/{id}", put(handlers::subjects::update_subject).delete(handlers::subjects::delete_subject))
        .route("/subjects/{id}/default-instructors",
               get(handlers::subjects::list_subject_default_instructors)
               .post(handlers::subjects::add_subject_default_instructor))
        .route("/subjects/{id}/default-instructors/{uid}",
               axum::routing::delete(handlers::subjects::remove_subject_default_instructor)
               .put(handlers::subjects::update_subject_default_instructor_role))

        // Course Planning
        .route("/planning/plan-subjects", get(handlers::course_planning::list_plan_subjects))
        .route("/planning/courses", get(handlers::course_planning::list_classroom_courses).post(handlers::course_planning::assign_courses))
        .route("/planning/courses/{id}", put(handlers::course_planning::update_course).delete(handlers::course_planning::remove_course))
        // Batch endpoint — MUST be registered BEFORE `/planning/courses/{id}/instructors`
        // so Axum doesn't match the literal path `/planning/courses/instructors` as `id = "instructors"`.
        .route("/planning/courses/instructors", get(handlers::course_planning::batch_list_course_instructors))
        .route("/planning/courses/{id}/instructors",
               get(handlers::course_planning::list_course_instructors)
               .post(handlers::course_planning::add_course_instructor))
        .route("/planning/courses/{id}/instructors/{uid}",
               axum::routing::delete(handlers::course_planning::remove_course_instructor)
               .put(handlers::course_planning::update_course_instructor_role))

        // Timetable: Periods
        .route("/periods", get(handlers::timetable::list_periods).post(handlers::timetable::create_period))
        .route("/periods/{id}", put(handlers::timetable::update_period).delete(handlers::timetable::delete_period))

        // Timetable: Entries
        .route("/timetable", get(handlers::timetable::list_timetable_entries).post(handlers::timetable::create_timetable_entry))
        .route("/timetable/batch", post(handlers::timetable::create_batch_timetable_entries).delete(handlers::timetable::delete_batch_timetable_entries))
        .route("/timetable/{id}", axum::routing::put(handlers::timetable::update_timetable_entry).delete(handlers::timetable::delete_timetable_entry))
        .route("/timetable/{id}/my-activity", get(handlers::timetable::get_my_activity_for_entry))
        .route("/timetable/{id}/instructors", post(handlers::timetable::add_entry_instructor))
        .route("/timetable/{id}/instructors/{uid}", axum::routing::delete(handlers::timetable::remove_entry_instructor))
        .route("/timetable/slots/{slot_id}/instructors/{uid}/restore",
               post(handlers::timetable::restore_instructor_to_slot_entries))
        .route("/timetable/slots/{slot_id}/instructors/{uid}",
               axum::routing::delete(handlers::timetable::hide_instructor_from_slot_entries))

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

        // Study Plan Activities (template)
        .route("/study-plan-versions/{id}/activities",
               get(handlers::study_plans::list_plan_activities)
               .post(handlers::study_plans::add_plan_activity))
        .route("/study-plan-activities/{id}",
               put(handlers::study_plans::update_plan_activity)
               .delete(handlers::study_plans::delete_plan_activity))

        // Generate activities from plan
        .route("/activities/generate-from-plan",
               post(handlers::study_plans::generate_activities_from_plan))

        // Activity Catalog (คลังกิจกรรม — pattern เดียวกับ subjects)
        .route("/activity-catalog",
               get(handlers::study_plans::list_activity_catalog)
               .post(handlers::study_plans::create_activity_catalog))
        .route("/activity-catalog/{id}",
               put(handlers::study_plans::update_activity_catalog)
               .delete(handlers::study_plans::delete_activity_catalog))
        
        // Auto-Scheduling
        .route("/scheduling/auto-schedule", post(handlers::scheduling::auto_schedule_timetable))
        .route("/scheduling/jobs", get(handlers::scheduling::list_scheduling_jobs))
        .route("/scheduling/jobs/{id}", get(handlers::scheduling::get_scheduling_job))
        
        // Instructor Preferences
        .route("/instructor-preferences", post(handlers::scheduling::create_instructor_preference))
        
        // Instructor Room Assignments
        .route("/instructor-rooms", post(handlers::scheduling::create_instructor_room_assignment))
        
        // Locked Slots
        .route("/timetable/locked-slots", post(handlers::scheduling::create_locked_slot).get(handlers::scheduling::list_locked_slots))
        .route("/timetable/locked-slots/{id}", axum::routing::delete(handlers::scheduling::delete_locked_slot))

        // Scheduling Constraints Config
        .route("/scheduling/instructors", get(handlers::scheduling_config::list_instructor_constraints))
        .route("/scheduling/instructors/{id}", put(handlers::scheduling_config::update_instructor_constraints))
        .route("/scheduling/subjects", get(handlers::scheduling_config::list_subject_constraints))
        .route("/scheduling/subjects/{id}", put(handlers::scheduling_config::update_subject_constraints))

        // Activity Slots (ช่องกิจกรรม — Admin)
        .route("/activity-slots", get(handlers::activity::list_activity_slots).post(handlers::activity::create_activity_slot))
        .route("/activity-slots/{id}", put(handlers::activity::update_activity_slot).delete(handlers::activity::delete_activity_slot))
        .route("/activity-slots/{id}/instructors", get(handlers::activity::list_slot_instructors).post(handlers::activity::add_slot_instructor))
        .route("/activity-slots/{id}/groups", axum::routing::delete(handlers::activity::delete_all_slot_groups))
        .route("/activity-slots/{id}/timetable-entries", axum::routing::delete(handlers::activity::delete_slot_timetable_entries))
        .route("/activity-slots/{id}/instructors/all", axum::routing::delete(handlers::activity::remove_all_slot_instructors))
        .route("/activity-slots/{id}/instructors/{user_id}", axum::routing::delete(handlers::activity::remove_slot_instructor))
        .route("/activity-slots/{id}/classroom-assignments", get(handlers::activity::list_slot_classroom_assignments).post(handlers::activity::batch_upsert_slot_classroom_assignments))
        .route("/activity-slots/{id}/classroom-assignments/all", axum::routing::delete(handlers::activity::delete_all_slot_classroom_assignments))
        .route("/activity-slots/{id}/classroom-assignments/{assignment_id}", axum::routing::delete(handlers::activity::delete_slot_classroom_assignment))

        // Activity Groups (กิจกรรมจริง ภายใต้ slot)
        .route("/activities/my-enrollments", get(handlers::activity::my_enrollments))
        .route("/activities", get(handlers::activity::list_activity_groups).post(handlers::activity::create_activity_group))
        .route("/activities/{id}", put(handlers::activity::update_activity_group).delete(handlers::activity::delete_activity_group))
        .route("/activities/{id}/members", get(handlers::activity::list_members).post(handlers::activity::add_members))
        .route("/activities/{id}/enroll", post(handlers::activity::self_enroll).delete(handlers::activity::self_unenroll))
        .route("/activities/{id}/members/{student_id}", axum::routing::delete(handlers::activity::remove_member))
        .route("/activities/members/{member_id}/result", put(handlers::activity::update_member_result))
        .route("/activities/{id}/instructors", get(handlers::activity::list_instructors).post(handlers::activity::add_instructor))
        .route("/activities/{id}/instructors/{instructor_id}", axum::routing::delete(handlers::activity::remove_instructor))
}
