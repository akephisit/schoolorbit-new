use super::handlers;
use crate::AppState;
use axum::{
    extract::DefaultBodyLimit,
    routing::{get, post},
    Router,
};

pub fn public_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/public/files/{id}/content",
            get(handlers::get_public_file_content),
        )
        .route(
            "/public/files/{id}/delivery",
            get(handlers::get_public_file_delivery),
        )
}

pub fn protected_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/files",
            post(handlers::upload_file).layer(DefaultBodyLimit::max(24 * 1024 * 1024)),
        )
        .route(
            "/files/{id}",
            get(handlers::get_file_metadata).delete(handlers::delete_file),
        )
        .route("/files/{id}/download", post(handlers::download_file))
}
