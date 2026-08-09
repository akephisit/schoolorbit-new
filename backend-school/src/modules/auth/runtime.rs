use std::sync::Arc;

use axum::extract::FromRef;
use tokio::sync::broadcast;

use crate::{
    db::{admin_client::AdminClient, permission_cache::PermissionCache, pool_manager::PoolManager},
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
}

impl FromRef<AppState> for AuthRuntime {
    fn from_ref(state: &AppState) -> Self {
        state.auth_runtime.clone()
    }
}
