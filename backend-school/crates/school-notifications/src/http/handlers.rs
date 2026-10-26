use crate::events::TenantNotificationEvent;
use crate::events::WorkChangeEvent;
use crate::http::state::NotificationHttpState;
use crate::models::ListNotificationsQuery;
use crate::models::{RegisterPushDeviceRequest, UnregisterPushDeviceQuery};
use crate::services as notification_service;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{
    extract::{Extension, Path, Query, State},
    http::{HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use futures::{stream::Stream, StreamExt};
use school_auth::{
    audit::{self, SessionFailureReason},
    events::{PermissionChangeEvent, SessionRevocationEvent},
    session_service::{self, AuthenticatedSession},
};
use school_auth_http::context::current_user_tenant_context_from_session;
use school_http::ApiResponse;
use school_http::HttpError as AppError;
#[cfg(feature = "api-docs")]
use school_http::{ApiErrorResponse, EmptyData};
use std::{convert::Infallible, future::Future, time::Duration};
use tokio::{
    sync::broadcast,
    time::{interval_at, Instant, Interval, MissedTickBehavior},
};
use uuid::Uuid;

const SESSION_REVALIDATION_INTERVAL: Duration = Duration::from_secs(30);
const X_ACCEL_BUFFERING: HeaderName = HeaderName::from_static("x-accel-buffering");

#[derive(Debug, Eq, PartialEq)]
enum NotificationStreamEvent {
    Notification(String),
    PermissionChanged,
    ReconcileRequired,
    WorkChanged(&'static str),
    SessionInvalid,
    SessionUnavailable,
}

impl NotificationStreamEvent {
    fn into_sse_event(self) -> Event {
        match self {
            Self::Notification(data) => Event::default().data(data),
            Self::PermissionChanged => Event::default().event("permission_changed").data("{}"),
            Self::ReconcileRequired => Event::default().event("reconcile_required").data("{}"),
            Self::WorkChanged(event_name) => Event::default().event(event_name).data("{}"),
            Self::SessionInvalid => Event::default().event("session_invalid").data("{}"),
            Self::SessionUnavailable => Event::default().event("session_unavailable").data("{}"),
        }
    }
}

fn audit_session_stream_disconnect(session: &AuthenticatedSession, reason: SessionFailureReason) {
    audit::session_realtime_disconnect(
        session.tenant.tenant_id,
        session.user_id,
        session.session_id,
        reason,
    );
}

fn session_bound_notification_stream<F, Fut, E>(
    session: AuthenticatedSession,
    mut notification_rx: broadcast::Receiver<TenantNotificationEvent>,
    mut permission_rx: broadcast::Receiver<PermissionChangeEvent>,
    mut work_rx: broadcast::Receiver<WorkChangeEvent>,
    mut session_rx: broadcast::Receiver<SessionRevocationEvent>,
    mut revalidation_interval: Interval,
    mut revalidate: F,
) -> impl Stream<Item = NotificationStreamEvent>
where
    F: FnMut() -> Fut + Send + 'static,
    Fut: Future<Output = Result<bool, E>> + Send + 'static,
    E: Send + 'static,
{
    let tenant = session.tenant.subdomain.clone();
    let user_id = session.user_id;

    async_stream::stream! {
        loop {
            tokio::select! {
                biased;
                session_result = session_rx.recv() => {
                    match session_result {
                        Ok(event) if event.applies_to(&tenant, user_id, session.session_id) => {
                            audit_session_stream_disconnect(
                                &session,
                                SessionFailureReason::RealtimeSessionInvalid,
                            );
                            yield NotificationStreamEvent::SessionInvalid;
                            break;
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            audit_session_stream_disconnect(&session, SessionFailureReason::RealtimeSessionUnavailable);
                            yield NotificationStreamEvent::SessionUnavailable;
                            break;
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            audit_session_stream_disconnect(
                                &session,
                                SessionFailureReason::RealtimeSessionUnavailable,
                            );
                            yield NotificationStreamEvent::SessionUnavailable;
                            break;
                        }
                    }
                }
                _ = revalidation_interval.tick() => {
                    match revalidate().await {
                        Ok(true) => {}
                        Ok(false) => {
                            audit_session_stream_disconnect(
                                &session,
                                SessionFailureReason::RealtimeSessionInvalid,
                            );
                            yield NotificationStreamEvent::SessionInvalid;
                            break;
                        }
                        Err(_) => {
                            audit_session_stream_disconnect(
                                &session,
                                SessionFailureReason::RealtimeSessionUnavailable,
                            );
                            yield NotificationStreamEvent::SessionUnavailable;
                            break;
                        }
                    }
                }
                permission_result = permission_rx.recv() => {
                    match permission_result {
                        Ok(event) if event.applies_to(&tenant, user_id) => {
                            match revalidate().await {
                                Ok(true) => yield NotificationStreamEvent::PermissionChanged,
                                Ok(false) => {
                                    audit_session_stream_disconnect(&session, SessionFailureReason::RealtimeSessionInvalid);
                                    yield NotificationStreamEvent::SessionInvalid;
                                    break;
                                }
                                Err(_) => {
                                    audit_session_stream_disconnect(&session, SessionFailureReason::RealtimeSessionUnavailable);
                                    yield NotificationStreamEvent::SessionUnavailable;
                                    break;
                                }
                            }
                        }
                        Ok(_) => {}
                        Err(_) => {
                            yield NotificationStreamEvent::SessionUnavailable;
                            break;
                        }
                    }
                }
                notification_result = notification_rx.recv() => {
                    match notification_result {
                        Ok(event) if event.applies_to(&tenant, user_id) => {
                            if let Ok(data) = serde_json::to_string(&event.notification) {
                                yield NotificationStreamEvent::Notification(data);
                            }
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            yield NotificationStreamEvent::ReconcileRequired;
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                work_result = work_rx.recv() => {
                    match work_result {
                        Ok(event) if event.applies_to(&tenant) => {
                            yield NotificationStreamEvent::WorkChanged(event.event_name());
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            yield NotificationStreamEvent::ReconcileRequired;
                        }
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    }
}

/// List notifications for current user
#[cfg_attr(feature = "api-docs", utoipa::path(
    get,
    path = "/api/notifications",
    operation_id = "listNotifications",
    tag = "notifications",
    params(ListNotificationsQuery),
    responses(
        (status = 200, description = "Current user's notifications", body = ApiResponse<crate::models::ListNotificationsResponse>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse)
    )
))]
pub async fn list_notifications(
    Extension(session): Extension<AuthenticatedSession>,
    Query(query): Query<ListNotificationsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_session(&session);
    let notifications =
        notification_service::list_notifications(&context.tenant.pool, context.user_id, query)
            .await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(notifications))))
}

/// Mark a notification as read
pub async fn mark_as_read(
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_session(&session);
    notification_service::mark_as_read(&context.tenant.pool, context.user_id, id).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("อ่านแล้ว")),
    ))
}

/// Mark all notifications as read
pub async fn mark_all_as_read(
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_session(&session);
    notification_service::mark_all_as_read(&context.tenant.pool, context.user_id).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("อ่านทั้งหมดแล้ว")),
    ))
}

