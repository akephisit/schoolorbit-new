use sqlx::PgPool;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::notification::events::TenantNotificationEvent;
use crate::services::notification::{
    NotificationService, NotificationType, TenantNotificationPublisher,
};

use super::models::{
    CreateNotificationRequest, ListNotificationsQuery, ListNotificationsResponse, Notification,
    SubscribePushRequest,
};

pub async fn list_notifications(
    pool: &PgPool,
    user_id: Uuid,
    query: ListNotificationsQuery,
) -> Result<ListNotificationsResponse, AppError> {
    let params = notification_page_params(&query);

    let mut sql = r#"
        SELECT id, title, message, type AS type_, link, read_at, created_at
        FROM notifications
        WHERE user_id = $1
        "#
    .to_string();

    if query.unread_only.unwrap_or(false) {
        sql.push_str(" AND read_at IS NULL");
    }

    sql.push_str(" ORDER BY created_at DESC LIMIT $2 OFFSET $3");

    let items = sqlx::query_as::<_, Notification>(&sql)
        .bind(user_id)
        .bind(params.limit)
        .bind(params.offset)
        .fetch_all(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to fetch notifications: {}", e);
            AppError::InternalServerError("ไม่สามารถดึงข้อมูลการแจ้งเตือนได้".to_string())
        })?;

    let unread_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND read_at IS NULL",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to count unread notifications: {}", e);
        AppError::InternalServerError("ไม่สามารถดึงข้อมูลการแจ้งเตือนได้".to_string())
    })?;

    Ok(ListNotificationsResponse {
        items,
        unread_count,
        page: params.page,
        limit: params.limit,
    })
}

pub async fn mark_as_read(
    pool: &PgPool,
    user_id: Uuid,
    notification_id: Uuid,
) -> Result<(), AppError> {
    sqlx::query("UPDATE notifications SET read_at = NOW() WHERE id = $1 AND user_id = $2")
        .bind(notification_id)
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to mark notification as read: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาดในการอัพเดตสถานะ".to_string())
        })?;

    Ok(())
}

pub async fn mark_all_as_read(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    sqlx::query("UPDATE notifications SET read_at = NOW() WHERE user_id = $1 AND read_at IS NULL")
        .bind(user_id)
        .execute(pool)
        .await
        .map_err(|e| {
            tracing::error!("Failed to mark all notifications as read: {}", e);
            AppError::InternalServerError("เกิดข้อผิดพลาดในการอัพเดตสถานะ".to_string())
        })?;

    Ok(())
}

pub async fn create_notification(
    pool: &PgPool,
    notification_channel: &broadcast::Sender<TenantNotificationEvent>,
    tenant: &str,
    actor_user_id: Uuid,
    payload: CreateNotificationRequest,
) -> Result<(), AppError> {
    let notification_type = notification_type_from_str(&payload.type_);
    let target_user_id = notification_target_user_id(payload.user_id, actor_user_id);
    let publisher = TenantNotificationPublisher::new(tenant, notification_channel);

    NotificationService::send(
        pool,
        &publisher,
        target_user_id,
        &payload.title,
        &payload.message,
        notification_type,
        payload.link.as_deref(),
    )
    .await
    .map_err(|e| {
        tracing::error!("Failed to create notification: {}", e);
        AppError::InternalServerError("Failed to create notification".to_string())
    })?;

    Ok(())
}

pub async fn subscribe_push(
    pool: &PgPool,
    user_id: Uuid,
    payload: SubscribePushRequest,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        INSERT INTO push_subscriptions (user_id, endpoint, p256dh_key, auth_key, updated_at)
        VALUES ($1, $2, $3, $4, NOW())
        ON CONFLICT (endpoint) DO UPDATE
        SET user_id = EXCLUDED.user_id,
            p256dh_key = EXCLUDED.p256dh_key,
            auth_key = EXCLUDED.auth_key,
            updated_at = NOW()
        "#,
    )
    .bind(user_id)
    .bind(payload.endpoint)
    .bind(payload.p256dh)
    .bind(payload.auth)
    .execute(pool)
    .await
    .map_err(|e| {
        tracing::error!("Failed to save push subscription: {}", e);
        AppError::InternalServerError("Failed to subscribe".to_string())
    })?;

    Ok(())
}

struct NotificationPageParams {
    page: i64,
    limit: i64,
    offset: i64,
}

fn notification_page_params(query: &ListNotificationsQuery) -> NotificationPageParams {
    let page = query.page.unwrap_or(1);
    let limit = query.limit.unwrap_or(20);
    NotificationPageParams {
        page,
        limit,
        offset: (page - 1) * limit,
    }
}

fn notification_type_from_str(value: &str) -> NotificationType {
    match value {
        "success" => NotificationType::Success,
        "warning" => NotificationType::Warning,
        "error" => NotificationType::Error,
        _ => NotificationType::Info,
    }
}

fn notification_target_user_id(requested_user_id: Option<Uuid>, actor_user_id: Uuid) -> Uuid {
    requested_user_id.unwrap_or(actor_user_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_page_params_default_to_first_page_and_twenty_items() {
        let params = notification_page_params(&ListNotificationsQuery {
            page: None,
            limit: None,
            unread_only: None,
        });

        assert_eq!(params.page, 1);
        assert_eq!(params.limit, 20);
        assert_eq!(params.offset, 0);
    }

    #[test]
    fn notification_page_params_calculate_offset_from_page_and_limit() {
        let params = notification_page_params(&ListNotificationsQuery {
            page: Some(3),
            limit: Some(15),
            unread_only: None,
        });

        assert_eq!(params.page, 3);
        assert_eq!(params.limit, 15);
        assert_eq!(params.offset, 30);
    }

    #[test]
    fn notification_type_defaults_unknown_input_to_info() {
        assert_eq!(notification_type_from_str("unknown").as_str(), "info");
        assert_eq!(notification_type_from_str("warning").as_str(), "warning");
    }

    #[test]
    fn notification_target_user_defaults_to_actor() {
        let actor_user_id = Uuid::new_v4();
        let requested_user_id = Uuid::new_v4();

        assert_eq!(
            notification_target_user_id(None, actor_user_id),
            actor_user_id
        );
        assert_eq!(
            notification_target_user_id(Some(requested_user_id), actor_user_id),
            requested_user_id
        );
    }
}
