use async_trait::async_trait;
use school_auth::{
    models::{UpdateProfileRequest, User},
    runtime::AuthRuntime,
};
use school_errors::AppError;
use sqlx::PgPool;
use std::sync::Arc;
use uuid::Uuid;

/// Application composition owns the profile command spanning Auth and Files.
#[async_trait]
pub trait ProfileUpdatePort: Send + Sync {
    async fn update_profile(
        &self,
        pool: &PgPool,
        user_id: Uuid,
        payload: UpdateProfileRequest,
    ) -> Result<User, AppError>;
}

#[derive(Clone)]
pub struct AuthHttpState {
    pub auth_runtime: AuthRuntime,
    pub profile_updates: Arc<dyn ProfileUpdatePort>,
}

impl axum::extract::FromRef<AuthHttpState> for school_auth::runtime::AuthRuntime {
    fn from_ref(state: &AuthHttpState) -> Self {
        state.auth_runtime.clone()
    }
}
