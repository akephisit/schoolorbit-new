pub mod handlers;
pub mod http;
pub mod profile_orchestrator;
pub mod session_handlers;

#[cfg(test)]
mod session_http_tests;

#[cfg(test)]
mod profile_integration_tests;

#[cfg(test)]
mod staff_integration_tests;

use axum::extract::FromRef;

use crate::AppState;

impl FromRef<AppState> for school_auth::runtime::AuthRuntime {
    fn from_ref(state: &AppState) -> Self {
        state.auth_runtime.clone()
    }
}
