use crate::{handlers, middleware, AppState};
use axum::{
    routing::{get, post},
    Router,
};
use tower_cookies::CookieManagerLayer;

pub fn build_app(state: AppState) -> Router {
    Router::new()
        // Public routes
        .route(
            "/",
            get(|| async {
                serde_json::json!({
                    "service": "SchoolOrbit Backend Admin",
                    "version": "0.1.0",
                    "status": "running"
                })
                .to_string()
            }),
        )
        .route("/health", get(handlers::health::health_check))
        .route("/api/v1/auth/login", post(handlers::auth::login_handler))
        .route("/api/v1/auth/logout", post(handlers::auth::logout_handler))
        .route("/api/v1/auth/me", get(handlers::auth::me_handler))
        // Internal routes (protected by INTERNAL_API_SECRET header)
        .route(
            "/internal/schools",
            get(handlers::internal::list_schools_internal),
        )
        .route(
            "/internal/schools/{subdomain}",
            get(handlers::internal::get_school_by_subdomain_internal),
        )
        .route(
            "/internal/schools/{subdomain}/migration-status",
            axum::routing::put(handlers::internal::update_migration_status_internal),
        )
        // Protected routes (require authentication)
        .nest(
            "/api/v1/schools",
            Router::new()
                .route("/", post(handlers::school::create_school))
                .route("/", get(handlers::school::list_schools))
                .route("/{id}", get(handlers::school::get_school))
                .route("/{id}", axum::routing::put(handlers::school::update_school))
                .route(
                    "/{id}",
                    axum::routing::delete(handlers::school::delete_school),
                )
                // SSE endpoints for real-time logs
                .route("/stream", post(handlers::school_sse::create_school_sse))
                .route(
                    "/{id}/stream",
                    axum::routing::delete(handlers::school_sse::delete_school_sse),
                )
                // Deployment endpoints
                .route("/{id}/deploy", post(handlers::school::deploy_school))
                .route("/deploy/bulk", post(handlers::school::bulk_deploy_schools))
                .route(
                    "/{id}/deployments",
                    get(handlers::school::get_deployment_history),
                )
                .layer(axum::middleware::from_fn(middleware::auth::require_auth)),
        )
        // Global layers
        .layer(CookieManagerLayer::new())
        .with_state(state)
}