// SSE Handler
pub async fn stream_notifications(
    State(state): State<NotificationHttpState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Response, AppError> {
    let notification_rx = state.notification_channel.subscribe();
    let permission_rx = state.auth_runtime.permission_events.subscribe();
    let work_rx = state.work_event_channel.subscribe();
    let session_rx = state.auth_runtime.session_events.subscribe();

    if !session_service::revalidate(&session, Utc::now()).await? {
        return Err(AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string()));
    }

    let mut revalidation_interval = interval_at(
        Instant::now() + SESSION_REVALIDATION_INTERVAL,
        SESSION_REVALIDATION_INTERVAL,
    );
    revalidation_interval.set_missed_tick_behavior(MissedTickBehavior::Delay);
    let mut session = session;
    session.tenant.pool.release_for_realtime();
    let revalidation_session = session.clone();
    let revalidation_runtime = state.auth_runtime.clone();
    let events = session_bound_notification_stream(
        session,
        notification_rx,
        permission_rx,
        work_rx,
        session_rx,
        revalidation_interval,
        move || {
            let session = revalidation_session.clone();
            let runtime = revalidation_runtime.clone();
            async move {
                school_auth_http::tenant::revalidate_realtime_session(&runtime, &session).await
            }
        },
    );
    let stream = events.map(|event| Ok::<_, Infallible>(event.into_sse_event()));
    let mut response = Sse::new(stream)
        .keep_alive(KeepAlive::default())
        .into_response();
    response
        .headers_mut()
        .insert(X_ACCEL_BUFFERING, HeaderValue::from_static("no"));
    Ok(response)
}

