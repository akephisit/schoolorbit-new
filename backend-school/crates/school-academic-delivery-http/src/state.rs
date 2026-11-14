use school_academic_realtime::AcademicRealtimePort;
use school_auth::runtime::AuthRuntime;
use std::sync::Arc;

#[derive(Clone)]
pub struct AcademicDeliveryHttpState {
    pub delivery_deletion: &'static dyn school_academic_delivery::ports::DeliveryDeletionPort,
    pub auth_runtime: AuthRuntime,
    pub websocket_manager: Arc<dyn AcademicRealtimePort>,
    pub assessment_current: &'static dyn school_academic_delivery::ports::AssessmentCurrentPort,
}
