use school_authorization::{staff_users_with_all_permissions, ActorContext};
use school_calendar::{
    models::UpsertCalendarEventRequest,
    requests::{
        self, CalendarEventRequest, CalendarRequestApproval, CalendarRequestStatus,
        CreateCalendarRequest,
    },
};
use school_errors::AppError;
use school_permissions::registry::codes;
use sqlx::PgPool;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::{
    modules::notification::events::TenantNotificationEvent,
    services::notification::{NotificationService, NotificationType, TenantNotificationPublisher},
};

/// Application adapter: domain mutations commit first; delivery failures must not
/// turn a committed request/decision into a retryable client mutation error.
pub struct CalendarRequestWorkflow<'a> {
    pool: &'a PgPool,
    channel: &'a broadcast::Sender<TenantNotificationEvent>,
    tenant: &'a str,
}

impl<'a> CalendarRequestWorkflow<'a> {
    pub fn new(
        pool: &'a PgPool,
        channel: &'a broadcast::Sender<TenantNotificationEvent>,
        tenant: &'a str,
    ) -> Self {
        Self {
            pool,
            channel,
            tenant,
        }
    }

    pub async fn create(
        &self,
        actor: &ActorContext,
        payload: CreateCalendarRequest,
    ) -> Result<CalendarEventRequest, AppError> {
        let request = requests::create_request(self.pool, actor, payload).await?;
        self.notify(&request).await;
        Ok(request)
    }

    pub async fn approve(
        &self,
        actor: &ActorContext,
        id: Uuid,
        payload: UpsertCalendarEventRequest,
    ) -> Result<CalendarRequestApproval, AppError> {
        let outcome = requests::approve_request(self.pool, actor, id, payload).await?;
        self.notify(&outcome.request).await;
        Ok(outcome)
    }

    pub async fn reject(
        &self,
        actor: &ActorContext,
        id: Uuid,
        reason: &str,
    ) -> Result<CalendarEventRequest, AppError> {
        let request = requests::reject_request(self.pool, actor, id, reason).await?;
        self.notify(&request).await;
        Ok(request)
    }

    async fn notify(&self, request: &CalendarEventRequest) {
        match self.send_notifications(request).await {
            Ok(failed) if failed > 0 => {
                tracing::error!(request_id=%request.id, failed_recipient_count=failed, "Calendar request notification delivery incomplete")
            }
            Err(_) => {
                tracing::error!(request_id=%request.id, "Calendar request notification recipient lookup failed after commit")
            }
            Ok(_) => {}
        }
    }

    async fn send_notifications(&self, request: &CalendarEventRequest) -> Result<usize, AppError> {
        let recipients = match request.status {
            CalendarRequestStatus::Pending => {
                staff_users_with_all_permissions(
                    self.pool,
                    &[codes::CALENDAR_READ_SCHOOL, codes::CALENDAR_MANAGE_SCHOOL],
                )
                .await?
            }
            CalendarRequestStatus::Approved | CalendarRequestStatus::Rejected => {
                sqlx::query_scalar::<_, Uuid>(
                    "SELECT id FROM users WHERE id=$1 AND status='active' AND user_type='staff'",
                )
                .bind(request.requested_by)
                .fetch_all(self.pool)
                .await?
            }
        };
        let (title, message, link) = notification_content(request);
        let publisher = TenantNotificationPublisher::new(self.tenant, self.channel);
        let mut failed = 0;
        for recipient in recipients {
            if NotificationService::send(
                self.pool,
                &publisher,
                recipient,
                title,
                &message,
                kind_for_status(&request.status),
                Some(link),
            )
            .await
            .is_err()
            {
                failed += 1;
                tracing::error!(request_id=%request.id, recipient_user_id=%recipient, "Calendar request notification could not be stored");
            }
        }
        Ok(failed)
    }
}

fn kind_for_status(status: &CalendarRequestStatus) -> NotificationType {
    match status {
        CalendarRequestStatus::Pending => NotificationType::Info,
        CalendarRequestStatus::Approved => NotificationType::Success,
        CalendarRequestStatus::Rejected => NotificationType::Warning,
    }
}

fn notification_content(request: &CalendarEventRequest) -> (&'static str, String, &'static str) {
    let (title, suffix, link) = match request.status {
        CalendarRequestStatus::Pending => (
            "มีคำร้องขอเพิ่มกิจกรรมใหม่",
            "รอผู้ดูแลตรวจและอนุมัติ",
            "/staff/calendar/requests?review=true&status=pending",
        ),
        CalendarRequestStatus::Approved => (
            "คำร้องขอเพิ่มกิจกรรมได้รับอนุมัติ",
            "ได้รับอนุมัติและเพิ่มในปฏิทินแล้ว",
            "/staff/calendar/requests?status=approved",
        ),
        CalendarRequestStatus::Rejected => (
            "คำร้องขอเพิ่มกิจกรรมไม่อนุมัติ",
            "ไม่อนุมัติ ดูเหตุผลได้ในคำร้องของฉัน",
            "/staff/calendar/requests?status=rejected",
        ),
    };
    (
        title,
        format!("{} · {} · {}", request.title, request.start_date, suffix),
        link,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decisions_use_success_and_warning_instead_of_new_request_info() {
        assert_eq!(
            kind_for_status(&CalendarRequestStatus::Pending).as_str(),
            "info"
        );
        assert_eq!(
            kind_for_status(&CalendarRequestStatus::Approved).as_str(),
            "success"
        );
        assert_eq!(
            kind_for_status(&CalendarRequestStatus::Rejected).as_str(),
            "warning"
        );
    }
}
