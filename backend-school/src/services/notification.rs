use crate::modules::notification::handlers::Notification;
use uuid::Uuid;
use tokio::sync::broadcast;

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
    /// This handles database insertion and real-time broadcasting via SSE.
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
        // If no one is listening (error), that's fine, just ignore it.
        let _ = notification_tx.send((user_id, notification));

        Ok(id)
    }
    
    // Helper to send using AppState (if you have the correct pool already)
    // Note: Since we use multi-tenant pools, we usually need the specific school pool, not just AppState.
    // So the direct `send` method above is flexible. This helper is for convenience if logic allows.
}
