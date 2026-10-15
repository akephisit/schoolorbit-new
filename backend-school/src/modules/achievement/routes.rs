use super::handlers;
use crate::AppState;
use axum::{
    routing::{get, put},
    Router,
};

pub fn protected_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/achievements",
            get(handlers::list_achievements).post(handlers::create_achievement),
        )
        .route(
            "/achievements/{id}",
            put(handlers::update_achievement).delete(handlers::delete_achievement),
        )
}
