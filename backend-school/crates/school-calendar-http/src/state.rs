use school_auth::runtime::AuthRuntime;

#[derive(Clone)]
pub struct CalendarHttpState {
    pub auth_runtime: AuthRuntime,
    pub notification_channel:
        tokio::sync::broadcast::Sender<school_notifications::events::TenantNotificationEvent>,
}
