pub mod core;
#[cfg(test)]
pub mod cutover_test_preflight;
#[cfg(test)]
mod cutover_test_preflight_database_tests;
#[cfg(test)]
pub mod cutover_test_support;
pub mod delivery;
pub mod handlers;
pub mod models;
pub mod reconciliation;
pub mod services;
pub mod websockets;

use crate::AppState;
use axum::routing::{delete, get, patch, post, put};
use axum::Router;

pub fn academic_routes() -> Router<AppState> {
    core::routes().merge(delivery::routes()).merge(
        Router::new()
            // Assessment Plans (โครงสร้างคะแนนรายวิชา)
            .route(
                "/assessments/plans",
                get(handlers::assessment::list_assessment_plans),
            )
            .route(
                "/assessments/phase-controls",
                get(handlers::assessment::list_assessment_phase_controls),
            )
            .route(
                "/assessments/phase-controls/{control_id}",
                put(handlers::assessment::update_assessment_phase_control),
            )
            .route(
                "/assessments/offerings/{offering_id}",
                get(handlers::assessment::get_assessment_plan)
                    .put(handlers::assessment::save_assessment_plan),
            )
            // Question Bank
            .nest(
                "/question-bank",
                crate::modules::question_bank::question_bank_routes(),
            )
            // Exam Schedules
            .route(
                "/exam-schedules",
                get(handlers::exam_schedule::list_rounds)
                    .post(handlers::exam_schedule::create_round),
            )
            .route(
                "/exam-schedules/{round_id}",
                get(handlers::exam_schedule::get_workspace)
                    .patch(handlers::exam_schedule::update_round)
                    .delete(handlers::exam_schedule::delete_round),
            )
            .route(
                "/exam-schedules/{round_id}/source-preview",
                get(handlers::exam_schedule::preview_sources),
            )
            .route(
                "/exam-schedules/{round_id}/source-sync",
                post(handlers::exam_schedule::sync_sources),
            )
            .route(
                "/exam-schedules/{round_id}/days",
                post(handlers::exam_schedule::upsert_day),
            )
            .route(
                "/exam-schedules/days/{exam_day_id}",
                patch(handlers::exam_schedule::update_day)
                    .delete(handlers::exam_schedule::delete_day),
            )
            .route(
                "/exam-schedules/days/{exam_day_id}/room-assignments",
                get(handlers::exam_schedule::list_day_room_assignments)
                    .post(handlers::exam_schedule::upsert_day_room_assignment),
            )
            .route(
                "/exam-schedules/room-assignments/{assignment_id}/seats",
                post(handlers::exam_schedule::generate_seats),
            )
            .route(
                "/exam-schedules/sessions",
                post(handlers::exam_schedule::place_session),
            )
            .route(
                "/exam-schedules/sessions/{session_id}",
                delete(handlers::exam_schedule::delete_session),
            )
            .route(
                "/exam-schedules/{round_id}/invigilators",
                get(handlers::exam_schedule::get_invigilator_workspace),
            )
            .route(
                "/exam-schedules/{round_id}/invigilator-staff-options",
                get(handlers::exam_schedule::get_invigilator_staff_options),
            )
            .route(
                "/exam-schedules/room-assignments/{assignment_id}/invigilators/{staff_id}",
                put(handlers::exam_schedule::assign_assignment_invigilator)
                    .delete(handlers::exam_schedule::remove_assignment_invigilator),
            )
            .route(
                "/exam-schedules/room-assignments/{assignment_id}/invigilators",
                put(handlers::exam_schedule::update_assignment_invigilators),
            )
            .route(
                "/exam-schedules/{round_id}/publish",
                post(handlers::exam_schedule::publish_round),
            )
            // Timetable versions: register literal paths before timetable entry IDs.
            .route(
                "/timetable-versions/resolve",
                get(handlers::timetable_versions::resolve_version),
            )
            .route(
                "/timetable-versions",
                get(handlers::timetable_versions::list_versions),
            )
            .route(
                "/timetable-versions/{source_id}/clone",
                post(handlers::timetable_versions::clone_version),
            )
            // Canonical timetable blocks. Literal action paths must precede block IDs.
            .route(
                "/timetable-blocks/workspace",
                get(handlers::timetable_blocks::get_workspace),
            )
            .route(
                "/timetable-blocks/placement-preview",
                post(handlers::timetable_blocks::preview_placement),
            )
            .route(
                "/timetable-blocks/ordinary",
                post(handlers::timetable_blocks::create_ordinary),
            )
            .route(
                "/timetable-blocks/synchronized",
                post(handlers::timetable_blocks::create_synchronized),
            )
            .route(
                "/timetable-blocks/structural",
                post(handlers::timetable_blocks::create_structural),
            )
            .route(
                "/timetable-blocks/swap",
                post(handlers::timetable_blocks::swap_blocks),
            )
            .route(
                "/timetable-blocks/series/{series_id}",
                delete(handlers::timetable_blocks::delete_series),
            )
            .route(
                "/timetable-blocks/{block_id}/targets",
                delete(handlers::timetable_blocks::remove_target),
            )
            .route(
                "/timetable-blocks/{block_id}/sync",
                post(handlers::timetable_blocks::retry_sync),
            )
            .route(
                "/timetable-blocks/{block_id}/restore",
                post(handlers::timetable_blocks::restore_group),
            )
            .route(
                "/timetable-blocks/{block_id}",
                put(handlers::timetable_blocks::update_block)
                    .delete(handlers::timetable_blocks::delete_block),
            )
            .route(
                "/timetable/daily-teaching",
                get(handlers::timetable_blocks::daily_teaching_overview),
            )
            // Phase F: Timetable Templates
            // from-current + clear ต้อง register ก่อน /{id} กัน Axum match path เป็น id
            .route(
                "/timetable-templates/from-current",
                post(handlers::timetable_templates::from_current),
            )
            .route(
                "/timetable-blocks/clear",
                axum::routing::delete(handlers::timetable_templates::clear_timetable),
            )
            .route(
                "/timetable-templates",
                get(handlers::timetable_templates::list_templates)
                    .post(handlers::timetable_templates::create_template),
            )
            .route(
                "/timetable-templates/{id}",
                get(handlers::timetable_templates::get_template)
                    .put(handlers::timetable_templates::update_template)
                    .delete(handlers::timetable_templates::delete_template),
            )
            .route(
                "/timetable-templates/{id}/apply",
                post(handlers::timetable_templates::apply_template),
            ),
    )
}
