use axum::{
    extract::{Request, State},
    http::{HeaderName, Method},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use school_http::HttpError as AppError;

use school_auth::{
    audit::{self, SessionFailureReason},
    config::CSRF_HEADER_NAME,
    runtime::AuthRuntime,
    session_service,
};
use school_auth_http::{
    cookies::{csrf_response_header, presented_session_token, validate_csrf},
    origin::parse_realtime_tenant_hint,
    tenant::resolve_auth_tenant_context,
};

pub async fn session_middleware(
    State(runtime): State<AuthRuntime>,
    request: Request,
    next: Next,
) -> Response {
    let session_response = request.uri().path().starts_with("/api/auth/");
    let mut response = match authenticate_request(&runtime, request, next).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    };
    if session_response {
        response.headers_mut().insert(
            axum::http::header::CACHE_CONTROL,
            axum::http::HeaderValue::from_static("no-store"),
        );
    }
    response
}

async fn authenticate_request(
    runtime: &AuthRuntime,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let realtime_hint = if method == Method::GET && realtime_hint_path(&path) {
        parse_realtime_tenant_hint(request.uri().query()).map_err(audit_origin_rejection)?
    } else {
        None
    };
    let tenant = resolve_auth_tenant_context(runtime, request.headers(), realtime_hint.as_deref())
        .await
        .map_err(audit_origin_rejection)?;
    let token = presented_session_token(request.headers())?.ok_or_else(authentication_required)?;
    let context = runtime.service_context(tenant);
    let authentication = session_service::authenticate(&context, token.token_hash(), Utc::now())
        .await?
        .ok_or_else(authentication_required)?;

    if is_unsafe_method(&method) {
        validate_csrf(request.headers(), &authentication.csrf_token).map_err(|error| {
            audit::csrf_rejected(
                context.tenant().tenant_id,
                SessionFailureReason::InvalidCsrf,
            );
            error
        })?;
    }

    // Activity checks expectedSessionId in its owning handler before any promotion/touch.
    let authenticated = if method == Method::POST && path == "/api/auth/activity" {
        authentication.authenticated
    } else {
        session_service::acknowledge(&context, &authentication.authenticated, Utc::now()).await?
    };
    let csrf_token = authentication.csrf_token;
    request.extensions_mut().insert(authenticated);
    let mut response = next.run(request).await;

    if method == Method::GET && path == "/api/auth/me" {
        insert_csrf_header(&mut response, &csrf_token);
    }

    Ok(response)
}

fn is_unsafe_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn insert_csrf_header(response: &mut Response, token: &school_auth::session_crypto::CsrfToken) {
    response.headers_mut().insert(
        HeaderName::from_static(CSRF_HEADER_NAME),
        csrf_response_header(token),
    );
}

fn audit_origin_rejection(error: school_errors::AppError) -> AppError {
    if matches!(error, school_errors::AppError::Forbidden(_)) {
        audit::origin_rejected(SessionFailureReason::InvalidOrigin);
    }
    error.into()
}

fn authentication_required() -> AppError {
    AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string())
}

fn realtime_hint_path(path: &str) -> bool {
    if path == "/api/notifications/stream" {
        return true;
    }
    path.strip_prefix("/api/staff/file-transfer/devices/")
        .and_then(|suffix| suffix.strip_suffix("/events"))
        .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
}

#[cfg(test)]
mod file_transfer_hint_tests {
    use super::realtime_hint_path;
    #[test]
    fn realtime_hint_is_limited_to_exact_stream_paths() {
        assert!(realtime_hint_path("/api/notifications/stream"));
        assert!(realtime_hint_path(
            "/api/staff/file-transfer/devices/11111111-1111-1111-1111-111111111111/events"
        ));
        for path in [
            "/api/staff/file-transfer/devices/not-a-uuid/events",
            "/api/staff/file-transfer/devices/11111111-1111-1111-1111-111111111111",
            "/api/staff/file-transfer/devices/11111111-1111-1111-1111-111111111111/events/other",
        ] {
            assert!(!realtime_hint_path(path));
        }
    }
}
