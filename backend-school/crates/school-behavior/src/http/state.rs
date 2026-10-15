use school_auth::runtime::AuthRuntime;

#[derive(Clone)]
pub struct BehaviorHttpState {
    pub auth_runtime: AuthRuntime,
    pub notification_channel:
        tokio::sync::broadcast::Sender<school_notifications::events::TenantNotificationEvent>,
    pub reset: &'static dyn BehaviorResetPort,
    pub runtime: &'static dyn BehaviorRuntimePort,
}

/// Refresh cross-domain runtime state only after the authoritative mutation commits.
#[async_trait::async_trait]
pub trait BehaviorRuntimePort: Send + Sync {
    async fn refresh_after_commit(
        &self,
        tenant: &str,
        pool: sqlx::PgPool,
        channel: tokio::sync::broadcast::Sender<
            school_notifications::events::TenantNotificationEvent,
        >,
    );
}

#[async_trait::async_trait]
pub trait BehaviorResetPort: Send + Sync {
    async fn reset_scores(
        &self,
        pool: &sqlx::PgPool,
        actor: uuid::Uuid,
        payload: crate::models::ResetBehaviorScores,
    ) -> Result<crate::models::BehaviorCleanupOutcome, school_errors::AppError>;
}
