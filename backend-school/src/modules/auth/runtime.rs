use std::sync::Arc;

use axum::extract::FromRef;
use tokio::sync::broadcast;

use crate::{
    db::{admin_client::AdminClient, permission_cache::PermissionCache, pool_manager::PoolManager},
    modules::notification::events::PermissionChangeEvent,
    utils::tenant::TenantContext,
    AppState,
};

use super::{
    config::SessionConfig, events::SessionRevocationEvent, session_service::SessionServiceContext,
};

#[derive(Clone)]
pub struct AuthRuntime {
    pub admin_client: Arc<AdminClient>,
    pub pool_manager: Arc<PoolManager>,
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
            Arc::clone(&self.config),
            self.session_events.clone(),
        )
    }

    pub fn notify_all_permissions_changed(&self, tenant: &str) {
        let _ = self
            .permission_events
            .send(PermissionChangeEvent::for_all_users(tenant));
    }
}

impl FromRef<AppState> for AuthRuntime {
    fn from_ref(state: &AppState) -> Self {
        state.auth_runtime.clone()
    }
}
