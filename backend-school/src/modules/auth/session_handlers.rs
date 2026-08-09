use std::net::SocketAddr;

use axum::{
    extract::{rejection::JsonRejection, ConnectInfo, Extension, Path, State},
    http::{HeaderMap, HeaderName, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use uuid::Uuid;

use crate::{
    api_response::{ApiResponse, EmptyData},
    error::AppError,
    utils::{
        client_address::client_address,
        tenant::{resolve_auth_tenant_context, TenantContext},
    },
};

use super::{
    audit::{self, SessionFailureReason},
    config::CSRF_HEADER_NAME,
    http::{
        append_expired_auth_cookies, append_response_cookie, csrf_response_header,
        expire_legacy_cookie, presented_session_token, set_session_cookie, validate_csrf,
    },
    models::{
        ChangePasswordRequest, CurrentUserResponse, LoginRequest, SessionListData,
        SessionLoginData, SessionResponse,
    },
    runtime::AuthRuntime,
    session_crypto::RawSessionToken,
    session_repository::SessionMaintenanceMode,
    session_service::{self, AuthenticatedSession, LoginCommand, LoginUserSnapshot},
};

#[utoipa::path(
    post,
    path = "/api/auth/login",
    operation_id = "loginWithSession",
    tag = "auth",
    request_body = LoginRequest,
    responses(
        (status = 200, description = "Authenticated session", body = ApiResponse<SessionLoginData>),
        (status = 400, description = "Malformed request", body = crate::api_response::ApiErrorResponse),
        (status = 401, description = "Invalid credentials", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Origin rejected", body = crate::api_response::ApiErrorResponse),
        (status = 429, description = "Login rate limited", body = crate::api_response::ApiErrorResponse),
        (status = 503, description = "Authentication service unavailable", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn login(
    State(runtime): State<AuthRuntime>,
    ConnectInfo(peer): ConnectInfo<SocketAddr>,
    headers: HeaderMap,
    payload_result: Result<Json<LoginRequest>, JsonRejection>,
) -> Result<Response, AppError> {
    let tenant = resolve_auth_tenant_context(&runtime, &headers, None)
        .await
        .map_err(audit_origin_rejection)?;
    let payload = parse_json_payload(payload_result)?;
    login_with_tenant(&runtime, tenant, peer, &headers, payload).await
}

pub(super) async fn login_with_tenant(
    runtime: &AuthRuntime,
    tenant: TenantContext,
    peer: SocketAddr,
    headers: &HeaderMap,
    payload: LoginRequest,
) -> Result<Response, AppError> {
    let source = client_address(peer, headers, &runtime.config.trusted_proxy_cidrs);
    let user_agent = single_user_agent(headers);
    let context = runtime.service_context(tenant);
    let result = session_service::login(
        &context,
        LoginCommand {
            username: &payload.username,
            password: &payload.password,
            remember_me: payload.remember_me.unwrap_or(false),
            source,
            user_agent,
            now: Utc::now(),
        },
        RawSessionToken::generate,
    )
    .await?;

    let data = SessionLoginData {
        user: current_user_response(result.user),
    };
    let encoded = result.credential.encoded();
    let mut response = (StatusCode::OK, Json(ApiResponse::ok(data))).into_response();
    append_response_cookie(
        &mut response,
        set_session_cookie(
            encoded.expose_for_cookie(),
            result.credential.cookie_max_age_seconds,
        ),
    );
    append_response_cookie(&mut response, expire_legacy_cookie());
    insert_csrf_header(&mut response, &result.csrf_token);
    Ok(response)
}

#[utoipa::path(
    post,
    path = "/api/auth/logout",
    operation_id = "logoutSession",
    tag = "auth",
    responses(
        (status = 200, description = "Session revoked or stale credentials cleared", body = ApiResponse<EmptyData>),
        (status = 401, description = "Ambiguous session credential", body = crate::api_response::ApiErrorResponse),
        (status = 403, description = "Origin or CSRF rejected", body = crate::api_response::ApiErrorResponse),
        (status = 503, description = "Session store unavailable", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn logout(
    State(runtime): State<AuthRuntime>,
    headers: HeaderMap,
) -> Result<Response, AppError> {
    let tenant = resolve_auth_tenant_context(&runtime, &headers, None)
        .await
        .map_err(audit_origin_rejection)?;
    logout_with_tenant(&runtime, tenant, &headers).await
}

pub(super) async fn logout_with_tenant(
    runtime: &AuthRuntime,
    tenant: TenantContext,
    headers: &HeaderMap,
) -> Result<Response, AppError> {
    let Some(token) = presented_session_token(headers)? else {
        return Ok(expired_auth_response("ออกจากระบบสำเร็จ"));
    };
    let context = runtime.service_context(tenant);
    let now = Utc::now();
    let Some(authentication) = session_service::authenticate(
        &context,
        token.token_hash(),
        now,
        SessionMaintenanceMode::TouchOnly,
        RawSessionToken::generate,
    )
    .await?
    else {
        return Ok(expired_auth_response("ออกจากระบบสำเร็จ"));
    };

    validate_csrf(headers, &authentication.csrf_token).map_err(|error| {
        audit::csrf_rejected(
            context.tenant().tenant_id,
            SessionFailureReason::InvalidCsrf,
        );
        error
    })?;
    session_service::logout(&context, &authentication.authenticated, now).await?;
    Ok(expired_auth_response("ออกจากระบบสำเร็จ"))
}

#[utoipa::path(
    get,
    path = "/api/auth/me",
    operation_id = "getSessionCurrentUser",
    tag = "auth",
    responses(
        (status = 200, description = "Minimal current user", body = ApiResponse<CurrentUserResponse>),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 503, description = "Identity or permission store unavailable", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn me(
    State(runtime): State<AuthRuntime>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Response, AppError> {
    let context = runtime.service_context(session.tenant.clone());
    let user = session_service::load_current_user(&context, &session).await?;
    Ok((
        StatusCode::OK,
        Json(ApiResponse::ok(current_user_response(user))),
    )
        .into_response())
}

#[utoipa::path(
    get,
    path = "/api/auth/sessions",
    operation_id = "listAuthSessions",
    tag = "auth",
    responses(
        (status = 200, description = "Active sessions", body = ApiResponse<SessionListData>),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 503, description = "Session store unavailable", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn list_sessions(
    State(_runtime): State<AuthRuntime>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Response, AppError> {
    let sessions = session_service::list_sessions(&session, Utc::now()).await?;
    let sessions = sessions
        .into_iter()
        .map(|row| SessionResponse {
            id: row.id,
            device_label: row.device_label,
            remember_me: row.remember_me,
            created_at: row.created_at,
            last_seen_at: row.last_seen_at,
            idle_expires_at: row.idle_expires_at,
            absolute_expires_at: row.absolute_expires_at,
            is_current: row.id == session.session_id,
        })
        .collect();
    Ok((
        StatusCode::OK,
        Json(ApiResponse::ok(SessionListData { sessions })),
    )
        .into_response())
}

#[utoipa::path(
    delete,
    path = "/api/auth/sessions/{id}",
    operation_id = "revokeAuthSession",
    tag = "auth",
    params(("id" = Uuid, Path, description = "Owned session identifier")),
    responses(
        (status = 200, description = "Owned session revoked", body = ApiResponse<EmptyData>),
        (status = 404, description = "Owned session not found", body = crate::api_response::ApiErrorResponse),
        (status = 503, description = "Session store unavailable", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn revoke_session(
    State(runtime): State<AuthRuntime>,
    Extension(session): Extension<AuthenticatedSession>,
    Path(selected_session_id): Path<Uuid>,
) -> Result<Response, AppError> {
    let context = runtime.service_context(session.tenant.clone());
    let result =
        session_service::revoke_selected(&context, &session, selected_session_id, Utc::now())
            .await?;
    let mut response = empty_response("เพิกถอนเซสชันสำเร็จ");
    if result.current_revoked {
        append_expired_auth_cookies(&mut response);
    }
    Ok(response)
}

#[utoipa::path(
    post,
    path = "/api/auth/logout-all",
    operation_id = "logoutAllSessions",
    tag = "auth",
    responses(
        (status = 200, description = "All owned sessions revoked", body = ApiResponse<EmptyData>),
        (status = 503, description = "Session store unavailable", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn logout_all(
    State(runtime): State<AuthRuntime>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Response, AppError> {
    let context = runtime.service_context(session.tenant.clone());
    session_service::logout_all(&context, &session, Utc::now()).await?;
    Ok(expired_auth_response("ออกจากระบบทุกอุปกรณ์สำเร็จ"))
}

#[utoipa::path(
    post,
    path = "/api/auth/me/change-password",
    operation_id = "changeSessionPassword",
    tag = "auth",
    request_body = ChangePasswordRequest,
    responses(
        (status = 200, description = "Password changed and current credential replaced", body = ApiResponse<EmptyData>),
        (status = 400, description = "Password validation failed", body = crate::api_response::ApiErrorResponse),
        (status = 401, description = "Authentication required", body = crate::api_response::ApiErrorResponse),
        (status = 409, description = "Concurrent password change", body = crate::api_response::ApiErrorResponse),
        (status = 503, description = "Session store unavailable", body = crate::api_response::ApiErrorResponse)
    )
)]
pub async fn change_password(
    State(runtime): State<AuthRuntime>,
    Extension(session): Extension<AuthenticatedSession>,
    payload_result: Result<Json<ChangePasswordRequest>, JsonRejection>,
) -> Result<Response, AppError> {
    let payload = parse_json_payload(payload_result)?;
    let context = runtime.service_context(session.tenant.clone());
    let result = session_service::change_password(
        &context,
        &session,
        &payload.current_password,
        &payload.new_password,
        Utc::now(),
        RawSessionToken::generate,
    )
    .await?;

    let encoded = result.credential.encoded();
    let mut response = empty_response("เปลี่ยนรหัสผ่านสำเร็จ");
    append_response_cookie(
        &mut response,
        set_session_cookie(
            encoded.expose_for_cookie(),
            result.credential.cookie_max_age_seconds,
        ),
    );
    insert_csrf_header(&mut response, &result.csrf_token);
    Ok(response)
}

fn parse_json_payload<T>(payload_result: Result<Json<T>, JsonRejection>) -> Result<T, AppError> {
    match payload_result {
        Ok(Json(payload)) => Ok(payload),
        Err(rejection) if rejection.status() == StatusCode::PAYLOAD_TOO_LARGE => {
            Err(AppError::PayloadTooLarge)
        }
        Err(_) => Err(AppError::BadRequest("invalid_request_body".to_string())),
    }
}

fn current_user_response(user: LoginUserSnapshot) -> CurrentUserResponse {
    CurrentUserResponse {
        id: user.id,
        username: user.username,
        first_name: user.first_name,
        last_name: user.last_name,
        user_type: user.user_type,
        status: user.status,
        primary_role_name: user.primary_role_name,
        profile_image_file_id: user.profile_image_file_id,
        permissions: user.permissions,
    }
}

fn empty_response(message: &str) -> Response {
    (
        StatusCode::OK,
        Json(ApiResponse::empty_with_message(message)),
    )
        .into_response()
}

fn expired_auth_response(message: &str) -> Response {
    let mut response = empty_response(message);
    append_expired_auth_cookies(&mut response);
    response
}

fn insert_csrf_header(response: &mut Response, token: &super::session_crypto::CsrfToken) {
    response.headers_mut().insert(
        HeaderName::from_static(CSRF_HEADER_NAME),
        csrf_response_header(token),
    );
}

fn single_user_agent(headers: &HeaderMap) -> Option<&str> {
    let mut values = headers.get_all("user-agent").iter();
    let value = values.next()?.to_str().ok()?;
    if values.next().is_some() {
        return None;
    }
    Some(value)
}

fn audit_origin_rejection(error: AppError) -> AppError {
    if matches!(error, AppError::Forbidden(_)) {
        audit::origin_rejected(SessionFailureReason::InvalidOrigin);
    }
    error
}
