use crate::db::school_mapping::get_school_database_url;
use crate::middleware::auth::extract_user_id;
use crate::utils::subdomain::extract_subdomain_from_request;
use crate::AppState;
use crate::error::AppError;
use axum::{
    extract::{Path, State, Query},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use axum::response::sse::{Event, Sse, KeepAlive};
use futures::stream::{self, Stream};
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::StreamExt as _;
use tokio::sync::broadcast;

// Models
#[derive(Debug, Serialize, Clone, sqlx::FromRow)]
pub struct Notification {
    pub id: Uuid,
    pub title: String,
    pub message: String,
    pub type_: String, // "info", "success", "warning", "error"
    pub link: Option<String>,
    pub read_at: Option<chrono::DateTime<chrono::Utc>>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct ListNotificationsQuery {
    pub page: Option<i64>,
    pub limit: Option<i64>,
    pub unread_only: Option<bool>,
}

#[derive(Debug, Deserialize)]
pub struct CreateNotificationRequest {
    #[serde(default)]
    pub user_id: Option<Uuid>, // Optional: if None, send to self (creator)
    pub title: String,
    pub message: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub link: Option<String>,
}

// Handlers

/// List notifications for current user
pub async fn list_notifications(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<ListNotificationsQuery>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get database pool: {}", e);
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;

    let user_id = extract_user_id(&headers, &pool)
        .await
        .map_err(|e| AppError::AuthError(e))?;

    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    let offset = (page - 1) * limit;

    let mut sql = r#"
        SELECT id, title, message, type AS type_, link, read_at, created_at
        FROM notifications
        WHERE user_id = $1
    "#.to_string();

    if query.unread_only.unwrap_or(false) {
        sql.push_str(" AND read_at IS NULL");
    }

    sql.push_str(" ORDER BY created_at DESC LIMIT $2 OFFSET $3");

    let notifications = sqlx::query_as::<_, Notification>(&sql)
        .bind(user_id)
        .bind(limit)
        .bind(offset)
        .fetch_all(&pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch notifications: {}", e);
            AppError::InternalServerError("ไม่สามารถดึงข้อมูลการแจ้งเตือนได้".to_string())
        })?;

    // Count unread
    let unread_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND read_at IS NULL"
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap_or(0);

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "data": notifications,
            "unread_count": unread_count,
            "page": page,
            "limit": limit
        })),
    ))
}

/// Mark a notification as read
pub async fn mark_as_read(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get database pool: {}", e);
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;

    let user_id = extract_user_id(&headers, &pool)
        .await
        .map_err(|e| AppError::AuthError(e))?;

    sqlx::query(
        "UPDATE notifications SET read_at = NOW() WHERE id = $1 AND user_id = $2"
    )
    .bind(id)
    .bind(user_id)
    .execute(&pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to mark notification as read: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการอัพเดตสถานะ".to_string())
    })?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "อ่านแล้ว"
        })),
    ))
}

/// Mark all notifications as read
pub async fn mark_all_as_read(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get school database: {}", e);
            AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| {
            tracing::error!("Failed to get database pool: {}", e);
            AppError::InternalServerError("ไม่สามารถเชื่อมต่อฐานข้อมูลได้".to_string())
        })?;

    let user_id = extract_user_id(&headers, &pool)
        .await
        .map_err(|e| AppError::AuthError(e))?;

    sqlx::query(
        "UPDATE notifications SET read_at = NOW() WHERE user_id = $1 AND read_at IS NULL"
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to mark all notifications as read: {}", e);
        AppError::InternalServerError("เกิดข้อผิดพลาดในการอัพเดตสถานะ".to_string())
    })?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "อ่านทั้งหมดแล้ว"
        })),
    ))
}

// SSE Handler
pub async fn stream_notifications(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, AppError> {
     let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    // We verify the user, but we don't need the DB connection for the stream itself, 
    // unless to verify the user exists/is active.
    // However, to extract_user_id cleanly we need the pool.
    
    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|e| {
             // If DB lookup fails, we can't authenticate, so unauthorized or not found
             AppError::NotFound("ไม่พบโรงเรียน".to_string())
        })?;
        
    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|e| AppError::InternalServerError("Database error".to_string()))?;

    let user_id = extract_user_id(&headers, &pool)
        .await
        .map_err(|e| AppError::AuthError(e))?;

    let mut rx = state.notification_channel.subscribe();

    let stream = async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok((target_user_id, notification)) => {
                    if target_user_id == user_id {
                         if let Ok(data) = serde_json::to_string(&notification) {
                             yield Ok(Event::default().data(data));
                         }
                    }
                }
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    // Skip lagged messages
                }
                Err(broadcast::error::RecvError::Closed) => {
                    break;
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
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|_| AppError::NotFound("ไม่พบโรงเรียน".to_string()))?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    let user_id = extract_user_id(&headers, &pool)
        .await
        .map_err(|e| AppError::AuthError(e))?;
    
    // In real app, check permissions here (e.g. only staff/admin can broadcast)
    // For now, allow test.

    use crate::services::notification::{NotificationService, NotificationType};

    let notif_type = match payload.type_.as_str() {
        "success" => NotificationType::Success,
        "warning" => NotificationType::Warning,
        "error" => NotificationType::Error,
        _ => NotificationType::Info,
    };


    let target_user_id = payload.user_id.unwrap_or(user_id);

    NotificationService::send(
        &pool,
        &state.notification_channel,
        target_user_id,
        &payload.title,
        &payload.message,
        notif_type,
        payload.link.as_deref(),
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to create notification: {}", e);
        AppError::InternalServerError("Failed to create notification".to_string())
    })?;

    Ok((
        StatusCode::CREATED,
        Json(serde_json::json!({
            "success": true,
            "message": "Notification created"
        })),
    ))
}

#[derive(Debug, Deserialize)]
pub struct SubscribePushRequest {
    pub endpoint: String,
    pub p256dh: String,
    pub auth: String,
}

/// Subscribe to Web Push Notifications
pub async fn subscribe_push(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SubscribePushRequest>,
) -> Result<impl IntoResponse, AppError> {
    let subdomain = extract_subdomain_from_request(&headers)
        .map_err(|_| AppError::BadRequest("Missing or invalid subdomain".to_string()))?;

    let db_url = get_school_database_url(&state.admin_pool, &subdomain)
        .await
        .map_err(|_| AppError::NotFound("ไม่พบโรงเรียน".to_string()))?;

    let pool = state.pool_manager.get_pool(&db_url, &subdomain)
        .await
        .map_err(|_| AppError::InternalServerError("Database error".to_string()))?;

    let user_id = extract_user_id(&headers, &pool)
        .await
        .map_err(|e| AppError::AuthError(e))?;

    // Upsert subscription
    // If endpoint exists, update keys and timestamp
    sqlx::query(r#"
        INSERT INTO push_subscriptions (user_id, endpoint, p256dh_key, auth_key, updated_at)
        VALUES ($1, $2, $3, $4, NOW())
        ON CONFLICT (endpoint) DO UPDATE
        SET user_id = EXCLUDED.user_id,
            p256dh_key = EXCLUDED.p256dh_key,
            auth_key = EXCLUDED.auth_key,
            updated_at = NOW()
    "#)
    .bind(user_id)
    .bind(payload.endpoint)
    .bind(payload.p256dh)
    .bind(payload.auth)
    .execute(&pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to save push subscription: {}", e);
        AppError::InternalServerError("Failed to subscribe".to_string())
    })?;

    Ok((
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "Subscribed to push notifications"
        })),
    ))
}



