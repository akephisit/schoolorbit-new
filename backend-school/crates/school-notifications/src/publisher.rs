use crate::{events::TenantNotificationEvent, models::Notification};
use futures::{stream, StreamExt};
use sqlx::Connection;
use std::{env, time::Duration};
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

#[derive(sqlx::FromRow)]
struct PushSubscription {
    endpoint: String,
    p256dh_key: String,
    auth_key: String,
}

/// Owned transport input; it cannot retain a pool, transaction or database permit.
pub(crate) struct PreparedNotification {
    pub(crate) notification: Notification,
    subscriptions: Vec<PushSubscription>,
}
pub(crate) struct DeliveryReceipt {
    pub(crate) invalid_endpoints: Vec<String>,
    pub(crate) retryable: bool,
}

impl NotificationService {
    /// Deliver an already durably stored notification without inserting a second row.
    pub async fn deliver_stored(
        pool: &sqlx::PgPool,
        publisher: &TenantNotificationPublisher<'_>,
        user_id: Uuid,
        notification: Notification,
    ) -> Result<(), &'static str> {
        let prepared = Self::prepare_stored(pool, user_id, notification, 1).await?;
        let receipt = Self::deliver_prepared(publisher, user_id, prepared).await?;
        Self::complete_delivery(pool, user_id, receipt).await
    }

    pub(crate) async fn prepare_stored(
        pool: &sqlx::PgPool,
        user_id: Uuid,
        notification: Notification,
        attempts: i32,
    ) -> Result<PreparedNotification, &'static str> {
        let mut subscriptions = sqlx::query_as::<_, PushSubscription>(
            "SELECT endpoint, p256dh_key, auth_key FROM push_subscriptions WHERE user_id=$1 ORDER BY endpoint")
            .bind(user_id).fetch_all(pool).await.map_err(|_| "notification_subscription_unavailable")?;
        rotate_for_attempt(&mut subscriptions, attempts);
        Ok(PreparedNotification {
            notification,
            subscriptions,
        })
    }

    pub(crate) async fn deliver_prepared(
        publisher: &TenantNotificationPublisher<'_>,
        user_id: Uuid,
        prepared: PreparedNotification,
    ) -> Result<DeliveryReceipt, &'static str> {
        publisher.publish(user_id, prepared.notification.clone());
        Self::send_web_push(prepared)
            .await
            .map_err(|_| "notification_transport_unavailable")
    }

    pub(crate) async fn complete_delivery(
        pool: &sqlx::PgPool,
        user_id: Uuid,
        receipt: DeliveryReceipt,
    ) -> Result<(), &'static str> {
        let mut connection = pool
            .acquire()
            .await
            .map_err(|_| "notification_database_unavailable")?;
        Self::complete_delivery_on_connection(&mut connection, user_id, receipt).await
    }
    pub(crate) async fn complete_delivery_on_connection(
        connection: &mut sqlx::PgConnection,
        user_id: Uuid,
        receipt: DeliveryReceipt,
    ) -> Result<(), &'static str> {
        for endpoint in receipt.invalid_endpoints {
            sqlx::query("DELETE FROM push_subscriptions WHERE endpoint=$1 AND user_id=$2")
                .bind(endpoint)
                .bind(user_id)
                .execute(&mut *connection)
                .await
                .map_err(|_| "notification_subscription_unavailable")?;
        }
        if receipt.retryable {
            Err("notification_push_retry")
        } else {
            Ok(())
        }
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
        let mut connection = pool.acquire().await?;
        Self::send_on_connection(
            &mut connection,
            publisher,
            user_id,
            title,
            message,
            type_,
            link,
        )
        .await
    }

    /// Preserve the caller's held session while committing each recipient independently.
    pub async fn send_on_connection(
        connection: &mut sqlx::PgConnection,
        publisher: &TenantNotificationPublisher<'_>,
        user_id: Uuid,
        title: &str,
        message: &str,
        type_: NotificationType,
        link: Option<&str>,
    ) -> Result<Uuid, sqlx::Error> {
        let item = crate::dispatch::QueuedNotification {
            id: Uuid::now_v7(),
            notification_id: Uuid::now_v7(),
            recipient_id: user_id,
            source: crate::dispatch::NotificationSource::Direct,
            dedup_key: None,
            title: title.into(),
            message: message.into(),
            type_,
            link: link.map(str::to_owned),
        };
        let mut tx = connection.begin().await?;
        // Direct notifications select their recipient once at acceptance, not retry.
        sqlx::query_scalar::<_, Uuid>("SELECT id FROM users WHERE id=$1 AND status='active'")
            .bind(user_id)
            .fetch_one(&mut *tx)
            .await?;
        let id = crate::dispatch::store_and_enqueue_in_transaction(&mut tx, &item)
            .await
            .map_err(|_| sqlx::Error::Protocol("notification_enqueue_failed".into()))?;
        tx.commit().await?;
        crate::runtime::wake_after_commit(publisher.tenant);
        Ok(id)
    }

    /// External I/O only. Invalid endpoint cleanup returns to the admitted DB phase.
    async fn send_web_push(
        prepared: PreparedNotification,
    ) -> Result<DeliveryReceipt, Box<dyn std::error::Error + Send + Sync>> {
        let PreparedNotification {
            notification,
            subscriptions: subs,
        } = prepared;
        if subs.is_empty() {
            return Ok(DeliveryReceipt {
                invalid_endpoints: Vec::new(),
                retryable: false,
            });
        }
        // 1. Get VAPID config from .env
        let _vapid_public = env::var("VAPID_PUBLIC_KEY")?;
        let vapid_private = env::var("VAPID_PRIVATE_KEY")?;
        let vapid_subject =
            env::var("VAPID_SUBJECT").unwrap_or_else(|_| "mailto:admin@localhost".to_string());

        let payload = serde_json::to_vec(&PushPayload {
            id: notification.id,
            title: &notification.title,
            body: &notification.message,
            link: notification.link.as_deref(),
        })?;

        let client = IsahcWebPushClient::new()?;
        let result = attempt_subscriptions(subs, |sub| {
            let client = &client;
            let payload = &payload;
            let vapid_private = &vapid_private;
            let vapid_subject = &vapid_subject;
            async move {
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
                    builder.set_payload(ContentEncoding::Aes128Gcm, payload.as_slice());
                    let mut signature =
                        VapidSignatureBuilder::from_base64(&vapid_private, &subscription_info)?;
                    signature.add_claim("sub", vapid_subject.clone());
                    builder.set_vapid_signature(signature.build()?);
                    client.send(builder.build()?).await
                }
                .await;
                match result {
                    Ok(()) => Ok(EndpointDelivery::Delivered),
                    Err(WebPushError::EndpointNotValid(_) | WebPushError::EndpointNotFound(_)) => {
                        Ok(EndpointDelivery::Removed(sub.endpoint))
                    }
                    Err(_) => Err(()),
                }
            }
        })
        .await;
        let (counts, retryable) = match result {
            Ok(counts) => (counts, false),
            Err(counts) => (counts, true),
        };
        if retryable {
            tracing::warn!(
                error_code = "notification_push_retry",
                retryable = counts.retryable,
                "Push subscription delivery will retry"
            );
        }
        Ok(DeliveryReceipt {
            invalid_endpoints: counts.invalid_endpoints,
            retryable,
        })
    }
}

