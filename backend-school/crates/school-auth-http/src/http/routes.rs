use super::{handlers, session_handlers};
use crate::http::state::AuthHttpState;
use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, post},
    Router,
};

pub const AUTH_JSON_BODY_LIMIT: usize = 16 * 1024;

pub fn public_routes() -> Router<AuthHttpState> {
    Router::new()
        .route(
            "/auth/login",
            post(session_handlers::login).layer(DefaultBodyLimit::max(AUTH_JSON_BODY_LIMIT)),
        )
        .route("/auth/logout", post(session_handlers::logout))
}

pub fn protected_routes() -> Router<AuthHttpState> {
    Router::new()
        .route("/auth/me", get(session_handlers::me))
        .route(
            "/auth/me/profile",
            get(handlers::get_profile).put(handlers::update_profile),
        )
        .route(
            "/auth/me/change-password",
            post(session_handlers::change_password)
                .layer(DefaultBodyLimit::max(AUTH_JSON_BODY_LIMIT)),
        )
        .route("/auth/sessions", get(session_handlers::list_sessions))
        .route(
            "/auth/sessions/{id}",
            delete(session_handlers::revoke_session),
        )
        .route("/auth/logout-all", post(session_handlers::logout_all))
}
