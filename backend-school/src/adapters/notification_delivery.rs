//! Compose structural source guards; notification recipients are immutable snapshots.
use school_errors::AppError;
use school_notifications::dispatch::{DeliveryEligibilityPort, NotificationSource};
use sqlx::PgConnection;
use uuid::Uuid;

pub struct ApplicationNotificationEligibility;
pub static APPLICATION_NOTIFICATION_ELIGIBILITY: ApplicationNotificationEligibility =
    ApplicationNotificationEligibility;

#[async_trait::async_trait]
impl DeliveryEligibilityPort for ApplicationNotificationEligibility {
    async fn authorize_on_connection(
        &self,
        connection: &mut PgConnection,
        source: &NotificationSource,
        recipient_id: Uuid,
    ) -> Result<bool, AppError> {
        match source {
            NotificationSource::Direct => Ok(true),
            NotificationSource::Attendance { source_id } => {
                school_attendance::services::notifications::authorize_delivery_on_connection(
                    connection,
                    *source_id,
                    recipient_id,
                )
                .await
            }
        }
    }
}
