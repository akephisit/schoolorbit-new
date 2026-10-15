use super::handlers;
use crate::AppState;
use axum::{
    routing::{get, post},
    Router,
};

pub fn public_routes() -> Router<AppState> {
    Router::new().route("/consent/types", get(handlers::get_consent_types))
}

pub fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/consent/my-status", get(handlers::get_my_consent_status))
        .route("/consent", post(handlers::create_consent))
        .route("/consent/{id}/withdraw", post(handlers::withdraw_consent))
        .route("/consent/summary", get(handlers::get_consent_summary))
}
