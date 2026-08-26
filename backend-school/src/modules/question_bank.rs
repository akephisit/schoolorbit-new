pub mod handlers;
pub mod models;
pub mod services;

#[cfg(test)]
mod services_tests;

use crate::AppState;
use axum::routing::{get, post};
use axum::Router;

pub fn question_bank_routes() -> Router<AppState> {
    Router::new()
        .route("/options", get(handlers::list_options))
        .route(
            "/questions",
            get(handlers::list_questions).post(handlers::create_question),
        )
        .route(
            "/questions/export-data",
            post(handlers::export_question_data),
        )
        .route(
            "/questions/{id}",
            get(handlers::get_question)
                .put(handlers::update_question)
                .delete(handlers::delete_question),
        )
        .route(
            "/questions/{question_id}/files/{file_id}",
            get(handlers::get_question_file),
        )
}
