use crate::{events::TenantNotificationEvent, models::Notification};
use std::env;
use tokio::sync::broadcast;
use uuid::Uuid;
use web_push::*;

pub struct NotificationService;

pub struct TenantNotificationPublisher<'a> {
    tenant: &'a str,
    notification_tx: &'a broadcast::Sender<TenantNotificationEvent>,
}

impl<'a> TenantNotificationPublisher<'a> {
    pub fn new(
        tenant: &'a str,
        notification_tx: &'a broadcast::Sender<TenantNotificationEvent>,
    ) -> Self {
        Self {
            tenant,
            notification_tx,
        }
    }

    fn publish(&self, user_id: Uuid, notification: Notification) {
        drop(self.notification_tx.send(TenantNotificationEvent::new(
            self.tenant,
            user_id,
            notification,
        )));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationType {
    Info,
    Success,
    Warning,
    Error,
}

impl NotificationType {
    pub fn as_str(&self) -> &'static str {
        match self {
            NotificationType::Info => "info",
            NotificationType::Success => "success",
            NotificationType::Warning => "warning",
            NotificationType::Error => "error",
        }
    }
}

impl NotificationService {
    /// Deliver an already durably stored notification without inserting a second row.
    pub async fn deliver_stored(
        pool: &sqlx::PgPool,
        publisher: &TenantNotificationPublisher<'_>,
        user_id: Uuid,
        notification: Notification,
    ) -> Result<(), &'static str> {
        publisher.publish(user_id, notification.clone());
        let subscribed: bool =
            sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM push_subscriptions WHERE user_id=$1)")
                .bind(user_id)
                .fetch_one(pool)
                .await
                .map_err(|_| "notification_subscription_unavailable")?;
        if !subscribed {
            return Ok(());
        }
        Self::send_web_push(
            pool,
            user_id,
            notification.id,
            &notification.title,
            &notification.message,
            notification.link.as_deref(),
        )
        .await
        .map_err(|_| "notification_transport_unavailable")
    }

    /// Send a notification to a specific user.
    /// Store the inbox and durable dispatch job atomically; transport follows commit.
    pub async fn send(
        pool: &sqlx::PgPool,
        publisher: &TenantNotificationPublisher<'_>,
        user_id: Uuid,
        title: &str,
        message: &str,
        type_: NotificationType,
        link: Option<&str>,
    ) -> Result<Uuid, sqlx::Error> {
        let item = crate::dispatch::QueuedNotification {
            id: Uuid::new_v4(),
            notification_id: Uuid::new_v4(),
            recipient_id: user_id,
            source: crate::dispatch::NotificationSource::Direct,
            dedup_key: None,
            title: title.into(),
            message: message.into(),
            type_,
            link: link.map(str::to_owned),
        };
        let mut tx = pool.begin().await?;
        let id = crate::dispatch::store_and_enqueue_in_transaction(&mut tx, &item)
            .await
            .map_err(|_| sqlx::Error::Protocol("notification_enqueue_failed".into()))?;
        tx.commit().await?;
        crate::runtime::wake_after_commit(publisher.tenant);
        Ok(id)
    }

    /// Internal helper to send Web Push
    async fn send_web_push(
        pool: &sqlx::PgPool,
        user_id: Uuid,
        notification_id: Uuid,
        title: &str,
        message: &str,
        link: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 3. Get user subscriptions
        #[derive(sqlx::FromRow)]
        struct SubRow {
            endpoint: String,
            p256dh_key: String,
            auth_key: String,
        }

        let subs = sqlx::query_as::<_, SubRow>(
            "SELECT endpoint, p256dh_key, auth_key FROM push_subscriptions WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        if subs.is_empty() {
            return Ok(());
        }

        // 1. Get VAPID config from .env
        let _vapid_public = env::var("VAPID_PUBLIC_KEY")?;
        let vapid_private = env::var("VAPID_PRIVATE_KEY")?;
        let vapid_subject =
            env::var("VAPID_SUBJECT").unwrap_or_else(|_| "mailto:admin@localhost".to_string());

        // 4. Construct payload
        let payload = serde_json::json!({
            "id": notification_id,
            "title": title,
            "body": message,
            "link": link,
            // Add icon if needed
        })
        .to_string();

        let client = IsahcWebPushClient::new()?;
        let mut retryable = 0usize;
        let mut removed = 0usize;
        for sub in subs {
            let subscription_info = SubscriptionInfo {
                endpoint: sub.endpoint.clone(),
                keys: SubscriptionKeys {
                    p256dh: sub.p256dh_key,
                    auth: sub.auth_key,
                },
            };
            // One malformed/device failure must not skip the remaining subscriptions.
            let result = async {
                let mut builder = WebPushMessageBuilder::new(&subscription_info);
                builder.set_payload(ContentEncoding::Aes128Gcm, payload.as_bytes());
                let mut signature =
                    VapidSignatureBuilder::from_base64(&vapid_private, &subscription_info)?;
                signature.add_claim("sub", vapid_subject.clone());
                builder.set_vapid_signature(signature.build()?);
                client.send(builder.build()?).await
            }
            .await;
            match result {
                Ok(()) => {}
                Err(WebPushError::EndpointNotValid(_) | WebPushError::EndpointNotFound(_)) => {
                    if sqlx::query(
                        "DELETE FROM push_subscriptions WHERE endpoint=$1 AND user_id=$2",
                    )
                    .bind(&sub.endpoint)
                    .bind(user_id)
                    .execute(pool)
                    .await
                    .is_ok()
                    {
                        removed += 1;
                    } else {
                        retryable += 1;
                    }
                }
                Err(_) => retryable += 1,
            }
        }
        if removed > 0 {
            tracing::debug!(
                error_code = "notification_subscription_removed",
                removed,
                "Invalid push subscriptions removed"
            );
        }
        if retryable > 0 {
            tracing::warn!(error_code="notification_push_retry",retryable,notification_id=%notification_id,"Push subscription delivery will retry");
            return Err(std::io::Error::other("notification_push_retry").into());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn tenant_publisher_emits_supplied_tenant_and_user() {
        let (notification_tx, mut notification_rx) = broadcast::channel(1);
        let tenant = "tenant-a";
        let user_id = Uuid::new_v4();
        let publisher = TenantNotificationPublisher::new(tenant, &notification_tx);
        let notification = Notification {
            id: Uuid::new_v4(),
            title: "Test".to_string(),
            message: "Message".to_string(),
            type_: "info".to_string(),
            link: None,
            read_at: None,
            created_at: Utc::now(),
        };

        publisher.publish(user_id, notification);

        let event = notification_rx
            .try_recv()
            .expect("tenant notification event should be published");
        assert_eq!(event.tenant, tenant);
        assert_eq!(event.user_id, user_id);
    }
}
