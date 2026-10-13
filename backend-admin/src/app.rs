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
        .route("/ready", get(handlers::health::readiness_check))
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
        .route(
            "/internal/schools/{subdomain}/migration-connection",
            get(handlers::internal::get_migration_connection_internal),
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
        .layer(axum::middleware::from_fn(request_timing))
        .with_state(state)
}

async fn request_timing(
    request: axum::extract::Request,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let route = request
        .extensions()
        .get::<axum::extract::MatchedPath>()
        .map(|path| path.as_str().to_owned())
        .unwrap_or_else(|| "unmatched".into());
    let method = request.method().clone();
    let started = std::time::Instant::now();
    let response = next.run(request).await;
    if !matches!(route.as_str(), "/health" | "/ready") {
        tracing::info!(operation = "http_response_headers", route, method = %method,
            status = response.status().as_u16(), duration_ms = started.elapsed().as_secs_f64() * 1000.0);
    }
    response
}
