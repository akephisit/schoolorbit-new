use school_academic_realtime::AcademicRealtimePort;
use school_auth::runtime::AuthRuntime;
use std::sync::Arc;

#[derive(Clone)]
pub struct AcademicDeliveryHttpState {
    pub auth_runtime: AuthRuntime,
    pub websocket_manager: Arc<dyn AcademicRealtimePort>,
    pub assessment_publication:
        &'static dyn school_academic_delivery::ports::AssessmentPublicationPort,
    pub publication_history: &'static dyn school_academic_delivery::ports::PublicationHistoryPort,
}
