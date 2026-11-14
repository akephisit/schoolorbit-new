pub mod bulk_group_setup_handlers;
pub mod handlers;

use axum::routing::{get, post, put};
use axum::Router;

use crate::state::AcademicDeliveryHttpState;

#[inline(never)]
pub fn routes() -> Router<AcademicDeliveryHttpState> {
    Router::new()
        .route(
            "/offerings/{id}/deletion-preview",
            get(handlers::preview_offering_deletion),
        )
        .route(
            "/offerings/{id}/weekly-period-target",
            axum::routing::patch(handlers::update_weekly_period_target),
        )
        .route(
            "/learning-groups/{id}/deletion-preview",
            get(handlers::preview_group_deletion),
        )
        .route("/delivery/workspace", get(handlers::get_delivery_workspace))
        .route(
            "/delivery/homerooms",
            get(handlers::get_homeroom_delivery_workspace),
        )
        .route(
            "/delivery/management-options",
            get(handlers::get_delivery_management_options),
        )
        .route(
            "/offerings",
            get(handlers::list_offerings).post(handlers::create_offering),
        )
        .route(
            "/offerings/preview-from-curriculum",
            post(handlers::preview_offerings_from_curriculum),
        )
        .route(
            "/offerings/apply-from-curriculum",
            post(handlers::apply_offerings_from_curriculum),
        )
        .route(
            "/offerings/{id}",
            get(handlers::get_offering)
                .patch(handlers::update_offering)
                .delete(handlers::delete_offering),
        )
        .route(
            "/offerings/{id}/groups",
            get(handlers::list_groups).post(handlers::create_group),
        )
        .route(
            "/offerings/{id}/groups/bulk-setup/preview",
            post(bulk_group_setup_handlers::preview_bulk_group_setup),
        )
        .route(
            "/offerings/{id}/groups/bulk-setup",
            put(bulk_group_setup_handlers::apply_bulk_group_setup),
        )
        .route("/learning-groups", get(handlers::list_groups_for_term))
        .route(
            "/learning-groups/{id}",
            get(handlers::get_group)
                .patch(handlers::update_group)
                .delete(handlers::delete_group),
        )
        .route(
            "/learning-groups/{id}/homerooms",
            get(handlers::list_group_homerooms).put(handlers::replace_group_homerooms),
        )
        .route(
            "/learning-groups/{id}/teachers",
            get(handlers::list_group_teachers).put(handlers::replace_group_teachers),
        )
        .route(
            "/learning-groups/{id}/roster",
            get(handlers::preview_group_roster).put(handlers::apply_group_roster),
        )
        .route(
            "/learning-groups/{id}/roster-tracking",
            get(handlers::get_group_roster_tracking).put(handlers::update_group_roster_tracking),
        )
        .route(
            "/learning-groups/{id}/memberships",
            get(handlers::list_group_memberships).post(handlers::add_group_membership),
        )
        .route(
            "/learning-groups/{id}/memberships/{membershipId}/end",
            post(handlers::end_group_membership),
        )
}
