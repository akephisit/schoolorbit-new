use school_auth::runtime::AuthRuntime;

#[derive(Clone)]
pub struct AdmissionHttpState {
    pub auth_runtime: AuthRuntime,
    pub placement_rosters: &'static dyn school_academic_core::ports::PlacementRosterPort,
    pub file_authorization: &'static dyn school_file_platform::access::FileAuthorizationPort,
    pub file_platform: Arc<FilePlatform>,
}
use school_file_platform::platform_service::FilePlatform;
use std::sync::Arc;
