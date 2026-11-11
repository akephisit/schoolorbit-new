use crate::state::AcademicDeliveryHttpState;
use axum::Router;

#[inline(never)]
pub fn academic_routes() -> Router<AcademicDeliveryHttpState> {
    crate::delivery::routes()
}

#[inline(never)]
pub fn personal_routes() -> Router<AcademicDeliveryHttpState> {
    Router::new()
        .route(
            "/me/activity-registrations",
            axum::routing::get(crate::delivery::handlers::list_my_activity_registrations),
        )
        .route(
            "/me/activity-registrations/{group_id}",
            axum::routing::post(crate::delivery::handlers::enroll_my_activity_registration)
                .delete(crate::delivery::handlers::unenroll_my_activity_registration),
        )
}
