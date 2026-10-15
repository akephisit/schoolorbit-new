use school_auth::runtime::AuthRuntime;

#[derive(Clone)]
pub struct AuthHttpState {
    pub auth_runtime: AuthRuntime,
    pub file_platform: Arc<FilePlatform>,
}
use school_file_platform::platform_service::FilePlatform;
use std::sync::Arc;

impl axum::extract::FromRef<AuthHttpState> for school_auth::runtime::AuthRuntime {
    fn from_ref(state: &AuthHttpState) -> Self {
        state.auth_runtime.clone()
    }
}
