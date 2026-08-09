use axum::{
    extract::{Request, State},
    http::{HeaderName, Method},
    middleware::Next,
    response::{IntoResponse, Response},
};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    error::AppError,
    modules::auth::{
        audit::{self, SessionFailureReason},
        config::CSRF_HEADER_NAME,
        http::{
            append_response_cookie, csrf_response_header, presented_session_token,
            set_session_cookie, validate_csrf,
        },
        runtime::AuthRuntime,
        session_crypto::RawSessionToken,
        session_repository::SessionMaintenanceMode,
        session_service,
    },
    utils::{subdomain::parse_realtime_tenant_hint, tenant::resolve_auth_tenant_context},
};

pub async fn session_middleware(
    State(runtime): State<AuthRuntime>,
    request: Request,
    next: Next,
) -> Response {
    match authenticate_request(&runtime, request, next).await {
        Ok(response) => response,
        Err(error) => error.into_response(),
    }
}

async fn authenticate_request(
    runtime: &AuthRuntime,
    mut request: Request,
    next: Next,
) -> Result<Response, AppError> {
    let method = request.method().clone();
    let path = request.uri().path().to_string();
    let realtime_hint = if method == Method::GET && path == "/api/notifications/stream" {
        parse_realtime_tenant_hint(request.uri().query()).map_err(audit_origin_rejection)?
    } else {
        None
    };
    let tenant = resolve_auth_tenant_context(runtime, request.headers(), realtime_hint.as_deref())
        .await
        .map_err(audit_origin_rejection)?;
    let token = presented_session_token(request.headers())?.ok_or_else(authentication_required)?;
    let context = runtime.service_context(tenant);
    let authentication = session_service::authenticate(
        &context,
        token.token_hash(),
        Utc::now(),
        maintenance_mode(&method, &path),
        RawSessionToken::generate,
    )
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

    let csrf_token = authentication.csrf_token;
    let replacement = authentication.replacement;
    request
        .extensions_mut()
        .insert(authentication.authenticated);
    let mut response = next.run(request).await;

    if let Some(replacement) = replacement {
        let encoded = replacement.encoded();
        append_response_cookie(
            &mut response,
            set_session_cookie(
                encoded.expose_for_cookie(),
                replacement.cookie_max_age_seconds,
            ),
        );
        insert_csrf_header(&mut response, &csrf_token);
    } else if method == Method::GET && path == "/api/auth/me" {
        insert_csrf_header(&mut response, &csrf_token);
    }

    Ok(response)
}

pub(crate) fn maintenance_mode(method: &Method, path: &str) -> SessionMaintenanceMode {
    let must_defer_rotation = (method == Method::GET && path == "/api/notifications/stream")
        || (method == Method::POST && path == "/api/auth/logout-all")
        || (method == Method::POST && path == "/api/auth/me/change-password")
        || is_session_revoke_route(method, path);
    if must_defer_rotation {
        SessionMaintenanceMode::TouchOnly
    } else {
        SessionMaintenanceMode::RotateAndTouch
    }
}

fn is_session_revoke_route(method: &Method, path: &str) -> bool {
    method == Method::DELETE
        && path
            .strip_prefix("/api/auth/sessions/")
            .and_then(|value| value.parse::<Uuid>().ok())
            .is_some()
}

fn is_unsafe_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    )
}

fn insert_csrf_header(
    response: &mut Response,
    token: &crate::modules::auth::session_crypto::CsrfToken,
) {
    response.headers_mut().insert(
        HeaderName::from_static(CSRF_HEADER_NAME),
        csrf_response_header(token),
    );
}

fn audit_origin_rejection(error: AppError) -> AppError {
    if matches!(error, AppError::Forbidden(_)) {
        audit::origin_rejected(SessionFailureReason::InvalidOrigin);
    }
    error
}

fn authentication_required() -> AppError {
    AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string())
}
