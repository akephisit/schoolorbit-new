use crate::modules::notification::handlers::Notification;
use uuid::Uuid;
use tokio::sync::broadcast;
use web_push::*;
use std::env;

pub struct NotificationService;

#[derive(Debug)]
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
    /// Send a notification to a specific user.
    /// This handles database insertion, real-time broadcasting via SSE, AND Web Push.
    pub async fn send(
        pool: &sqlx::PgPool,
        notification_tx: &broadcast::Sender<(Uuid, Notification)>,
        user_id: Uuid,
        title: &str,
        message: &str,
        type_: NotificationType,
        link: Option<&str>,
    ) -> Result<Uuid, sqlx::Error> {
        let notification = sqlx::query_as::<_, Notification>(
            r#"
            INSERT INTO notifications (user_id, title, message, type, link)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING id, title, message, type AS type_, link, read_at, created_at
            "#
        )
        .bind(user_id)
        .bind(title)
        .bind(message)
        .bind(type_.as_str())
        .bind(link)
        .fetch_one(pool)
        .await?;
        
        let id = notification.id;

        // Broadcast to SSE (Real-time)
        let _ = notification_tx.send((user_id, notification));

        // 🚀 Trigger Web Push (Fire-and-forget task)
        let pool_clone = pool.clone();
        let title = title.to_string();
        let message = message.to_string();
        let link = link.map(|s| s.to_string());
        
        tokio::spawn(async move {
            if let Err(e) = Self::send_web_push(&pool_clone, user_id, &title, &message, link.as_deref()).await {
                tracing::error!("Web Push Failed for user {}: {}", user_id, e);
            }
        });

        Ok(id)
    }

    /// Internal helper to send Web Push
    async fn send_web_push(
        pool: &sqlx::PgPool,
        user_id: Uuid,
        title: &str,
        message: &str,
        link: Option<&str>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // 1. Get VAPID config from .env
        let vapid_public = env::var("VAPID_PUBLIC_KEY")?;
        let vapid_private = env::var("VAPID_PRIVATE_KEY")?;
        let vapid_subject = env::var("VAPID_SUBJECT").unwrap_or_else(|_| "mailto:admin@localhost".to_string());

        // 3. Get user subscriptions
        #[derive(sqlx::FromRow)]
        struct SubRow {
            endpoint: String,
            p256dh_key: String,
            auth_key: String,
        }
        
        let subs = sqlx::query_as::<_, SubRow>(
            "SELECT endpoint, p256dh_key, auth_key FROM push_subscriptions WHERE user_id = $1"
        )
        .bind(user_id)
        .fetch_all(pool)
        .await?;

        if subs.is_empty() {
            return Ok(());
        }

        // 4. Construct payload
        let payload = serde_json::json!({
            "title": title,
            "body": message,
            "link": link,
            // Add icon if needed
        }).to_string();

        // 5. Send to each subscription
        let client = IsahcWebPushClient::new()?; // Or async-std/tokio client depending on features
        
        // Note: web-push 0.10+ uses different API. Assuming 0.11
        // Let's use simple loop
        for sub in subs {
            let subscription_info = SubscriptionInfo {
                endpoint: sub.endpoint.clone(),
                keys: SubscriptionKeys {
                    p256dh: sub.p256dh_key,
                    auth: sub.auth_key,
                },
            };

            let mut builder = WebPushMessageBuilder::new(&subscription_info);
            builder.set_payload(ContentEncoding::Aes128Gcm, payload.as_bytes());
            
            // Sign with VAPID
            let mut sig_builder = VapidSignatureBuilder::from_base64(&vapid_private, &subscription_info)?;
            sig_builder.add_claim("sub", vapid_subject.clone());
            let signature = sig_builder.build()?;
            builder.set_vapid_signature(signature);

            match client.send(builder.build()?).await {
                Ok(_) => {
                    tracing::info!("Push sent to device");
                },
                Err(e) => {
                    tracing::warn!("Push failed for endpoint {}: {:?}", sub.endpoint, e);
                    
                    let should_delete = matches!(e, WebPushError::EndpointNotValid(_) | WebPushError::EndpointNotFound(_));

                    if should_delete {
                        tracing::info!("Removing invalid subscription: {}", sub.endpoint);
                        let _ = sqlx::query("DELETE FROM push_subscriptions WHERE endpoint = $1")
                            .bind(&sub.endpoint)
                            .execute(pool)
                            .await;
                    }
                }
            }
        }

        Ok(())
    }
}
