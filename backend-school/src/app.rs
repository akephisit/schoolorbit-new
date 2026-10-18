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
            school_student_care::http::routes::protected_routes().with_state(
                school_student_care::http::state::StudentCareHttpState::from_ref(state),
            ),
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
            school_workflow::http::routes::work_routes()
                .merge(school_workflow::http::process_handlers::routes())
                .with_state(school_workflow::http::state::WorkflowHttpState::from_ref(
                    state,
                )),
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
    let mut timing = RequestTiming {
        route,
        method: timing_method(request.method()),
        diagnostic_id: diagnostic_id(request.headers()),
        started: std::time::Instant::now(),
        completed: false,
    };
    let response = next.run(request).await;
    timing.completed = true;
    if timing.enabled() {
        tracing::info!(
            operation = "http_response_headers",
            route = timing.route,
            method = timing.method,
            diagnostic_id = timing.diagnostic_id.as_deref(),
            status = response.status().as_u16(),
            duration_ms = timing.duration_ms()
        );
    }
    response
}

// Retain only server-owned labels and a validated opaque diagnostic UUID across await.
// Dropping the request future before response headers must still leave timing evidence.
struct RequestTiming {
    route: String,
    method: &'static str,
    diagnostic_id: Option<String>,
    started: std::time::Instant,
    completed: bool,
}

impl RequestTiming {
    fn enabled(&self) -> bool {
        !matches!(self.route.as_str(), "/health" | "/ready")
    }

    fn duration_ms(&self) -> f64 {
        self.started.elapsed().as_secs_f64() * 1000.0
    }
}

impl Drop for RequestTiming {
    fn drop(&mut self) {
        if !self.completed && self.enabled() {
            tracing::info!(
                operation = "http_request_cancelled",
                route = self.route,
                method = self.method,
                diagnostic_id = self.diagnostic_id.as_deref(),
                duration_ms = self.duration_ms()
            );
        }
    }
}

fn timing_method(method: &axum::http::Method) -> &'static str {
    match method.as_str() {
        "GET" => "GET",
        "HEAD" => "HEAD",
        "POST" => "POST",
        "PUT" => "PUT",
        "PATCH" => "PATCH",
        "DELETE" => "DELETE",
        "OPTIONS" => "OPTIONS",
        "CONNECT" => "CONNECT",
        "TRACE" => "TRACE",
        _ => "OTHER",
    }
}

fn diagnostic_id(headers: &axum::http::HeaderMap) -> Option<String> {
    let mut values = headers.get_all("x-schoolorbit-diagnostic-id").iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    let uuid = uuid::Uuid::parse_str(value).ok()?;
    (uuid.get_version_num() == 4
        && uuid.get_variant() == uuid::Variant::RFC4122
        && uuid.hyphenated().to_string() == value)
        .then(|| value.to_owned())
}

#[cfg(test)]
mod request_timing_tests {
    use super::request_timing;
    use axum::{body::Body, http::Request, middleware::from_fn, routing::any, Router};
    use std::{
        io::Write,
        sync::{Arc, Mutex},
        time::Duration,
    };
    use tower::ServiceExt;

    #[derive(Clone, Default)]
    struct LogBuffer(Arc<Mutex<Vec<u8>>>);

    impl Write for LogBuffer {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl LogBuffer {
        fn events(&self) -> Vec<serde_json::Value> {
            String::from_utf8(self.0.lock().unwrap().clone())
                .unwrap()
                .lines()
                .map(|line| serde_json::from_str(line).unwrap())
                .collect()
        }
    }

    fn capture() -> (LogBuffer, tracing::subscriber::DefaultGuard) {
        let output = LogBuffer::default();
        let writer = output.clone();
        let subscriber = tracing_subscriber::fmt()
            .json()
            .without_time()
            .with_writer(move || writer.clone())
            .finish();
        (output, tracing::subscriber::set_default(subscriber))
    }

    const DIAGNOSTIC_ID: &str = "9ab961be-2edd-4502-b26c-1943cf272380";

