use crate::{handlers, AppState};
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
        .nest(
            "/api/v1/auth",
            Router::new()
                .route("/login", post(crate::portal::http::login))
                .route("/register", post(crate::portal::http::register))
                .route("/logout", post(crate::portal::http::logout))
                .route("/me", get(crate::portal::http::me))
                .route("/resend-verification", post(crate::portal::http::resend))
                .route("/forgot-password", post(crate::portal::http::forgot))
                .route("/verify-email", post(crate::portal::http::verify))
                .route("/reset-password", post(crate::portal::http::reset)),
        )
        .nest(
            "/api/v1/portal",
            Router::new()
                .route("/schools", get(crate::portal::http::schools))
                .route(
                    "/requests",
                    get(crate::portal::http::requests).post(crate::portal::http::create_request),
                )
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    crate::portal::http::require_session,
                )),
        )
        .nest(
            "/api/v1/operator",
            Router::new()
                .route("/migration/status", get(crate::portal::migration::status))
                .route(
                    "/migration/migrate-all",
                    post(crate::portal::migration::migrate_all),
                )
                .route("/requests/{id}/decision", post(crate::portal::http::decide))
                .route("/schools/{id}/members", post(crate::portal::http::assign))
                .layer(axum::middleware::from_fn(
                    crate::portal::http::require_operator,
                ))
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    crate::portal::http::require_session,
                )),
        )
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
                // SSE endpoints for real-time logs
                .route("/stream", post(handlers::school_sse::create_school_sse))
                // Deployment endpoints
                .route("/{id}/deploy", post(handlers::school::deploy_school))
                .route("/deploy/bulk", post(handlers::school::bulk_deploy_schools))
                .route(
                    "/{id}/deployments",
                    get(handlers::school::get_deployment_history),
                )
                .layer(axum::middleware::from_fn(
                    crate::portal::http::require_operator,
                ))
                .layer(axum::middleware::from_fn_with_state(
                    state.clone(),
                    crate::portal::http::require_session,
                )),
        )
        .layer(axum::extract::DefaultBodyLimit::max(16 * 1024))
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
    let mut response = next.run(request).await;
    if route.starts_with("/api/v1/") {
        response.headers_mut().insert(
            "cache-control",
            axum::http::HeaderValue::from_static("no-store"),
        );
    }
    if !matches!(route.as_str(), "/health" | "/ready") {
        tracing::info!(operation = "http_response_headers", route, method = %method,
            status = response.status().as_u16(), duration_ms = started.elapsed().as_secs_f64() * 1000.0);
    }
    response
}
