use crate::{devices, events::TenantNotificationEvent, models::Notification};
use chrono::{DateTime, Utc};
use futures::{stream, StreamExt};
use std::time::Duration;
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
    pub(crate) fn publish(&self, user: Uuid, notification: Notification) {
        drop(self.notification_tx.send(TenantNotificationEvent::new(
            self.tenant,
            user,
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
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }
}
#[derive(sqlx::FromRow)]
struct PushSubscription {
    id: Uuid,
    version: Uuid,
    binding_token: Uuid,
    endpoint: String,
    p256dh_key: String,
    auth_key: String,
}
/// Owned transport inputs cannot retain a pool, transaction or database admission permit.
pub(crate) struct PreparedNotification {
    pub(crate) notification: Notification,
    expires_at: DateTime<Utc>,
    subscriptions: Vec<PushSubscription>,
}
#[derive(Default)]
pub(crate) struct DeliveryReceipt {
    pub(crate) invalid_devices: Vec<(Uuid, Uuid)>,
}
impl NotificationService {
    pub(crate) async fn prepare_stored(
        pool: &sqlx::PgPool,
        user: Uuid,
        notification: Notification,
        expires_at: DateTime<Utc>,
    ) -> Result<PreparedNotification, &'static str> {
        let mut subscriptions = Vec::new();
        if let Some(config) = devices::vapid_config() {
            let mut c = school_tenancy::TenantPool::observed_checkout_pool(
                pool,
                "notifications.prepare_stored",
            )
            .await
            .map_err(|_| "notification_database_unavailable")?;
            subscriptions=school_database::DbSession::new(&mut *c).query_as("notifications.presentation.devices",r#"SELECT p.id,p.version,p.binding_token,p.endpoint,p.p256dh_key,p.auth_key FROM push_subscriptions p
                JOIN auth_sessions s ON s.id=p.session_id AND s.user_id=p.user_id JOIN users u ON u.id=s.user_id
                WHERE p.user_id=$1 AND p.key_id=$2 AND p.expires_at>now() AND s.revoked_at IS NULL
                AND s.idle_expires_at>now() AND s.absolute_expires_at>now() AND u.status='active' ORDER BY p.last_confirmed_at DESC,p.id LIMIT 20"#)
                .bind(user).bind(config.key_id).fetch_all().await.map_err(|_|"notification_subscription_unavailable")?;
        }
        Ok(PreparedNotification {
            notification,
            expires_at,
            subscriptions,
        })
    }
    pub(crate) async fn deliver_prepared(
        _publisher: &TenantNotificationPublisher<'_>,
        _user: Uuid,
        prepared: PreparedNotification,
    ) -> Result<DeliveryReceipt, &'static str> {
        // SSE was already attempted independently before subscription lookup.
        Ok(Self::send_web_push(prepared).await)
    }
    pub(crate) async fn complete_delivery(
        pool: &sqlx::PgPool,
        user: Uuid,
        receipt: DeliveryReceipt,
    ) -> Result<(), &'static str> {
        if receipt.invalid_devices.is_empty() {
            return Ok(());
        }
        let mut c = school_tenancy::TenantPool::observed_checkout_pool(
            pool,
            "notifications.complete_delivery",
        )
        .await
        .map_err(|_| "notification_database_unavailable")?;
        // Exact registration-version cleanup cannot revoke an updated account/key binding.
        for (id, version) in receipt.invalid_devices {
            school_database::DbSession::new(&mut *c)
                .query(
                    "notifications.presentation.retire_invalid_device",
                    "DELETE FROM push_subscriptions WHERE id=$1 AND user_id=$2 AND version=$3",
                )
                .bind(id)
                .bind(user)
                .bind(version)
                .execute()
                .await
                .map_err(|_| "notification_subscription_cleanup_unavailable")?;
        }
        Ok(())
    }
    async fn send_web_push(prepared: PreparedNotification) -> DeliveryReceipt {
        let PreparedNotification {
            notification,
            expires_at,
            subscriptions,
        } = prepared;
        if subscriptions.is_empty() {
            return DeliveryReceipt::default();
        }
        let Some(config) = devices::vapid_config() else {
            tracing::warn!(
                error_code = "notification_push_configuration_unavailable",
                "Push unavailable; consumed presentation will not replay"
            );
            return DeliveryReceipt::default();
        };
        let Ok(client) = reqwest::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(2))
            .connect_timeout(Duration::from_secs(1))
            .no_proxy()
            .build()
        else {
            tracing::warn!(
                error_code = "notification_push_client_unavailable",
                "Push unavailable"
            );
            return DeliveryReceipt::default();
        };
        let results = attempt_subscriptions(subscriptions, |sub| {
            let client = &client;
            let notification = &notification;
            let config = &config;
            async move {
                let remaining = (expires_at - Utc::now()).num_seconds();
                if remaining <= 0 {
                    return EndpointResult::Skipped;
                }
                // Revalidate persisted endpoints before any external I/O, including old data.
                if !devices::valid_endpoint(&sub.endpoint) {
                    return EndpointResult::Skipped;
                }
                let info = SubscriptionInfo {
                    endpoint: sub.endpoint,
                    keys: SubscriptionKeys {
                        p256dh: sub.p256dh_key,
                        auth: sub.auth_key,
                    },
                };
                let payload = PushPayload {
                    id: notification.id,
                    title: "SchoolOrbit",
                    body: "มีการแจ้งเตือนใหม่ กรุณาเปิดแอปเพื่อดูรายละเอียด",
                    link: notification
                        .link
                        .as_deref()
                        .filter(|p| crate::dispatch::valid_app_link(p))
                        .unwrap_or("/"),
                    device_id: sub.id,
                    device_version: sub.version,
                    binding_token: sub.binding_token,
                    expires_at,
                };
                let Ok(payload) = serde_json::to_vec(&payload) else {
                    return EndpointResult::MessageDefect;
                };
                let result = async {
                    let mut builder = WebPushMessageBuilder::new(&info);
                    builder.set_ttl(u32::try_from(remaining).unwrap_or(0));
                    builder.set_payload(ContentEncoding::Aes128Gcm, &payload);
                    let mut signature =
                        VapidSignatureBuilder::from_base64(&config.private_key, &info)?;
                    signature.add_claim("sub", config.subject.clone());
                    builder.set_vapid_signature(signature.build()?);
                    send_message(client, builder.build()?).await
                }
                .await;
                match result {
                    Ok(()) => EndpointResult::Attempted,
                    Err(error) => classify_push_error(&error, sub.id, sub.version),
                }
            }
        })
        .await;
        let mut receipt = DeliveryReceipt::default();
        let mut failed = 0usize;
        let mut config_errors = 0usize;
        for result in results {
            match result {
                EndpointResult::InvalidDevice(id, version) => {
                    receipt.invalid_devices.push((id, version))
                }
                EndpointResult::Configuration => config_errors += 1,
                EndpointResult::Failed | EndpointResult::MessageDefect => failed += 1,
                _ => {}
            }
        }
        if failed + config_errors > 0 {
            tracing::warn!(
                error_code = "notification_push_attempt_failed",
                failed,
                configuration_failures = config_errors,
                "Push attempts ended without retry; devices retained"
            );
        }
        receipt
    }
}
/// The library's default client traces whole requests and has no native timeout.
/// Keep its protocol/encryption builder, with a redacted, redirect-free bounded HTTP adapter.
async fn send_message(
    client: &reqwest::Client,
    message: WebPushMessage,
) -> Result<(), WebPushError> {
    let request = web_push::request_builder::build_request::<reqwest::Body>(message);
    let (parts, body) = request.into_parts();
    let mut builder = client.post(parts.uri.to_string()).body(body);
    for (name, value) in &parts.headers {
        let mut header = reqwest::header::HeaderValue::from_bytes(value.as_bytes())
            .map_err(|_| WebPushError::Unspecified)?;
        header.set_sensitive(true);
        builder = builder.header(name.as_str(), header);
    }
    let response = builder
        .send()
        .await
        .map_err(|_| WebPushError::Unspecified)?;
    // Status alone determines lifecycle policy; provider bodies never enter logs or storage.
    let status = response
        .status()
        .as_u16()
        .try_into()
        .map_err(|_| WebPushError::InvalidResponse)?;
    web_push::request_builder::parse_response(status, Vec::new())
}
#[derive(serde::Serialize)]
struct PushPayload<'a> {
    id: Uuid,
    title: &'a str,
    body: &'a str,
    link: &'a str,
    device_id: Uuid,
    device_version: Uuid,
    binding_token: Uuid,
    expires_at: DateTime<Utc>,
}
#[derive(Debug, PartialEq, Eq)]
enum EndpointResult {
    Attempted,
    InvalidDevice(Uuid, Uuid),
    Configuration,
    MessageDefect,
    Failed,
    Skipped,
}
fn classify_push_error(error: &WebPushError, id: Uuid, version: Uuid) -> EndpointResult {
    match error {
        WebPushError::EndpointNotValid(_) | WebPushError::EndpointNotFound(_) => {
            EndpointResult::InvalidDevice(id, version)
        }
        WebPushError::Unauthorized(_) | WebPushError::InvalidClaims => {
            EndpointResult::Configuration
        }
        WebPushError::Other(info) if info.code == 401 || info.code == 403 => {
            EndpointResult::Configuration
        }
        WebPushError::PayloadTooLarge | WebPushError::InvalidTtl | WebPushError::InvalidTopic => {
            EndpointResult::MessageDefect
        }
        // A generic 400 is not proof that a device is bad. Preserve it for future messages.
        _ => EndpointResult::Failed,
    }
}
async fn attempt_subscriptions<T, F, Fut>(items: Vec<T>, deliver: F) -> Vec<EndpointResult>
where
    F: Fn(T) -> Fut,
    Fut: std::future::Future<Output = EndpointResult>,
{
    stream::iter(items)
        .map(|item| async {
            tokio::time::timeout(Duration::from_secs(2), deliver(item))
                .await
                .unwrap_or(EndpointResult::Failed)
        })
        .buffer_unordered(4)
        .collect()
        .await
}
#[cfg(test)]
mod tests {
    use super::*;
    macro_rules! info { ($code:expr) => {serde_json::from_value(serde_json::json!({"code":$code,"errno":0,"error":"synthetic","message":"synthetic"})).unwrap()}; }
    #[test]
    fn only_gone_or_missing_devices_are_retired() {
        let id = Uuid::new_v4();
        let version = Uuid::new_v4();
        for e in [
            WebPushError::Unauthorized(info!(401)),
            WebPushError::Other(info!(403)),
            WebPushError::BadRequest(info!(400)),
            WebPushError::PayloadTooLarge,
            WebPushError::Other(info!(429)),
            WebPushError::ServerError {
                retry_after: None,
                info: info!(500),
            },
        ] {
            assert!(!matches!(
                classify_push_error(&e, id, version),
                EndpointResult::InvalidDevice(..)
            ))
        }
        assert_eq!(
            classify_push_error(&WebPushError::EndpointNotValid(info!(410)), id, version),
            EndpointResult::InvalidDevice(id, version)
        );
    }
    #[tokio::test]
    async fn later_devices_run_after_multiple_stalled_waves_without_whole_message_deadline() {
        let visited = std::sync::atomic::AtomicUsize::new(0);
        let results = attempt_subscriptions((0..12).collect(), |item| {
            let visited = &visited;
            async move {
                if item < 8 {
                    std::future::pending::<()>().await;
                }
                visited.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                EndpointResult::Attempted
            }
        })
        .await;
        assert_eq!(visited.load(std::sync::atomic::Ordering::SeqCst), 4);
        assert_eq!(results.len(), 12);
    }
    #[test]
    fn tenant_publisher_retains_tenant_and_recipient_scope() {
        let (tx, mut rx) = broadcast::channel(1);
        let user = Uuid::new_v4();
        TenantNotificationPublisher::new("a", &tx).publish(
            user,
            Notification {
                id: Uuid::new_v4(),
                title: "x".into(),
                message: "y".into(),
                type_: "info".into(),
                link: None,
                read_at: None,
                created_at: Utc::now(),
            },
        );
        let event = rx.try_recv().unwrap();
        assert!(event.applies_to("a", user));
        assert!(!event.applies_to("b", user));
    }
}
