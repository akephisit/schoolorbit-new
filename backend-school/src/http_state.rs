use crate::{
    modules::academic::{
        lifecycle_provider_adapter::LIFECYCLE_PROVIDERS, result_lock_adapter::RESULT_LOCKS,
        websockets::WebSocketManager,
    },
    AppState,
};
use async_trait::async_trait;
use axum::extract::FromRef;
use school_academic_http::{
    realtime::{AcademicRealtimePort, TimetableEvent},
    state::AcademicHttpState,
};
use school_certificates_http::state::{CertificateFileDeletionPort, CertificateHttpState};
use school_errors::AppError;
use school_file_platform::platform_service::FilePlatform;
use sqlx::PgPool;
use uuid::Uuid;

static PLACEMENT_ROSTERS: school_academic_delivery::services::roster_tracking::RoomRosterTracking =
    school_academic_delivery::services::roster_tracking::RoomRosterTracking;

impl FromRef<AppState> for AcademicHttpState {
    fn from_ref(state: &AppState) -> Self {
        Self {
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
        crate::modules::files::consumer_service::request_deletions(platform, pool, file_ids).await
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

impl AcademicRealtimePort for WebSocketManager {
    fn broadcast_mutation(
        &self,
        school_key: String,
        academic_term_id: Uuid,
        event: TimetableEvent,
    ) -> u64 {
        WebSocketManager::broadcast_mutation(self, school_key, academic_term_id, event)
    }
    fn broadcast_academic_core_changed(
        &self,
        school_key: String,
        user_id: Uuid,
        entity_type: &str,
        entity_id: Option<Uuid>,
        academic_year_id: Option<Uuid>,
        academic_term_id: Option<Uuid>,
    ) {
        WebSocketManager::broadcast_academic_core_changed(
            self,
            school_key,
            user_id,
            entity_type,
            entity_id,
            academic_year_id,
            academic_term_id,
        )
    }
    fn broadcast_learning_delivery_changed(
        &self,
        school_key: String,
        user_id: Uuid,
        academic_term_id: Uuid,
        learning_offering_id: Uuid,
        learning_group_id: Option<Uuid>,
        revision: i64,
    ) {
        WebSocketManager::broadcast_learning_delivery_changed(
            self,
            school_key,
            user_id,
            academic_term_id,
            learning_offering_id,
            learning_group_id,
            revision,
        )
    }
}