#[cfg_attr(feature="api-docs", utoipa::path(get,path="/api/notifications/push/config",operation_id="getPushConfig",tag="notifications",responses((status=200,description="Current Web Push key and admission limits",body=ApiResponse<crate::models::PushConfigResponse>),(status=401,description="Authentication required",body=ApiErrorResponse))))]
pub async fn push_config(
    Extension(_session): Extension<AuthenticatedSession>,
) -> impl IntoResponse {
    Json(ApiResponse::ok(crate::devices::push_config()))
}

#[cfg_attr(feature="api-docs", utoipa::path(get,path="/api/notifications/push/devices",operation_id="listPushDevices",tag="notifications",responses((status=200,description="Current user's active Push device bindings",body=ApiResponse<crate::models::ListPushDevicesResponse>),(status=401,description="Authentication required",body=ApiErrorResponse))))]
pub async fn list_push_devices(
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_session(&session);
    Ok(Json(ApiResponse::ok(
        crate::devices::list(&context.tenant.pool, context.user_id).await?,
    )))
}
#[cfg_attr(feature="api-docs", utoipa::path(post,path="/api/notifications/push/devices",operation_id="registerPushDevice",tag="notifications",request_body=RegisterPushDeviceRequest,responses((status=200,description="Registered or refreshed this installation binding",body=ApiResponse<crate::models::PushDevice>),(status=400,description="Invalid endpoint, label or crypto keys",body=ApiErrorResponse),(status=401,description="Authentication required",body=ApiErrorResponse),(status=409,description="Key or binding changed, or device limit reached",body=ApiErrorResponse),(status=503,description="Push configuration unavailable",body=ApiErrorResponse))))]
pub async fn register_push_device(
    Extension(session): Extension<AuthenticatedSession>,
    Json(request): Json<RegisterPushDeviceRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_session(&session);
    Ok(Json(ApiResponse::ok(
        crate::devices::register(
            &context.tenant.pool,
            context.user_id,
            session.session_id,
            request,
        )
        .await?,
    )))
}
#[cfg_attr(feature="api-docs", utoipa::path(delete,path="/api/notifications/push/devices/{id}",operation_id="unregisterPushDevice",tag="notifications",params(("id"=Uuid,Path,description="Owned device ID"),UnregisterPushDeviceQuery),responses((status=200,description="Exact owned binding revoked or already absent",body=ApiResponse<EmptyData>),(status=401,description="Authentication required",body=ApiErrorResponse))))]
pub async fn unregister_push_device(
    Extension(session): Extension<AuthenticatedSession>,
    Path(id): Path<Uuid>,
    Query(query): Query<UnregisterPushDeviceQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_session(&session);
    crate::devices::unregister(&context.tenant.pool, context.user_id, id, query.version).await?;
    Ok(Json(ApiResponse::empty_with_message("Push device revoked")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;
    use school_auth::events::SessionRevocationEvent;
    use school_tenancy::TenantContext;
    use sqlx::postgres::PgPoolOptions;
    use std::future;
    use std::time::Duration;
    use tokio::time::{interval_at, Instant};

    fn authenticated_session(tenant: &str) -> AuthenticatedSession {
        AuthenticatedSession::for_tests(
            std::sync::Arc::new(school_auth::session_cache::SessionCache::new()),
            TenantContext {
                tenant_id: Uuid::new_v4(),
                subdomain: tenant.to_string(),
                pool: PgPoolOptions::new()
                    .connect_lazy("postgres://invalid:invalid@127.0.0.1:1/invalid")
                    .unwrap()
                    .into(),
            },
            Uuid::new_v4(),
            Uuid::new_v4(),
            "staff",
        )
    }

    fn event_receivers() -> (
        broadcast::Sender<crate::events::TenantNotificationEvent>,
        broadcast::Receiver<crate::events::TenantNotificationEvent>,
        broadcast::Sender<school_auth::events::PermissionChangeEvent>,
        broadcast::Receiver<school_auth::events::PermissionChangeEvent>,
        broadcast::Sender<crate::events::WorkChangeEvent>,
        broadcast::Receiver<crate::events::WorkChangeEvent>,
        broadcast::Sender<SessionRevocationEvent>,
        broadcast::Receiver<SessionRevocationEvent>,
    ) {
        let (notification_tx, notification_rx) = broadcast::channel(8);
        let (permission_tx, permission_rx) = broadcast::channel(8);
        let (work_tx, work_rx) = broadcast::channel(8);
        let (session_tx, session_rx) = broadcast::channel(8);
        (
            notification_tx,
            notification_rx,
            permission_tx,
            permission_rx,
            work_tx,
            work_rx,
            session_tx,
            session_rx,
        )
    }

    #[tokio::test]
    async fn reconciliation_control_event_has_empty_wire_payload() {
        let stream = futures::stream::iter([Ok::<_, Infallible>(
            NotificationStreamEvent::ReconcileRequired.into_sse_event(),
        )]);
        let response = Sse::new(stream).into_response();
        let bytes = axum::body::to_bytes(response.into_body(), 1024)
            .await
            .unwrap();
        assert_eq!(&bytes[..], b"event: reconcile_required\ndata: {}\n\n");
    }

    #[tokio::test]
    async fn missed_data_signals_request_reconciliation_and_keep_tenant_filtering() {
        for notification_lag in [false, true] {
            let session = authenticated_session("demo");
            let (
                notification_tx,
                notification_rx,
                permission_tx,
                permission_rx,
                work_tx,
                work_rx,
                session_tx,
                session_rx,
            ) = event_receivers();
            let stream = session_bound_notification_stream(
                session,
                notification_rx,
                permission_rx,
                work_rx,
                session_rx,
                interval_at(
                    Instant::now() + Duration::from_secs(3600),
                    Duration::from_secs(3600),
                ),
                || future::ready(Ok::<bool, school_errors::AppError>(true)),
            );
            futures::pin_mut!(stream);
            for _ in 0..9 {
                if notification_lag {
                    notification_tx
                        .send(TenantNotificationEvent::new(
                            "other",
                            Uuid::new_v4(),
                            crate::models::Notification {
                                id: Uuid::new_v4(),
                                title: "private".into(),
                                message: "private".into(),
                                type_: "info".into(),
                                link: None,
                                read_at: None,
                                created_at: Utc::now(),
                            },
                        ))
                        .unwrap();
                } else {
                    work_tx
                        .send(WorkChangeEvent::work_items_changed("other"))
                        .unwrap();
                }
            }
            assert_eq!(
                stream.next().await,
                Some(NotificationStreamEvent::ReconcileRequired)
            );
            assert!(
                tokio::time::timeout(Duration::from_millis(5), stream.next())
                    .await
                    .is_err()
            );
            work_tx
                .send(WorkChangeEvent::workflow_window_changed("demo"))
                .unwrap();
            assert_eq!(
                stream.next().await,
                Some(NotificationStreamEvent::WorkChanged(
                    "workflow_window_changed"
                ))
            );
            drop((notification_tx, permission_tx, work_tx, session_tx));
        }
    }

    #[tokio::test]
    async fn missed_identity_signals_close_instead_of_using_cached_identity() {
        for permission_lag in [false, true] {
            let session = authenticated_session("demo");
            let (
                notification_tx,
                notification_rx,
                permission_tx,
                permission_rx,
                work_tx,
                work_rx,
                session_tx,
                session_rx,
            ) = event_receivers();
            let stream = session_bound_notification_stream(
                session.clone(),
                notification_rx,
                permission_rx,
                work_rx,
                session_rx,
                interval_at(
                    Instant::now() + Duration::from_secs(3600),
                    Duration::from_secs(3600),
                ),
                || future::ready(Ok::<bool, school_errors::AppError>(true)),
            );
            futures::pin_mut!(stream);
            for _ in 0..9 {
                if permission_lag {
                    permission_tx
                        .send(PermissionChangeEvent::for_all_users("demo"))
                        .unwrap();
                } else {
                    session_tx
                        .send(SessionRevocationEvent::user("other", Uuid::new_v4(), None))
                        .unwrap();
                }
            }
            assert_eq!(
                stream.next().await,
                Some(NotificationStreamEvent::SessionUnavailable)
            );
            assert_eq!(stream.next().await, None);
            drop((notification_tx, permission_tx, work_tx, session_tx));
        }
    }

    #[tokio::test]
    async fn permission_signal_revalidates_user_status_before_delivering_more_data() {
        let session = authenticated_session("demo");
        let (
            notification_tx,
            notification_rx,
            permission_tx,
            permission_rx,
            work_tx,
            work_rx,
            session_tx,
            session_rx,
        ) = event_receivers();
        let stream = session_bound_notification_stream(
            session.clone(),
            notification_rx,
            permission_rx,
            work_rx,
            session_rx,
            interval_at(
                Instant::now() + Duration::from_secs(3600),
                Duration::from_secs(3600),
            ),
            || future::ready(Ok::<bool, school_errors::AppError>(false)),
        );
        futures::pin_mut!(stream);
        permission_tx
            .send(PermissionChangeEvent::for_all_users("demo"))
            .unwrap();
        assert_eq!(
            stream.next().await,
            Some(NotificationStreamEvent::SessionInvalid)
        );
        assert_eq!(stream.next().await, None);
        drop((notification_tx, work_tx, session_tx));
    }

    #[tokio::test]
    async fn matching_local_session_signal_yields_one_invalid_control_event_then_ends() {
        let session = authenticated_session("demo");
        let (
            notification_tx,
            notification_rx,
            permission_tx,
            permission_rx,
            work_tx,
            work_rx,
            session_tx,
            session_rx,
        ) = event_receivers();
        let stream = session_bound_notification_stream(
            session.clone(),
            notification_rx,
            permission_rx,
            work_rx,
            session_rx,
            interval_at(
                Instant::now() + Duration::from_secs(3600),
                Duration::from_secs(3600),
            ),
            || future::ready(Ok::<bool, school_errors::AppError>(true)),
        );
        futures::pin_mut!(stream);
        session_tx
            .send(SessionRevocationEvent::session(
                "demo",
                session.user_id,
                session.session_id,
            ))
            .unwrap();

        assert_eq!(
            stream.next().await,
            Some(NotificationStreamEvent::SessionInvalid)
        );
        assert_eq!(stream.next().await, None);
        drop((notification_tx, permission_tx, work_tx));
    }

    #[tokio::test]
    async fn nonmatching_session_signals_are_ignored() {
        let session = authenticated_session("demo");
        let (
            notification_tx,
            notification_rx,
            permission_tx,
            permission_rx,
            work_tx,
            work_rx,
            session_tx,
            session_rx,
        ) = event_receivers();
        let stream = session_bound_notification_stream(
            session.clone(),
            notification_rx,
            permission_rx,
            work_rx,
            session_rx,
            interval_at(
                Instant::now() + Duration::from_secs(3600),
                Duration::from_secs(3600),
            ),
            || future::ready(Ok::<bool, school_errors::AppError>(true)),
        );
        futures::pin_mut!(stream);
        session_tx
            .send(SessionRevocationEvent::session(
                "other",
                session.user_id,
                session.session_id,
            ))
            .unwrap();
        session_tx
            .send(SessionRevocationEvent::session(
                "demo",
                session.user_id,
                session.session_id,
            ))
            .unwrap();

        assert_eq!(
            stream.next().await,
            Some(NotificationStreamEvent::SessionInvalid)
        );
        assert_eq!(stream.next().await, None);
        drop((notification_tx, permission_tx, work_tx));
    }

    #[tokio::test]
    async fn failed_database_revalidation_yields_one_invalid_control_event_then_ends() {
        let session = authenticated_session("demo");
        let (
            notification_tx,
            notification_rx,
            permission_tx,
            permission_rx,
            work_tx,
            work_rx,
            session_tx,
            session_rx,
        ) = event_receivers();
        let stream = session_bound_notification_stream(
            session,
            notification_rx,
            permission_rx,
            work_rx,
            session_rx,
            interval_at(Instant::now(), Duration::from_secs(3600)),
            || future::ready(Ok::<bool, school_errors::AppError>(false)),
        );
        futures::pin_mut!(stream);

        assert_eq!(
            stream.next().await,
            Some(NotificationStreamEvent::SessionInvalid)
        );
        assert_eq!(stream.next().await, None);
        drop((notification_tx, permission_tx, work_tx, session_tx));
    }

    #[tokio::test]
    async fn database_error_yields_one_redacted_unavailable_control_event_then_ends() {
        let session = authenticated_session("demo");
        let (
            notification_tx,
            notification_rx,
            permission_tx,
            permission_rx,
            work_tx,
            work_rx,
            session_tx,
            session_rx,
        ) = event_receivers();
        let stream = session_bound_notification_stream(
            session,
            notification_rx,
            permission_rx,
            work_rx,
            session_rx,
            interval_at(Instant::now(), Duration::from_secs(3600)),
            || {
                future::ready(Err(AppError::ServiceUnavailable(
                    "sensitive database detail".into(),
                )))
            },
        );
        futures::pin_mut!(stream);

        assert_eq!(
            stream.next().await,
            Some(NotificationStreamEvent::SessionUnavailable)
        );
        assert_eq!(stream.next().await, None);
        drop((notification_tx, permission_tx, work_tx, session_tx));
    }
}
