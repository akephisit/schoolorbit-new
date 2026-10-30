use school_auth::runtime::AuthRuntime;

#[derive(Clone)]
pub struct AdministrationHttpState {
    pub auth_runtime: AuthRuntime,
    pub file_platform: Arc<FilePlatform>,
    pub facility_changes: Arc<dyn FacilityResourceChangePort>,
}

/// Composes downstream resource invalidation after a committed facility mutation.
/// Administration owns room writes; consumers own their derived state and transport.
pub trait FacilityResourceChangePort: Send + Sync {
    fn room_changed(
        &self,
        tenant_id: uuid::Uuid,
        school_key: &str,
        actor_id: uuid::Uuid,
        room_id: uuid::Uuid,
    );
}
use school_file_platform::platform_service::FilePlatform;
use std::sync::Arc;
