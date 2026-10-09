use crate::realtime::AcademicRealtimePort;
use school_academic_assessment::ports::ResultLockPort;
use school_academic_lifecycle::ports::ExternalLifecyclePort;
use school_auth::runtime::AuthRuntime;
use std::sync::Arc;

#[derive(Clone)]
pub struct AcademicHttpState {
    pub placement_rosters: &'static dyn school_academic_core::ports::PlacementRosterPort,
    pub auth_runtime: AuthRuntime,
    pub websocket_manager: Arc<dyn AcademicRealtimePort>,
    pub result_locks: &'static dyn ResultLockPort,
    pub lifecycle_providers: &'static dyn ExternalLifecyclePort,
}
