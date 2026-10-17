use crate::middleware::session::session_middleware;
use crate::{middleware, system, AppState};
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
        .route("/health", get(system::handlers::health::health_check))
        .route("/ready", get(system::handlers::health::readiness_check))
        .nest(
            "/api/admission",
            school_admission::http::routes::admission_public_routes().with_state(
                school_admission::http::state::AdmissionHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api",
            school_calendar::http::routes::public_routes().with_state(
                school_calendar::http::state::CalendarHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api",
            school_academic_http::routes::public_routes()
                .with_state(AcademicHttpState::from_ref(state)),
        )
        .nest(
            "/api",
            school_auth_http::http::routes::public_routes().with_state(
                school_auth_http::http::state::AuthHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api",
            school_auth_http::http::consent::routes::public_routes().with_state(
                school_auth_http::http::state::AuthHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api",
            school_files_http::http::routes::public_routes().with_state(
                school_files_http::http::state::FileHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api",
            school_administration::settings::http::routes::public_routes().with_state(
                school_administration::http::state::AdministrationHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api",
            school_navigation::http::routes::public_system_routes()
                .with_state(school_navigation::http::state::NavigationHttpState::from_ref(state)),
        )
        .nest(
            "/api",
            school_certificates_http::routes::public_routes()
                .with_state(CertificateHttpState::from_ref(state)),
        )
        .nest(
            "/ws",
            school_academic_http::routes::websocket_routes()
                .with_state(school_academic_http::state::AcademicWebSocketState::from_ref(state)),
        )
        .merge(internal_routes())
}

fn internal_routes() -> Router<AppState> {
    Router::new()
        .route(
            "/internal/tenant-directory/{subdomain}/begin",
            post(system::handlers::tenant_directory::begin),
        )
        .route(
            "/internal/tenant-directory/{subdomain}/finish",
            post(system::handlers::tenant_directory::finish),
        )
        .route(
            "/internal/provision",
            post(system::handlers::provision::provision_tenant),
        )
        .route(
            "/internal/migrate-all",
            post(system::handlers::migration::migrate_all_schools),
        )
        .route(
            "/internal/migration-status",
            get(system::handlers::migration::migration_status),
        )
        .route_layer(from_fn(middleware::internal_auth::validate_internal_secret))
}

fn protected_routes(state: &AppState) -> Router<AppState> {
    Router::new()
        .nest(
            "/api",
            school_auth_http::http::routes::protected_routes().with_state(
                school_auth_http::http::state::AuthHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api",
            school_staff::http::routes::protected_routes()
                .with_state(school_staff::http::state::StaffHttpState::from_ref(state)),
        )
        .nest(
            "/api",
            school_staff::achievement::http::routes::protected_routes()
                .with_state(school_staff::http::state::StaffHttpState::from_ref(state)),
        )
        .nest(
            "/api",
            school_fonts::http::routes::protected_routes()
                .with_state(school_fonts::http::state::FontHttpState::from_ref(state)),
        )
        .nest(
            "/api",
            school_certificates_http::routes::protected_routes()
                .with_state(CertificateHttpState::from_ref(state)),
        )
        .nest(
            "/api",
            school_students::http::routes::protected_routes().with_state(
                school_students::http::state::StudentHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api",
            school_students::parents::http::routes::protected_routes().with_state(
                school_students::http::state::StudentHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api",
            school_academic_http::routes::personal_routes()
                .with_state(AcademicHttpState::from_ref(state)),
        )
        .nest(
            "/api",
            school_calendar::http::routes::protected_routes().with_state(
                school_calendar::http::state::CalendarHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api",
            school_navigation::http::routes::protected_routes()
                .with_state(school_navigation::http::state::NavigationHttpState::from_ref(state)),
        )
        .nest(
            "/api",
            school_navigation::http::routes::protected_system_routes()
                .with_state(school_navigation::http::state::NavigationHttpState::from_ref(state)),
        )
        .nest(
            "/api",
            school_auth_http::http::consent::routes::protected_routes().with_state(
                school_auth_http::http::state::AuthHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api",
            school_files_http::http::routes::protected_routes().with_state(
                school_files_http::http::state::FileHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api",
            school_administration::settings::http::routes::protected_routes().with_state(
                school_administration::http::state::AdministrationHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api",
            school_notifications::http::routes::protected_routes().with_state(
                school_notifications::http::state::NotificationHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api",
            school_administration::lookup::http::routes::protected_routes().with_state(
                school_administration::http::state::AdministrationHttpState::from_ref(state),
            ),
        )
        .merge(
            school_attendance::http::routes::protected_routes()
                .with_state(school_attendance::http::state::AttendanceHttpState::from_ref(state)),
        )
        .merge(
            school_behavior::http::routes::protected_routes().with_state(
                school_behavior::http::state::BehaviorHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api/supply-catalog",
            school_administration::supply_catalog::http::routes::supply_catalog_routes()
                .with_state(
                    school_administration::http::state::AdministrationHttpState::from_ref(state),
                ),
        )
        .nest(
            "/api/calendar",
            school_calendar::http::routes::management_routes().with_state(
                school_calendar::http::state::CalendarHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api/supervision",
            school_supervision::http::routes::protected_routes()
                .with_state(school_supervision::http::state::SupervisionHttpState::from_ref(state)),
        )
        .nest(
            "/api/facilities",
            school_administration::facility::http::routes::facility_routes().with_state(
                school_administration::http::state::AdministrationHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api",
            school_workflow::http::routes::workflow_routes().with_state(
                school_workflow::http::state::WorkflowHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api",
            school_workflow::http::routes::work_routes().with_state(
                school_workflow::http::state::WorkflowHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api/admission",
            school_admission::http::routes::admission_staff_routes().with_state(
                school_admission::http::state::AdmissionHttpState::from_ref(state),
            ),
        )
        .nest(
            "/api/academic",
            school_academic_http::routes::academic_routes()
                .with_state(AcademicHttpState::from_ref(state))
                .nest(
                    "/question-bank",
                    school_question_bank::http::routes::question_bank_routes().with_state(
                        school_question_bank::http::state::QuestionBankHttpState::from_ref(state),
                    ),
                ),
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
