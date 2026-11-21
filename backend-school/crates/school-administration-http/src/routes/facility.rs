use crate::handlers;
use crate::state::AdministrationHttpState;
use axum::{
    routing::{get, put},
    Router,
};

#[inline(never)]
pub fn facility_routes() -> Router<AdministrationHttpState> {
    Router::new()
        .route(
            "/buildings",
            get(handlers::facility::list_buildings).post(handlers::facility::create_building),
        )
        .route(
            "/buildings/{id}",
            put(handlers::facility::update_building).delete(handlers::facility::delete_building),
        )
        .route(
            "/rooms",
            get(handlers::facility::list_rooms).post(handlers::facility::create_room),
        )
        .route(
            "/rooms/{id}",
            put(handlers::facility::update_room).delete(handlers::facility::delete_room),
        )
}
