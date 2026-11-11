use school_academic_realtime::AcademicRealtimePort;
use school_auth::runtime::AuthRuntime;
use std::sync::Arc;

#[derive(Clone)]
pub struct AcademicAssessmentHttpState {
    pub auth_runtime: AuthRuntime,
    pub websocket_manager: Arc<dyn AcademicRealtimePort>,
    pub result_locks: &'static dyn school_academic_assessment::ports::ResultLockPort,
}
