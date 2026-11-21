use school_auth::runtime::AuthRuntime;
use school_notifications::events::{TenantNotificationEvent, WorkChangeEvent};
use tokio::sync::broadcast;

#[derive(Clone)]
pub struct NotificationHttpState {
    pub auth_runtime: AuthRuntime,
    pub notification_channel: broadcast::Sender<TenantNotificationEvent>,
    pub work_event_channel: broadcast::Sender<WorkChangeEvent>,
}
