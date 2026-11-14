use crate::handlers;
use crate::state::AcademicTimetableHttpState;
use axum::routing::{delete, get, post, put};
use axum::Router;

#[inline(never)]
pub fn academic_routes() -> Router<AcademicTimetableHttpState> {
    Router::new()
        .route(
            "/timetable",
            get(handlers::timetable::get_timetable).post(handlers::timetable::create_timetable),
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
        )
}

#[inline(never)]
pub fn personal_routes() -> Router<AcademicTimetableHttpState> {
    Router::new().route(
        "/me/timetable",
        get(handlers::timetable_blocks::get_my_timetable),
    )
}

// The upgrade retains the shared session runtime and tenant-origin policy.
#[inline(never)]
pub fn websocket_routes() -> Router<crate::state::AcademicWebSocketState> {
    Router::new().route(
        "/timetable",
        get(crate::websockets::timetable_websocket_handler),
    )
}
