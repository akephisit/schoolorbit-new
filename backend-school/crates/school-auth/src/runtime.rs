use std::sync::Arc;

use school_authorization::PermissionCache;
use school_tenancy::{AdminClient, PoolManager, TenantContext};
use tokio::sync::broadcast;

use super::{
    config::SessionConfig,
    events::{PermissionChangeEvent, SessionRevocationEvent},
    session_cache::SessionCache,
    session_service::SessionServiceContext,
};

#[derive(Clone)]
pub struct AuthRuntime {
    pub admin_client: Arc<AdminClient>,
    pub pool_manager: Arc<PoolManager>,
    pub identity_cache: Arc<SessionCache>,
    pub permission_cache: Arc<PermissionCache>,
    pub config: Arc<SessionConfig>,
    pub session_events: broadcast::Sender<SessionRevocationEvent>,
    pub permission_events: broadcast::Sender<PermissionChangeEvent>,
}

impl AuthRuntime {
    pub fn service_context(&self, tenant: TenantContext) -> SessionServiceContext {
        SessionServiceContext::new(
            tenant,
            Arc::clone(&self.permission_cache),
            Arc::clone(&self.identity_cache),
            Arc::clone(&self.config),
            self.session_events.clone(),
        )
    }

    pub fn invalidate_permission_user(&self, tenant: &str, user_id: uuid::Uuid) {
        self.identity_cache
            .invalidate_identity_user(tenant, user_id);
        self.permission_cache.invalidate_user(tenant, user_id);
    }

    pub fn invalidate_permission_tenant(&self, tenant: &str) {
        self.identity_cache.invalidate_identity_tenant(tenant);
        self.permission_cache.invalidate_tenant(tenant);
    }

    pub fn notify_all_permissions_changed(&self, tenant: &str) {
        let _ = self
            .permission_events
            .send(PermissionChangeEvent::for_all_users(tenant));
    }
}