#[derive(serde::Serialize)]
struct PushPayload<'a> {
    id: Uuid,
    title: &'a str,
    body: &'a str,
    link: Option<&'a str>,
}
#[derive(Default, Debug, PartialEq)]
struct SubscriptionCounts {
    delivered: usize,
    removed: usize,
    retryable: usize,
    invalid_endpoints: Vec<String>,
}
enum EndpointDelivery {
    Delivered,
    Removed(String),
}
fn rotate_for_attempt<T>(items: &mut [T], attempts: i32) {
    if !items.is_empty() {
        let wave = usize::try_from(attempts.saturating_sub(1)).unwrap_or(0);
        let offset = wave.saturating_mul(4) % items.len();
        items.rotate_left(offset);
    }
}

async fn attempt_subscriptions<T, F, Fut>(
    items: Vec<T>,
    deliver: F,
) -> Result<SubscriptionCounts, SubscriptionCounts>
where
    F: Fn(T) -> Fut,
    Fut: std::future::Future<Output = Result<EndpointDelivery, ()>>,
{
    // Four item futures each poll at most four endpoints: at most 16 push requests.
    // A stalled first endpoint times out independently so later devices still run.
    let mut attempts = stream::iter(items)
        .map(|item| tokio::time::timeout(Duration::from_secs(2), deliver(item)))
        .buffer_unordered(4);
    let mut counts = SubscriptionCounts::default();
    while let Some(result) = attempts.next().await {
        match result {
            Ok(Ok(EndpointDelivery::Delivered)) => counts.delivered += 1,
            Ok(Ok(EndpointDelivery::Removed(endpoint))) => {
                counts.removed += 1;
                counts.invalid_endpoints.push(endpoint);
            }
            Ok(Err(())) | Err(_) => counts.retryable += 1,
        }
    }
    if counts.retryable > 0 {
        Err(counts)
    } else {
        Ok(counts)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn push_payload_serializes_notification_identity_and_browser_fields() {
        let id = Uuid::now_v7();
        let value = serde_json::to_value(PushPayload {
            id,
            title: "หัวข้อ",
            body: "ข้อความ",
            link: Some("/staff/work"),
        })
        .unwrap();
        assert_eq!(
            value,
            serde_json::json!({"id":id.to_string(),"title":"หัวข้อ","body":"ข้อความ","link":"/staff/work"})
        );
        let value = serde_json::to_value(PushPayload {
            id,
            title: "หัวข้อ",
            body: "ข้อความ",
            link: None,
        })
        .unwrap();
        assert!(value["link"].is_null());
    }

    #[tokio::test]
    async fn subscription_failure_propagates_after_all_siblings_and_terminal_removal_succeeds() {
        let attempted = std::sync::atomic::AtomicUsize::new(0);
        let result = attempt_subscriptions(vec![0, 1, 2], |item| {
            attempted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            async move {
                match item {
                    0 => Err(()),
                    1 => Ok(EndpointDelivery::Removed("synthetic".into())),
                    _ => Ok(EndpointDelivery::Delivered),
                }
            }
        })
        .await;
        assert_eq!(attempted.load(std::sync::atomic::Ordering::SeqCst), 3);
        assert_eq!(
            result,
            Err(SubscriptionCounts {
                delivered: 1,
                removed: 1,
                retryable: 1,
                invalid_endpoints: vec!["synthetic".into()],
            })
        );
        assert_eq!(
            attempt_subscriptions(vec![()], |_| async {
                Ok(EndpointDelivery::Removed("synthetic".into()))
            })
            .await,
            Ok(SubscriptionCounts {
                delivered: 0,
                removed: 1,
                retryable: 0,
                invalid_endpoints: vec!["synthetic".into()],
            })
        );
    }
    #[tokio::test]
    async fn slow_first_endpoint_does_not_block_later_endpoints_or_outlive_cancellation() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        struct Active(Arc<AtomicUsize>);
        impl Drop for Active {
            fn drop(&mut self) {
                self.0.fetch_sub(1, Ordering::SeqCst);
            }
        }
        let active = Arc::new(AtomicUsize::new(0));
        let delivered = Arc::new(AtomicUsize::new(0));
        let result = tokio::time::timeout(
            Duration::from_millis(100),
            attempt_subscriptions((0..8).collect(), |item| {
                let active = active.clone();
                let delivered = delivered.clone();
                async move {
                    active.fetch_add(1, Ordering::SeqCst);
                    let _active = Active(active);
                    if item == 0 {
                        std::future::pending::<()>().await;
                    }
                    delivered.fetch_add(1, Ordering::SeqCst);
                    Ok(EndpointDelivery::Delivered)
                }
            }),
        )
        .await;
        assert!(result.is_err());
        assert_eq!(delivered.load(Ordering::SeqCst), 7);
        assert_eq!(active.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn retry_rotation_reaches_tail_after_multiple_stalled_endpoint_waves() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let reached_tail = AtomicUsize::new(0);
        for attempt in 1..=3 {
            let mut endpoints: Vec<_> = (0..12).collect();
            rotate_for_attempt(&mut endpoints, attempt);
            let _deadline = tokio::time::timeout(
                Duration::from_millis(30),
                attempt_subscriptions(endpoints, |endpoint| {
                    let reached_tail = &reached_tail;
                    async move {
                        if endpoint < 8 {
                            std::future::pending::<()>().await;
                        }
                        reached_tail.fetch_add(1, Ordering::SeqCst);
                        Ok(EndpointDelivery::Delivered)
                    }
                }),
            )
            .await;
            assert_eq!(
                reached_tail.load(Ordering::SeqCst),
                if attempt < 3 { 0 } else { 4 }
            );
        }
    }

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