    #[tokio::test]
    async fn completed_headers_emit_once_without_cancellation_or_private_request_values() {
        let (output, _guard) = capture();
        let app = Router::new()
            .route("/students/{student}", any(|| async { "ok" }))
            .layer(from_fn(request_timing));
        let response = app
            .oneshot(
                Request::builder()
                    .uri("/students/private-student?national_id=private-query")
                    .header("Cookie", "private-cookie")
                    .header(axum::http::header::AUTHORIZATION, "private-authorization")
                    .header("X-SchoolOrbit-Diagnostic-Id", DIAGNOSTIC_ID)
                    .body(Body::from("private-body"))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), 200);
        drop(response);
        let events = output.events();
        assert_eq!(events.len(), 1);
        let fields = &events[0]["fields"];
        assert_eq!(fields["operation"], "http_response_headers");
        assert_eq!(fields["route"], "/students/{student}");
        assert_eq!(fields["method"], "GET");
        assert_eq!(fields["diagnostic_id"], DIAGNOSTIC_ID);
        assert_eq!(fields["status"], 200);
        assert!(fields["duration_ms"].as_f64().unwrap() >= 0.0);
        assert!(!events[0].to_string().contains("private-"));
    }

    #[tokio::test]
    async fn interrupted_handler_emits_one_cancellation_with_numeric_elapsed_time() {
        let (output, _guard) = capture();
        let app = Router::new()
            .route(
                "/students/{student}",
                any(|| async {
                    std::future::pending::<()>().await;
                    "unreachable"
                }),
            )
            .layer(from_fn(request_timing));
        let request = Request::builder()
            .uri("/students/private-student?private-query")
            .header("X-SchoolOrbit-Diagnostic-Id", DIAGNOSTIC_ID)
            .body(Body::empty())
            .unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(20), app.oneshot(request))
                .await
                .is_err()
        );
        let events = output.events();
        assert_eq!(events.len(), 1);
        let fields = &events[0]["fields"];
        assert_eq!(fields["operation"], "http_request_cancelled");
        assert_eq!(fields["route"], "/students/{student}");
        assert_eq!(fields["diagnostic_id"], DIAGNOSTIC_ID);
        assert!(fields["duration_ms"].as_f64().unwrap() > 0.0);
        assert!(fields.get("status").is_none());
        assert!(!events[0].to_string().contains("private-"));
    }

    #[tokio::test]
    async fn invalid_or_missing_diagnostic_ids_and_custom_methods_are_never_logged_raw() {
        let (output, _guard) = capture();
        let app = Router::new().layer(from_fn(request_timing));
        let invalid = [
            None,
            Some("private-header"),
            Some("9AB961BE-2EDD-4502-B26C-1943CF272380"),
            Some("9ab961be2edd4502b26c1943cf272380"),
            Some("9ab961be-2edd-5502-b26c-1943cf272380"),
            Some("9ab961be-2edd-4502-726c-1943cf272380"),
        ];
        for value in invalid {
            let mut request = Request::builder()
                .method("private-method")
                .uri("/private-unmatched?private-query")
                .header("Cookie", "private-cookie");
            if let Some(value) = value {
                request = request.header("X-SchoolOrbit-Diagnostic-Id", value);
            }
            app.clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap();
        }
        let duplicate = Request::builder()
            .uri("/private-unmatched")
            .header("X-SchoolOrbit-Diagnostic-Id", DIAGNOSTIC_ID)
            .header("X-SchoolOrbit-Diagnostic-Id", DIAGNOSTIC_ID)
            .body(Body::empty())
            .unwrap();
        app.oneshot(duplicate).await.unwrap();
        let events = output.events();
        assert_eq!(events.len(), invalid.len() + 1);
        for event in &events {
            let fields = &event["fields"];
            assert_eq!(fields["route"], "unmatched");
            assert!(fields.get("diagnostic_id").is_none());
            assert!(!event.to_string().contains("private-"));
        }
        assert_eq!(events[0]["fields"]["method"], "OTHER");
    }
}
