use school_auth::runtime::AuthRuntime;

#[derive(Clone)]
pub struct QuestionBankHttpState {
    pub auth_runtime: AuthRuntime,
    pub file_authorization: &'static dyn school_file_platform::access::FileAuthorizationPort,
    pub file_platform: Arc<FilePlatform>,
}
use school_file_platform::platform_service::FilePlatform;
use std::sync::Arc;
