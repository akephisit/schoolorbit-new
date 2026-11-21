use school_auth::runtime::AuthRuntime;

#[derive(Clone)]
pub struct AttendanceHttpState {
    pub auth_runtime: AuthRuntime,
    pub notification_channel:
        tokio::sync::broadcast::Sender<school_notifications::events::TenantNotificationEvent>,
    pub file_platform: Arc<FilePlatform>,
}
use school_file_platform::platform_service::FilePlatform;
use std::sync::Arc;
