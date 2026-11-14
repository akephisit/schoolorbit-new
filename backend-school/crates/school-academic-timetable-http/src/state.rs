use school_academic_realtime::AcademicRealtimePort;
use school_auth::runtime::AuthRuntime;
use std::sync::Arc;

#[derive(Clone)]
pub struct AcademicTimetableHttpState {
    pub auth_runtime: AuthRuntime,
    pub websocket_manager: Arc<dyn AcademicRealtimePort>,
    pub collaboration: Arc<school_academic_timetable::collaboration::TimetableCollaborationRuntime>,
    pub staff_directory: &'static dyn school_academic_timetable::ports::StaffDirectoryPort,
    pub lesson_source_detachment:
        &'static dyn school_academic_timetable::ports::LessonSourceDetachmentPort,
}

#[derive(Clone)]
pub struct AcademicWebSocketState {
    pub collaboration: Arc<school_academic_timetable::collaboration::TimetableCollaborationRuntime>,
    pub staff_directory: &'static dyn school_academic_timetable::ports::StaffDirectoryPort,
    pub auth_runtime: AuthRuntime,
    pub websocket_manager: Arc<crate::websockets::WebSocketManager>,
}
