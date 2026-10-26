use sqlx::PgPool;
use uuid::Uuid;

use school_errors::AppError;

use super::models::{ListNotificationsQuery, ListNotificationsResponse, Notification};

pub async fn list_notifications(
    pool: &PgPool,
    user_id: Uuid,
    query: ListNotificationsQuery,
) -> Result<ListNotificationsResponse, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "notifications.list_notifications",
    )
    .await?;
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

    let items = school_database::DbSession::new(&mut *connection)
        .query_as::<Notification>(
            "notifications.src.services.list_notifications",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(user_id)
        .bind(params.limit)
        .bind(params.offset)
        .fetch_all()
        .await
        .map_err(|e| {
            tracing::error!(
                error_class = school_database::error_class(&e),
                "Failed to fetch notifications"
            );
            AppError::InternalServerError("ไม่สามารถดึงข้อมูลการแจ้งเตือนได้".to_string())
        })?;

    let unread_count: i64 = school_database::DbSession::new(&mut *connection)
        .scalar(
            "notifications.src.services.list_notifications",
            "SELECT COUNT(*) FROM notifications WHERE user_id = $1 AND read_at IS NULL",
        )
        .bind(user_id)
        .fetch_one()
        .await
        .map_err(|e| {
            tracing::error!(
                error_class = school_database::error_class(&e),
                "Failed to count unread notifications"
            );
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
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "notifications.mark_as_read")
            .await?;
    school_database::DbSession::new(&mut *connection)
        .query(
            "notifications.src.services.mark_as_read",
            "UPDATE notifications SET read_at = coalesce(read_at,NOW()) WHERE id = $1 AND user_id = $2",
        )
        .bind(notification_id)
        .bind(user_id)
        .execute()
        .await
        .map_err(|e| {
            tracing::error!(
                error_class = school_database::error_class(&e),
                "Failed to mark notification as read"
            );
            AppError::InternalServerError("เกิดข้อผิดพลาดในการอัพเดตสถานะ".to_string())
        })?;

    Ok(())
}

pub async fn mark_all_as_read(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "notifications.mark_all_as_read")
            .await?;
    school_database::DbSession::new(&mut *connection)
        .query(
            "notifications.src.services.mark_all_as_read",
            "UPDATE notifications SET read_at = NOW() WHERE user_id = $1 AND read_at IS NULL",
        )
        .bind(user_id)
        .execute()
        .await
        .map_err(|e| {
            tracing::error!(
                error_class = school_database::error_class(&e),
                "Failed to mark all notifications as read"
            );
            AppError::InternalServerError("เกิดข้อผิดพลาดในการอัพเดตสถานะ".to_string())
        })?;

    Ok(())
}

struct NotificationPageParams {
    page: i64,
    limit: i64,
    offset: i64,
}

fn notification_page_params(query: &ListNotificationsQuery) -> NotificationPageParams {
    let page = query.page.unwrap_or(1).clamp(1, 1_000_000);
    let limit = query.limit.unwrap_or(20).clamp(1, 100);
    NotificationPageParams {
        page,
        limit,
        offset: (page - 1) * limit,
    }
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
}
