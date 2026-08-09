use crate::api_response::{ApiErrorResponse, ApiResponse};
use crate::error::AppError;
use crate::modules::auth::{
    audit::{self, SessionFailureReason},
    events::SessionRevocationEvent,
    session_service::{self, AuthenticatedSession},
};
use crate::modules::notification::events::{
    PermissionChangeEvent, TenantNotificationEvent, WorkChangeEvent,
};
use crate::modules::notification::models::{
    CreateNotificationRequest, ListNotificationsQuery, SubscribePushRequest,
};
use crate::modules::notification::services as notification_service;
use crate::utils::request_context::current_user_tenant_context_from_session;
use crate::AppState;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{
    extract::{Extension, Path, Query, State},
    http::{HeaderName, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use futures::{stream::Stream, StreamExt};
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
    WorkChanged(&'static str),
    SessionInvalid,
    SessionUnavailable,
}

impl NotificationStreamEvent {
    fn into_sse_event(self) -> Event {
        match self {
            Self::Notification(data) => Event::default().data(data),
            Self::PermissionChanged => Event::default().event("permission_changed").data("{}"),
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

fn session_bound_notification_stream<F, Fut>(
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
    Fut: Future<Output = Result<bool, AppError>> + Send + 'static,
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
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
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
                notification_result = notification_rx.recv() => {
                    match notification_result {
                        Ok(event) if event.applies_to(&tenant, user_id) => {
                            if let Ok(data) = serde_json::to_string(&event.notification) {
                                yield NotificationStreamEvent::Notification(data);
                            }
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                permission_result = permission_rx.recv() => {
                    match permission_result {
                        Ok(event) if event.applies_to(&tenant, user_id) => {
                            yield NotificationStreamEvent::PermissionChanged;
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
                work_result = work_rx.recv() => {
                    match work_result {
                        Ok(event) if event.applies_to(&tenant) => {
                            yield NotificationStreamEvent::WorkChanged(event.event_name());
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        }
    }
}

/// List notifications for current user
#[utoipa::path(
    get,
    path = "/api/notifications",
    operation_id = "listNotifications",
    tag = "notifications",
    params(ListNotificationsQuery),
    responses(
        (status = 200, description = "Current user's notifications", body = ApiResponse<crate::modules::notification::models::ListNotificationsResponse>),
        (status = 401, description = "Authentication required", body = ApiErrorResponse)
    )
)]
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
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
) -> Result<Response, AppError> {
    let notification_rx = state.notification_channel.subscribe();
    let permission_rx = state.permission_event_channel.subscribe();
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
    let revalidation_session = session.clone();
    let events = session_bound_notification_stream(
        session,
        notification_rx,
        permission_rx,
        work_rx,
        session_rx,
        revalidation_interval,
        move || {
            let session = revalidation_session.clone();
            async move { session_service::revalidate(&session, Utc::now()).await }
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

/// Create manual notification (For testing/internal use)
pub async fn create_notification(
    State(state): State<AppState>,
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<CreateNotificationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_session(&session);
    notification_service::create_notification(
        &context.tenant.pool,
        &state.notification_channel,
        &context.tenant.subdomain,
        context.user_id,
        payload,
    )
    .await?;

    Ok((
        StatusCode::CREATED,
        Json(ApiResponse::empty_with_message("Notification created")),
    ))
}

/// Subscribe to Web Push Notifications
pub async fn subscribe_push(
    Extension(session): Extension<AuthenticatedSession>,
    Json(payload): Json<SubscribePushRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_session(&session);
    notification_service::subscribe_push(&context.tenant.pool, context.user_id, payload).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message(
            "Subscribed to push notifications",
        )),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::auth::events::SessionRevocationEvent;
    use crate::utils::tenant::TenantContext;
    use futures::StreamExt;
    use sqlx::postgres::PgPoolOptions;
    use std::future;
    use std::time::Duration;
    use tokio::time::{interval_at, Instant};

    fn authenticated_session(tenant: &str) -> AuthenticatedSession {
        AuthenticatedSession {
            tenant: TenantContext {
                tenant_id: Uuid::new_v4(),
                subdomain: tenant.to_string(),
                pool: PgPoolOptions::new()
                    .connect_lazy("postgres://invalid:invalid@127.0.0.1:1/invalid")
                    .unwrap(),
            },
            session_id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            username: "teacher.one".to_string(),
            user_type: "staff".to_string(),
        }
    }

    fn event_receivers() -> (
        broadcast::Sender<crate::modules::notification::events::TenantNotificationEvent>,
        broadcast::Receiver<crate::modules::notification::events::TenantNotificationEvent>,
        broadcast::Sender<crate::modules::notification::events::PermissionChangeEvent>,
        broadcast::Receiver<crate::modules::notification::events::PermissionChangeEvent>,
        broadcast::Sender<crate::modules::notification::events::WorkChangeEvent>,
        broadcast::Receiver<crate::modules::notification::events::WorkChangeEvent>,
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
            || future::ready(Ok(true)),
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
            || future::ready(Ok(true)),
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
            || future::ready(Ok(false)),
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
