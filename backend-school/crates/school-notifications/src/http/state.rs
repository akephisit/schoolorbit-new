use school_auth::runtime::AuthRuntime;

#[derive(Clone)]
pub struct NotificationHttpState {
    pub auth_runtime: AuthRuntime,
    pub notification_channel:
        tokio::sync::broadcast::Sender<crate::events::TenantNotificationEvent>,
    pub work_event_channel: tokio::sync::broadcast::Sender<crate::events::WorkChangeEvent>,
}
