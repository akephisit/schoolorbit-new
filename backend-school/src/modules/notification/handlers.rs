use crate::error::AppError;
use crate::middleware::auth::extract_user_id;
use crate::modules::notification::models::{
    CreateNotificationRequest, ListNotificationsQuery, SubscribePushRequest,
};
use crate::modules::notification::services as notification_service;
use crate::utils::tenant::resolve_tenant_pool;
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

async fn get_pool(state: &AppState, headers: &HeaderMap) -> Result<sqlx::PgPool, AppError> {
    resolve_tenant_pool(state, headers).await
}

async fn current_user_id(headers: &HeaderMap, pool: &sqlx::PgPool) -> Result<Uuid, AppError> {
    extract_user_id(headers, pool)
        .await
        .map_err(AppError::AuthError)
}

/// List notifications for current user
pub async fn list_notifications(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListNotificationsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = current_user_id(&headers, &pool).await?;
    let notifications = notification_service::list_notifications(&pool, user_id, query).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "success": true, "data": notifications })),
    ))
}

/// Mark a notification as read
pub async fn mark_as_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = current_user_id(&headers, &pool).await?;
    notification_service::mark_as_read(&pool, user_id, id).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "success": true, "data": {}, "message": "อ่านแล้ว" })),
    ))
}

/// Mark all notifications as read
pub async fn mark_all_as_read(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = current_user_id(&headers, &pool).await?;
    notification_service::mark_all_as_read(&pool, user_id).await?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({ "success": true, "data": {}, "message": "อ่านทั้งหมดแล้ว" })),
    ))
}

// SSE Handler
pub async fn stream_notifications(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = current_user_id(&headers, &pool).await?;

    let mut notification_rx = state.notification_channel.subscribe();
    let mut permission_rx = state.permission_event_channel.subscribe();

    let stream = async_stream::stream! {
        loop {
            tokio::select! {
                notification_result = notification_rx.recv() => {
                    match notification_result {
                        Ok((target_user_id, notification)) => {
                            if target_user_id == user_id {
                                if let Ok(data) = serde_json::to_string(&notification) {
                                    yield Ok(Event::default().data(data));
                                }
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {}
                        Err(broadcast::error::RecvError::Closed) => {
                            break;
                        }
                    }
                }
                permission_result = permission_rx.recv() => {
                    match permission_result {
                        Ok(event) => {
                            if event.applies_to(user_id) {
                                yield Ok(Event::default().event("permission_changed").data("{}"));
                            }
                        }
                        Err(broadcast::error::RecvError::Lagged(_)) => {
                            yield Ok(Event::default().event("permission_changed").data("{}"));
                        }
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
    let pool = get_pool(&state, &headers).await?;
    let user_id = current_user_id(&headers, &pool).await?;
    notification_service::create_notification(&pool, &state.notification_channel, user_id, payload)
        .await?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({ "success": true, "data": {}, "message": "Notification created" })),
    ))
}

/// Subscribe to Web Push Notifications
pub async fn subscribe_push(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SubscribePushRequest>,
) -> Result<impl IntoResponse, AppError> {
    let pool = get_pool(&state, &headers).await?;
    let user_id = current_user_id(&headers, &pool).await?;
    notification_service::subscribe_push(&pool, user_id, payload).await?;

    Ok((
        StatusCode::OK,
        Json(
            serde_json::json!({ "success": true, "data": {}, "message": "Subscribed to push notifications" }),
        ),
    ))
}
