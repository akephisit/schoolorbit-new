use crate::api_response::ApiResponse;
use crate::error::AppError;
use crate::modules::notification::models::{
    CreateNotificationRequest, ListNotificationsQuery, SubscribePushRequest,
};
use crate::modules::notification::services as notification_service;
use crate::utils::request_context::current_user_tenant_context_from_headers;
use crate::AppState;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use futures::stream::Stream;
use std::convert::Infallible;
use tokio::sync::broadcast;
use uuid::Uuid;

/// List notifications for current user
pub async fn list_notifications(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListNotificationsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_headers(&state, &headers).await?;
    let notifications =
        notification_service::list_notifications(&context.tenant.pool, context.user_id, query)
            .await?;

    Ok((StatusCode::OK, Json(ApiResponse::ok(notifications))))
}

/// Mark a notification as read
pub async fn mark_as_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_headers(&state, &headers).await?;
    notification_service::mark_as_read(&context.tenant.pool, context.user_id, id).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("อ่านแล้ว")),
    ))
}

/// Mark all notifications as read
pub async fn mark_all_as_read(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_headers(&state, &headers).await?;
    notification_service::mark_all_as_read(&context.tenant.pool, context.user_id).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message("อ่านทั้งหมดแล้ว")),
    ))
}

// SSE Handler
pub async fn stream_notifications(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    let context = current_user_tenant_context_from_headers(&state, &headers).await?;
    let user_id = context.user_id;
    let tenant = context.tenant.subdomain;

    let mut notification_rx = state.notification_channel.subscribe();
    let mut permission_rx = state.permission_event_channel.subscribe();
    let mut work_rx = state.work_event_channel.subscribe();

    let stream = async_stream::stream! {
        loop {
            tokio::select! {
                notification_result = notification_rx.recv() => {
                    match notification_result {
                        Ok(event) if event.applies_to(&tenant, user_id) => {
                            if let Ok(data) = serde_json::to_string(&event.notification) {
                                yield Ok(Event::default().data(data));
                            }
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                permission_result = permission_rx.recv() => {
                    match permission_result {
                        Ok(event) if event.applies_to(&tenant, user_id) => {
                            yield Ok(Event::default().event("permission_changed").data("{}"));
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                work_result = work_rx.recv() => {
                    match work_result {
                        Ok(event) if event.applies_to(&tenant) => {
                            yield Ok(Event::default().event(event.event_name()).data("{}"));
                        }
                        Ok(_) => {}
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
            }
        }
    };

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}

/// Create manual notification (For testing/internal use)
pub async fn create_notification(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<CreateNotificationRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_headers(&state, &headers).await?;
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
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SubscribePushRequest>,
) -> Result<impl IntoResponse, AppError> {
    let context = current_user_tenant_context_from_headers(&state, &headers).await?;
    notification_service::subscribe_push(&context.tenant.pool, context.user_id, payload).await?;

    Ok((
        StatusCode::OK,
        Json(ApiResponse::empty_with_message(
            "Subscribed to push notifications",
        )),
    ))
}
