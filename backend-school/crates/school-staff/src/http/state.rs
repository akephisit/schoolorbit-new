use school_auth::runtime::AuthRuntime;

#[derive(Clone)]
pub struct StaffHttpState {
    pub auth_runtime: AuthRuntime,
    pub file_platform: Arc<FilePlatform>,
}
use school_file_platform::platform_service::FilePlatform;
use std::sync::Arc;
