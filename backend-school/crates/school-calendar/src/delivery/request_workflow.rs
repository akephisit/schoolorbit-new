use crate::{
    models::UpsertCalendarEventRequest,
    requests::{self, CalendarEventRequest, CalendarRequestApproval, CreateCalendarRequest},
};
use school_authorization::ActorContext;
use school_errors::AppError;
use sqlx::PgPool;
use uuid::Uuid;
/// Composition adapter only: domain transactions accept inboxes, then wake presentation.
pub struct CalendarRequestWorkflow<'a> {
    pool: &'a PgPool,
    tenant: &'a str,
}
impl<'a> CalendarRequestWorkflow<'a> {
    pub fn new(
        pool: &'a PgPool,
        _channel: &'a tokio::sync::broadcast::Sender<
            school_notifications::events::TenantNotificationEvent,
        >,
        tenant: &'a str,
    ) -> Self {
        Self { pool, tenant }
    }
    pub async fn create(
        &self,
        actor: &ActorContext,
        payload: CreateCalendarRequest,
    ) -> Result<CalendarEventRequest, AppError> {
        let result = requests::create_request(self.pool, actor, payload).await?;
        school_notifications::runtime::wake_after_commit(self.tenant);
        Ok(result)
    }
    pub async fn approve(
        &self,
        actor: &ActorContext,
        id: Uuid,
        payload: UpsertCalendarEventRequest,
    ) -> Result<CalendarRequestApproval, AppError> {
        let result = requests::approve_request(self.pool, actor, id, payload).await?;
        school_notifications::runtime::wake_after_commit(self.tenant);
        Ok(result)
    }
    pub async fn reject(
        &self,
        actor: &ActorContext,
        id: Uuid,
        reason: &str,
    ) -> Result<CalendarEventRequest, AppError> {
        let result = requests::reject_request(self.pool, actor, id, reason).await?;
        school_notifications::runtime::wake_after_commit(self.tenant);
        Ok(result)
    }
}
