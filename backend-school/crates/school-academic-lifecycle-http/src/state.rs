use school_academic_realtime::AcademicRealtimePort;
use school_auth::runtime::AuthRuntime;
use std::sync::Arc;

#[derive(Clone)]
pub struct AcademicLifecycleHttpState {
    pub auth_runtime: AuthRuntime,
    pub websocket_manager: Arc<dyn AcademicRealtimePort>,
    pub lifecycle_providers: Arc<dyn school_academic_lifecycle::ports::ExternalLifecyclePort>,
    pub retention:
        &'static dyn school_academic_lifecycle::services::retention::AcademicRetentionPort,
}
