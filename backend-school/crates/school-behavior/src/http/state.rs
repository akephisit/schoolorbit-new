use school_auth::runtime::AuthRuntime;

#[derive(Clone)]
pub struct BehaviorHttpState {
    pub auth_runtime: AuthRuntime,
    pub notification_channel:
        tokio::sync::broadcast::Sender<school_notifications::events::TenantNotificationEvent>,
    pub reset: &'static dyn BehaviorResetPort,
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
