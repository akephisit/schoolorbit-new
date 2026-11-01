use super::{
    models::*,
    runtime::{FileTransferIdentity, FileTransferRuntime},
    staff_display_name,
};
use axum::{
    extract::{
        rejection::{JsonRejection, PathRejection},
        DefaultBodyLimit, Extension, Path, State,
    },
    http::{header::CACHE_CONTROL, HeaderName, HeaderValue},
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse, Response,
    },
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use futures::{Stream, StreamExt};
use school_auth::{
    events::{PermissionChangeEvent, SessionRevocationEvent},
    runtime::AuthRuntime,
    session_service::AuthenticatedSession,
};
#[cfg(feature = "api-docs")]
use school_http::{ApiErrorResponse, EmptyData};
use school_http::{ApiResponse, HttpError};
use std::{future::Future, sync::Arc, time::Duration};
use tokio::{
    sync::broadcast,
    time::{interval_at, Instant, MissedTickBehavior},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct FileTransferHttpState {
    pub auth_runtime: AuthRuntime,
    pub runtime: Arc<FileTransferRuntime>,
}

pub fn routes() -> Router<FileTransferHttpState> {
    Router::new()
        .route("/staff/file-transfer/devices", post(register))
        .route(
            "/staff/file-transfer/devices/{deviceId}",
            get(snapshot).delete(remove),
        )
        .route(
            "/staff/file-transfer/devices/{deviceId}/events",
            get(events),
        )
        .route("/staff/file-transfer/pairs", post(create_pair))
        .route("/staff/file-transfer/pairs/{pairId}/accept", post(accept))
        .route("/staff/file-transfer/pairs/{pairId}/close", post(close))
        .route("/staff/file-transfer/pairs/{pairId}/signals", post(signal))
        .layer(DefaultBodyLimit::max(64 * 1024))
        .layer(axum::middleware::from_fn(no_store))
}
async fn no_store(request: axum::extract::Request, next: axum::middleware::Next) -> Response {
    let mut response = next.run(request).await;
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    response
}

fn parse_json_payload<T>(payload: Result<Json<T>, JsonRejection>) -> Result<T, HttpError> {
    match payload {
        Ok(Json(payload)) => Ok(payload),
        Err(rejection) if rejection.status() == axum::http::StatusCode::PAYLOAD_TOO_LARGE => {
            Err(HttpError::PayloadTooLarge)
        }
        Err(_) => Err(HttpError::BadRequest("invalid_request_body".into())),
    }
}
fn parse_device_or_pair_path(path: Result<Path<Uuid>, PathRejection>) -> Result<Uuid, HttpError> {
    path.map(|Path(id)| id)
        .map_err(|_| HttpError::BadRequest("invalid_resource_id".into()))
}

fn identity(session: &AuthenticatedSession) -> FileTransferIdentity {
    FileTransferIdentity {
        tenant_id: session.tenant.tenant_id,
        user_id: session.user_id,
        session_id: session.session_id,
    }
}
async fn staff(session: &AuthenticatedSession) -> Result<String, HttpError> {
    if session.user_type != "staff" {
        return Err(HttpError::Forbidden("Active staff account required".into()));
    }
    Ok(staff_display_name(&session.tenant.pool, session.user_id).await?)
}

#[cfg_attr(feature = "api-docs", utoipa::path(post, path="/api/staff/file-transfer/devices", operation_id="registerFileTransferDevice", tag="file-transfer", request_body=RegisterFileTransferDevice, responses((status=413,description="Request exceeds the 64 KiB wire budget",body=ApiErrorResponse),(status=200,description="Ephemeral device registration and current school-online snapshot",body=ApiResponse<FileTransferSnapshot>),(status=400,description="Invalid label",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Active staff required",body=ApiErrorResponse),(status=429,description="Device limit reached",body=ApiErrorResponse))))]
pub async fn register(
    State(state): State<FileTransferHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    payload: Result<Json<RegisterFileTransferDevice>, JsonRejection>,
) -> Result<impl IntoResponse, HttpError> {
    let request = parse_json_payload(payload)?;
    let display_name = staff(&session).await?;
    Ok(Json(ApiResponse::ok(state.runtime.register(
        identity(&session),
        display_name,
        request.label,
        Utc::now(),
    )?)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(get, path="/api/staff/file-transfer/devices/{deviceId}", operation_id="getFileTransferSnapshot", tag="file-transfer", params(("deviceId"=Uuid,Path,description="Own session-bound device")), responses((status=400,description="Invalid resource identifier",body=ApiErrorResponse),(status=200,description="Authoritative own device, school-online peers and own pairs",body=ApiResponse<FileTransferSnapshot>),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Active staff required",body=ApiErrorResponse),(status=404,description="Device unavailable",body=ApiErrorResponse))))]
pub async fn snapshot(
    State(state): State<FileTransferHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<impl IntoResponse, HttpError> {
    let id = parse_device_or_pair_path(path)?;
    staff(&session).await?;
    Ok(Json(ApiResponse::ok(state.runtime.snapshot(
        identity(&session),
        id,
        Utc::now(),
    )?)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(delete, path="/api/staff/file-transfer/devices/{deviceId}", operation_id="removeFileTransferDevice", tag="file-transfer", params(("deviceId"=Uuid,Path,description="Own session-bound device")), responses((status=400,description="Invalid resource identifier",body=ApiErrorResponse),(status=200,description="Device and active pairs removed",body=ApiResponse<EmptyData>),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Active staff required",body=ApiErrorResponse),(status=404,description="Device unavailable",body=ApiErrorResponse))))]
pub async fn remove(
    State(state): State<FileTransferHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<impl IntoResponse, HttpError> {
    let id = parse_device_or_pair_path(path)?;
    staff(&session).await?;
    state.runtime.remove(identity(&session), id, Utc::now())?;
    Ok(Json(ApiResponse::empty()))
}
#[cfg_attr(feature = "api-docs", utoipa::path(post, path="/api/staff/file-transfer/pairs", operation_id="createFileTransferPair", tag="file-transfer", request_body=CreateFileTransferPair, responses((status=413,description="Request exceeds the 64 KiB wire budget",body=ApiErrorResponse),(status=200,description="Pending pair with exact file metadata awaiting recipient consent",body=ApiResponse<FileTransferPair>),(status=400,description="Invalid file metadata",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Active staff required",body=ApiErrorResponse),(status=404,description="Target unavailable",body=ApiErrorResponse),(status=409,description="Device busy or offline",body=ApiErrorResponse),(status=429,description="Pairing code attempt limit reached",body=ApiErrorResponse))))]
pub async fn create_pair(
    State(state): State<FileTransferHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    payload: Result<Json<CreateFileTransferPair>, JsonRejection>,
) -> Result<impl IntoResponse, HttpError> {
    let request = parse_json_payload(payload)?;
    staff(&session).await?;
    Ok(Json(ApiResponse::ok(state.runtime.create_pair(
        identity(&session),
        request,
        Utc::now(),
    )?)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(post, path="/api/staff/file-transfer/pairs/{pairId}/accept", operation_id="acceptFileTransferPair", tag="file-transfer", params(("pairId"=Uuid,Path,description="Pending pair")), request_body=FileTransferDeviceRequest, responses((status=413,description="Request exceeds the 64 KiB wire budget",body=ApiErrorResponse),(status=400,description="Invalid request body or resource identifier",body=ApiErrorResponse),(status=200,description="Recipient accepted exact sender and file",body=ApiResponse<FileTransferPair>),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Active staff recipient required",body=ApiErrorResponse),(status=404,description="Pair unavailable",body=ApiErrorResponse),(status=409,description="Pair changed or disconnected",body=ApiErrorResponse))))]
pub async fn accept(
    State(state): State<FileTransferHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    path: Result<Path<Uuid>, PathRejection>,
    payload: Result<Json<FileTransferDeviceRequest>, JsonRejection>,
) -> Result<impl IntoResponse, HttpError> {
    let id = parse_device_or_pair_path(path)?;
    let request = parse_json_payload(payload)?;
    staff(&session).await?;
    Ok(Json(ApiResponse::ok(state.runtime.accept(
        identity(&session),
        request.device_id,
        id,
        Utc::now(),
    )?)))
}
#[cfg_attr(feature = "api-docs", utoipa::path(post, path="/api/staff/file-transfer/pairs/{pairId}/close", operation_id="closeFileTransferPair", tag="file-transfer", params(("pairId"=Uuid,Path,description="Pair to decline, cancel or finish")), request_body=FileTransferDeviceRequest, responses((status=413,description="Request exceeds the 64 KiB wire budget",body=ApiErrorResponse),(status=400,description="Invalid request body or resource identifier",body=ApiErrorResponse),(status=200,description="Pair closed",body=ApiResponse<EmptyData>),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Active staff required",body=ApiErrorResponse),(status=404,description="Pair unavailable",body=ApiErrorResponse))))]
pub async fn close(
    State(state): State<FileTransferHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    path: Result<Path<Uuid>, PathRejection>,
    payload: Result<Json<FileTransferDeviceRequest>, JsonRejection>,
) -> Result<impl IntoResponse, HttpError> {
    let id = parse_device_or_pair_path(path)?;
    let request = parse_json_payload(payload)?;
    staff(&session).await?;
    state
        .runtime
        .close(identity(&session), request.device_id, id, Utc::now())?;
    Ok(Json(ApiResponse::empty()))
}
#[cfg_attr(feature = "api-docs", utoipa::path(post, path="/api/staff/file-transfer/pairs/{pairId}/signals", operation_id="sendFileTransferSignal", tag="file-transfer", params(("pairId"=Uuid,Path,description="Accepted pair")), request_body=SendFileTransferSignal, responses((status=413,description="Request exceeds the 64 KiB wire budget",body=ApiErrorResponse),(status=200,description="Bounded host-only offer or answer queued to consented peer",body=ApiResponse<EmptyData>),(status=400,description="Invalid or non-host SDP",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Active staff required",body=ApiErrorResponse),(status=404,description="Pair unavailable",body=ApiErrorResponse),(status=409,description="Consent, direction, signaling deadline or sequence refused",body=ApiErrorResponse))))]
pub async fn signal(
    State(state): State<FileTransferHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
    path: Result<Path<Uuid>, PathRejection>,
    payload: Result<Json<SendFileTransferSignal>, JsonRejection>,
) -> Result<impl IntoResponse, HttpError> {
    let id = parse_device_or_pair_path(path)?;
    let request = parse_json_payload(payload)?;
    staff(&session).await?;
    state
        .runtime
        .signal(identity(&session), id, request, Utc::now())?;
    Ok(Json(ApiResponse::empty()))
}

fn session_stream<F, Fut>(
    mut subscription: super::runtime::FileTransferSubscription,
    session: AuthenticatedSession,
    mut revoked: broadcast::Receiver<SessionRevocationEvent>,
    mut permissions: broadcast::Receiver<PermissionChangeEvent>,
    mut revalidate: F,
    period: Duration,
) -> impl Stream<Item = FileTransferEvent>
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = Result<bool, school_errors::AppError>> + Send + 'static,
{
    async_stream::stream! {
        let _guard = subscription.guard;
        let mut timer = interval_at(Instant::now() + period, period);
        timer.set_missed_tick_behavior(MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                biased;
                event = revoked.recv() => {
                    match event {
                        Ok(event) if !event.applies_to(&session.tenant.subdomain, session.user_id, session.session_id) => {},
                        Ok(_) => { _guard.disconnect(); yield FileTransferEvent::SessionInvalid(FileTransferEmpty {}); break; },
                        Err(_) => { _guard.disconnect(); yield FileTransferEvent::SessionUnavailable(FileTransferEmpty {}); break; },
                    }
                }
                result = subscription.cancelled.changed() => { let _ = result; break; }
                event = permissions.recv() => {
                    match event {
                        Ok(event) if !event.applies_to(&session.tenant.subdomain, session.user_id) => {},
                        Ok(_) => match revalidate().await {
                            Ok(true) => {},
                            Ok(false) => { _guard.disconnect(); yield FileTransferEvent::SessionInvalid(FileTransferEmpty {}); break; },
                            Err(_) => { _guard.disconnect(); yield FileTransferEvent::SessionUnavailable(FileTransferEmpty {}); break; },
                        },
                        Err(_) => { _guard.disconnect(); yield FileTransferEvent::SessionUnavailable(FileTransferEmpty {}); break; },
                    }
                }
                _ = timer.tick() => {
                    match revalidate().await {
                        Ok(true) => {},
                        Ok(false) => { _guard.disconnect(); yield FileTransferEvent::SessionInvalid(FileTransferEmpty {}); break; },
                        Err(_) => { _guard.disconnect(); yield FileTransferEvent::SessionUnavailable(FileTransferEmpty {}); break; },
                    }
                }
                result = subscription.presence.changed() => {
                    if result.is_err() { break; }
                    yield FileTransferEvent::PresenceChanged(FileTransferEmpty {});
                }
                event = subscription.receiver.recv() => {
                    match event { Some(event) => yield event, None => break }
                }
            }
        }
    }
}

#[cfg_attr(feature = "api-docs", utoipa::path(get, path="/api/staff/file-transfer/devices/{deviceId}/events", operation_id="streamFileTransferEvents", tag="file-transfer", params(("deviceId"=Uuid,Path,description="Own session-bound device")), responses((status=400,description="Invalid resource identifier",body=ApiErrorResponse),(status=200,description="SSE event name file_transfer; each data field is FileTransferEvent. No replay; disconnect cancels pairs.",body=FileTransferEvent,content_type="text/event-stream"),(status=401,description="Authentication required",body=ApiErrorResponse),(status=403,description="Active staff required",body=ApiErrorResponse),(status=404,description="Device unavailable",body=ApiErrorResponse),(status=409,description="Device already streaming",body=ApiErrorResponse))))]
pub async fn events(
    State(state): State<FileTransferHttpState>,
    Extension(mut session): Extension<AuthenticatedSession>,
    path: Result<Path<Uuid>, PathRejection>,
) -> Result<Response, HttpError> {
    let id = parse_device_or_pair_path(path)?;
    let revoked = state.auth_runtime.session_events.subscribe();
    let permissions = state.auth_runtime.permission_events.subscribe();
    staff(&session).await?;
    if !school_auth::session_service::revalidate(&session, Utc::now()).await? {
        return Err(HttpError::AuthError("Session expired".into()));
    }
    let bound_identity = identity(&session);
    let subscription = state.runtime.subscribe(bound_identity, id, Utc::now())?;
    session.tenant.pool.release_for_realtime();
    let check_session = session.clone();
    let stream = session_stream(
        subscription,
        session,
        revoked,
        permissions,
        move || {
            let session = check_session.clone();
            let state = state.clone();
            async move {
                if !school_auth_http::tenant::revalidate_realtime_session(
                    &state.auth_runtime,
                    &session,
                )
                .await?
                {
                    return Ok(false);
                }
                let tenant = school_auth_http::tenant::resolve_tenant_context_by_subdomain(
                    &state.auth_runtime,
                    &session.tenant.subdomain,
                )
                .await?;
                if tenant.tenant_id != bound_identity.tenant_id {
                    return Ok(false);
                }
                match staff_display_name(&tenant.pool, session.user_id).await {
                    Ok(_) => {}
                    Err(school_errors::AppError::Forbidden(_)) => return Ok(false),
                    Err(error) => return Err(error),
                }
                state.runtime.refresh(bound_identity, id, Utc::now())?;
                Ok(true)
            }
        },
        Duration::from_secs(30),
    );
    let stream = stream.map(|event| {
        Event::default()
            .event("file_transfer")
            .json_data(event)
            .map_err(|_| FileTransferEncodingError)
    });
    // Serialization is over closed, typed DTOs only. If encoding fails, terminate
    // through the stream error rather than returning partial or fabricated payloads.
    let mut response = Sse::new(stream)
        .keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
        .into_response();
    response.headers_mut().insert(
        HeaderName::from_static("x-accel-buffering"),
        HeaderValue::from_static("no"),
    );
    response
        .headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

#[derive(Debug)]
struct FileTransferEncodingError;
impl std::fmt::Display for FileTransferEncodingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("File transfer event encoding failed")
    }
}
impl std::error::Error for FileTransferEncodingError {}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (
        FileTransferHttpState,
        AuthenticatedSession,
        super::super::runtime::FileTransferSubscription,
    ) {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .connect_lazy("postgres://invalid:invalid@127.0.0.1:1/invalid")
            .unwrap();
        let tenant = school_tenancy::TenantContext {
            tenant_id: Uuid::new_v4(),
            subdomain: "test".into(),
            pool: pool.into(),
        };
        let (session_events, _) = broadcast::channel(2);
        let (permission_events, _) = broadcast::channel(2);
        let runtime = AuthRuntime {
            admin_client: Arc::new(school_tenancy::AdminClient::new(
                "http://127.0.0.1:9".into(),
                "fixture".into(),
                school_tenancy::AdminClientConfig::from_env().unwrap(),
            )),
            pool_manager: Arc::new(school_tenancy::PoolManager::new()),
            identity_cache: Arc::new(school_auth::session_cache::SessionCache::new()),
            permission_cache: Arc::new(school_authorization::PermissionCache::new()),
            config: Arc::new(school_auth::config::SessionConfig::for_tests(
                school_auth::session_crypto::SessionHmacKey::for_tests([1; 32]),
            )),
            session_events,
            permission_events,
        };
        let session = AuthenticatedSession::for_tests(
            runtime.identity_cache.clone(),
            tenant,
            Uuid::new_v4(),
            Uuid::new_v4(),
            "staff",
        );
        let state = FileTransferHttpState {
            auth_runtime: runtime,
            runtime: Arc::new(FileTransferRuntime::default()),
        };
        let snapshot = state
            .runtime
            .register(
                identity(&session),
                "Staff".into(),
                "Laptop".into(),
                Utc::now(),
            )
            .unwrap();
        let subscription = state
            .runtime
            .subscribe(identity(&session), snapshot.device.id, Utc::now())
            .unwrap();
        (state, session, subscription)
    }
    #[tokio::test]
    async fn file_transfer_revocation_prioritizes_control_and_removes_binding_before_yield() {
        let (state, session, subscription) = fixture();
        let id = subscription.guard.device_id;
        let revoked = state.auth_runtime.session_events.subscribe();
        let permissions = state.auth_runtime.permission_events.subscribe();
        state
            .auth_runtime
            .session_events
            .send(SessionRevocationEvent::session(
                "test",
                session.user_id,
                session.session_id,
            ))
            .unwrap();
        let stream = session_stream(
            subscription,
            session.clone(),
            revoked,
            permissions,
            || async { Ok(true) },
            Duration::from_secs(30),
        );
        futures::pin_mut!(stream);
        assert!(matches!(
            stream.next().await,
            Some(FileTransferEvent::SessionInvalid(_))
        ));
        assert!(state
            .runtime
            .snapshot(identity(&session), id, Utc::now())
            .is_err());
        assert!(stream.next().await.is_none());
    }
    #[tokio::test]
    async fn file_transfer_signal_loss_and_revalidation_failure_close_stream() {
        for lag in [true, false] {
            let (state, session, subscription) = fixture();
            let revoked = state.auth_runtime.session_events.subscribe();
            let permissions = state.auth_runtime.permission_events.subscribe();
            if lag {
                for _ in 0..3 {
                    state
                        .auth_runtime
                        .session_events
                        .send(SessionRevocationEvent::user("other", Uuid::new_v4(), None))
                        .unwrap();
                }
            }
            let stream = session_stream(
                subscription,
                session,
                revoked,
                permissions,
                || async {
                    Err(school_errors::AppError::ServiceUnavailable(
                        "fixture".into(),
                    ))
                },
                Duration::from_millis(1),
            );
            futures::pin_mut!(stream);
            if !lag {
                assert!(matches!(
                    stream.next().await,
                    Some(FileTransferEvent::Ready(_))
                ));
            }
            assert!(matches!(
                tokio::time::timeout(Duration::from_secs(1), stream.next())
                    .await
                    .unwrap(),
                Some(FileTransferEvent::SessionUnavailable(_))
            ));
            assert!(stream.next().await.is_none());
        }
    }
}
