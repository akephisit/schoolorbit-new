use school_auth::runtime::AuthRuntime;
use school_file_platform::platform_service::FilePlatform;
use std::sync::Arc;

#[derive(Clone)]
pub struct QuestionBankHttpState {
    pub auth_runtime: AuthRuntime,
    pub file_authorization: &'static dyn school_file_contracts::access::FileAuthorizationPort,
    pub file_platform: Arc<FilePlatform>,
}
