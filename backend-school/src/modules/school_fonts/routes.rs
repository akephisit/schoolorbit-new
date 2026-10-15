use super::handlers;
use crate::AppState;
use axum::{
    routing::{delete, get, post},
    Router,
};

pub fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/school-fonts", get(handlers::list_school_fonts))
        .route(
            "/school-fonts/inspect",
            post(handlers::inspect_school_font_uploads),
        )
        .route(
            "/school-fonts/batch",
            post(handlers::attach_school_font_batch),
        )
        .route(
            "/school-fonts/{font_id}",
            delete(handlers::delete_school_font),
        )
}
