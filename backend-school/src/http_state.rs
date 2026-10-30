use crate::{
    adapters::{academic_lifecycle::LIFECYCLE_PROVIDERS, academic_result_locks::RESULT_LOCKS},
    AppState,
};
use async_trait::async_trait;
use axum::extract::FromRef;
use school_academic_http::state::AcademicHttpState;
use school_certificates_http::state::{CertificateFileDeletionPort, CertificateHttpState};
use school_errors::AppError;
use school_file_platform::platform_service::FilePlatform;
use sqlx::PgPool;
use uuid::Uuid;

pub(crate) static PLACEMENT_ROSTERS:
    school_academic_delivery::services::roster_tracking::RoomRosterTracking =
    school_academic_delivery::services::roster_tracking::RoomRosterTracking;

impl FromRef<AppState> for AcademicHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            collaboration: state.websocket_manager.collaboration(),
            staff_directory: &crate::adapters::staff_directory::STAFF_DIRECTORY,
            auth_runtime: state.auth_runtime.clone(),
            websocket_manager: state.websocket_manager.clone(),
            result_locks: &RESULT_LOCKS,
            placement_rosters: &PLACEMENT_ROSTERS,
            lifecycle_providers: &LIFECYCLE_PROVIDERS,
        }
    }
}

struct ApplicationCertificateFileDeletions;
static CERTIFICATE_FILE_DELETIONS: ApplicationCertificateFileDeletions =
    ApplicationCertificateFileDeletions;

#[async_trait]
impl CertificateFileDeletionPort for ApplicationCertificateFileDeletions {
    async fn request_deletions(
        &self,
        platform: &FilePlatform,
        pool: &PgPool,
        file_ids: Vec<Uuid>,
    ) -> Result<(), AppError> {
        school_file_platform::consumer_service::request_deletions(platform, pool, file_ids).await
    }
}

impl FromRef<AppState> for CertificateHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            auth_runtime: state.auth_runtime.clone(),
            file_platform: state.file_platform.clone(),
            certificate_verification_limiter: state.certificate_verification_limiter.clone(),
            file_deletions: &CERTIFICATE_FILE_DELETIONS,
        }
    }
}

impl FromRef<AppState> for school_students::http::state::StudentHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            placement_rosters: &PLACEMENT_ROSTERS,
            auth_runtime: state.auth_runtime.clone(),
        }
    }
}

impl FromRef<AppState> for school_fonts::http::state::FontHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            auth_runtime: state.auth_runtime.clone(),
            file_platform: state.file_platform.clone(),
        }
    }
}

impl FromRef<AppState> for school_staff::http::state::StaffHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            auth_runtime: state.auth_runtime.clone(),
            file_platform: state.file_platform.clone(),
        }
    }
}

impl FromRef<AppState> for school_supervision::http::state::SupervisionHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            auth_runtime: state.auth_runtime.clone(),
        }
    }
}

impl FromRef<AppState> for school_navigation::http::state::NavigationHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            auth_runtime: state.auth_runtime.clone(),
        }
    }
}

impl FromRef<AppState> for school_notifications::http::state::NotificationHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            auth_runtime: state.auth_runtime.clone(),
            notification_channel: state.notification_channel.clone(),
            work_event_channel: state.work_event_channel.clone(),
        }
    }
}

impl FromRef<AppState> for school_workflow::http::state::WorkflowHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            file_platform: state.file_platform.clone(),
            auth_runtime: state.auth_runtime.clone(),
            work_event_channel: state.work_event_channel.clone(),
        }
    }
}

impl FromRef<AppState> for school_calendar::http::state::CalendarHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            auth_runtime: state.auth_runtime.clone(),
            notification_channel: state.notification_channel.clone(),
        }
    }
}

impl FromRef<AppState> for school_auth_http::http::state::AuthHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            auth_runtime: state.auth_runtime.clone(),
            file_platform: state.file_platform.clone(),
        }
    }
}

impl FromRef<AppState> for school_administration::http::state::AdministrationHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            auth_runtime: state.auth_runtime.clone(),
            file_platform: state.file_platform.clone(),
        }
    }
}

impl FromRef<AppState> for school_attendance::http::state::AttendanceHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            auth_runtime: state.auth_runtime.clone(),
            notification_channel: state.notification_channel.clone(),
            file_platform: state.file_platform.clone(),
        }
    }
}

impl FromRef<AppState> for school_behavior::http::state::BehaviorHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            runtime: &crate::adapters::behavior::APPLICATION_BEHAVIOR,
            auth_runtime: state.auth_runtime.clone(),
            notification_channel: state.notification_channel.clone(),
            reset: &crate::adapters::behavior::APPLICATION_BEHAVIOR,
        }
    }
}

impl FromRef<AppState> for school_admission::http::state::AdmissionHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            placement_rosters: &PLACEMENT_ROSTERS,
            auth_runtime: state.auth_runtime.clone(),
            file_authorization: &school_files_http::FILE_AUTHORIZATION,
            file_platform: state.file_platform.clone(),
        }
    }
}

impl FromRef<AppState> for school_question_bank::http::state::QuestionBankHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            auth_runtime: state.auth_runtime.clone(),
            file_authorization: &school_files_http::FILE_AUTHORIZATION,
            file_platform: state.file_platform.clone(),
        }
    }
}

impl FromRef<AppState> for school_files_http::http::state::FileHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            auth_runtime: state.auth_runtime.clone(),
            file_platform: state.file_platform.clone(),
        }
    }
}

impl FromRef<AppState> for school_academic_http::state::AcademicWebSocketState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            collaboration: state.websocket_manager.collaboration(),
            staff_directory: &crate::adapters::staff_directory::STAFF_DIRECTORY,
            auth_runtime: state.auth_runtime.clone(),
            websocket_manager: state.websocket_manager.clone(),
        }
    }
}

impl FromRef<AppState> for school_auth::runtime::AuthRuntime {
    fn from_ref(state: &AppState) -> Self {
        state.auth_runtime.clone()
    }
}

impl FromRef<AppState> for school_student_care::http::state::StudentCareHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            auth_runtime: state.auth_runtime.clone(),
        }
    }
}

impl FromRef<AppState> for school_staff::leave::http::LeaveHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            auth_runtime: state.auth_runtime.clone(),
            work_event_channel: state.work_event_channel.clone(),
        }
    }
}

impl FromRef<AppState> for school_budget::http::BudgetHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
            auth_runtime: state.auth_runtime.clone(),
            catalog: &crate::adapters::budget::BUDGET_CATALOG,
            work_event_channel: state.work_event_channel.clone(),
        }
    }
}
