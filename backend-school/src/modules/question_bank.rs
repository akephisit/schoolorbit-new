pub mod handlers;
pub mod models;
pub mod services;

use crate::AppState;
use axum::routing::get;
use axum::Router;

pub fn question_bank_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/questions",
            get(handlers::list_questions).post(handlers::create_question),
        )
        .route(
            "/questions/{id}",
            get(handlers::get_question)
                .put(handlers::update_question)
                .delete(handlers::delete_question),
        )
}
