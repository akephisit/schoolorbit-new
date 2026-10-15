use crate::middleware::session::session_middleware;
use crate::{middleware, modules, AppState};
use axum::{
    extract::{DefaultBodyLimit, FromRef},
    middleware::{from_fn, from_fn_with_state},
    routing::{get, post},
    Json, Router,
};
use school_academic_http::state::AcademicHttpState;
use school_certificates_http::state::CertificateHttpState;
use serde_json::json;

pub const APPLICATION_BODY_LIMIT: usize = 20 * 1024 * 1024;

pub fn build_app(state: AppState) -> Router {
    let runtime = state.auth_runtime.clone();
    let protected = protected_routes(&state);
    public_routes(&state)
        .merge(protected.route_layer(from_fn_with_state(runtime, session_middleware)))
        .layer(DefaultBodyLimit::max(APPLICATION_BODY_LIMIT))
        .layer(axum::middleware::from_fn(request_timing))
        .with_state(state)
}

fn public_routes(state: &AppState) -> Router<AppState> {
    Router::new()
        .route("/", get(root_handler))
        .route(
            "/health",
            get(modules::system::handlers::health::health_check),
        )
        .route(
            "/ready",
            get(modules::system::handlers::health::readiness_check),
        )
        .nest(
            "/api/admission",
            modules::admission::admission_public_routes(),
        )
        .nest("/api", modules::calendar::routes::public_routes())
        .nest(
            "/api",
            school_academic_http::routes::public_routes()
                .with_state(AcademicHttpState::from_ref(state)),
        )
        .nest("/api", modules::auth::routes::public_routes())
        .nest("/api", modules::consent::routes::public_routes())
        .nest("/api", modules::files::routes::public_routes())
        .nest("/api", modules::school::routes::public_routes())
        .nest("/api", modules::system::routes::public_routes())
        .nest(
            "/api",
            school_certificates_http::routes::public_routes()
                .with_state(CertificateHttpState::from_ref(state)),
        )
        .nest("/ws", modules::academic::routes::websocket_routes())
        .merge(internal_routes())
}

fn internal_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/internal/provision",
            post(modules::system::handlers::provision::provision_tenant),
        )
        .route(
            "/internal/migrate-all",
            post(modules::system::handlers::migration::migrate_all_schools),
        )
        .route(
            "/internal/migration-status",
            get(modules::system::handlers::migration::migration_status),
        )
        .route_layer(from_fn(middleware::internal_auth::validate_internal_secret))
}

fn protected_routes(state: &AppState) -> Router<AppState> {
    Router::new()
        .nest("/api", modules::auth::routes::protected_routes())
        .nest("/api", modules::staff::routes::protected_routes())
        .nest("/api", modules::achievement::routes::protected_routes())
        .nest("/api", modules::school_fonts::routes::protected_routes())
        .nest(
            "/api",
            school_certificates_http::routes::protected_routes()
                .with_state(CertificateHttpState::from_ref(state)),
        )
        .nest("/api", modules::students::routes::protected_routes())
        .nest("/api", modules::parents::routes::protected_routes())
        .nest(
            "/api",
            school_academic_http::routes::personal_routes()
                .with_state(AcademicHttpState::from_ref(state)),
        )
        .nest("/api", modules::calendar::routes::protected_routes())
        .nest("/api", modules::menu::routes::protected_routes())
        .nest("/api", modules::system::routes::protected_routes())
        .nest("/api", modules::consent::routes::protected_routes())
        .nest("/api", modules::files::routes::protected_routes())
        .nest("/api", modules::school::routes::protected_routes())
        .nest("/api", modules::notification::routes::protected_routes())
        .nest("/api", modules::lookup::routes::protected_routes())
        .merge(modules::attendance::handlers::routes())
        .merge(modules::behavior::behavior_routes())
        .nest(
            "/api/supply-catalog",
            modules::supply_catalog::supply_catalog_routes(),
        )
        .nest("/api/calendar", modules::calendar::calendar_routes())
        .nest(
            "/api/supervision",
            modules::supervision::supervision_routes(),
        )
        .nest("/api/facilities", modules::facility::facility_routes())
        .nest("/api", modules::workflow::workflow_routes())
        .nest("/api", modules::work::work_routes())
        .nest(
            "/api/admission",
            modules::admission::admission_staff_routes(),
        )
        .nest(
            "/api/academic",
            modules::academic::academic_routes(AcademicHttpState::from_ref(state)),
        )
}

async fn root_handler() -> Json<serde_json::Value> {
    Json(json!({
        "service": "SchoolOrbit Backend School",
        "version": "0.2.0",
        "status": "running",
        "architecture": "multi-tenant with dynamic connection pools"
    }))
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
