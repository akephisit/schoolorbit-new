use sqlx::PgPool;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::calendar::models::CalendarEvent;
use crate::modules::notification::events::TenantNotificationEvent;
use crate::services::notification::{
    NotificationService, NotificationType, TenantNotificationPublisher,
};

use super::shared::dedupe_user_ids;

#[derive(sqlx::FromRow)]
struct NotificationRecipientRow {
    id: Uuid,
    user_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CalendarNotificationKind {
    Created,
    Updated,
    Reminder { days_before: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarNotificationSendOutcome {
    pub recipient_count: usize,
    pub successful_count: usize,
    pub failed_count: usize,
}

impl CalendarNotificationSendOutcome {
    pub(super) fn should_mark_reminder_sent(self) -> bool {
        self.recipient_count == 0 || self.successful_count > 0
    }
}

pub async fn resolve_event_recipient_user_ids(
    pool: &PgPool,
    event_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let ids = sqlx::query_scalar::<_, Uuid>(
        r#"
        WITH targets AS (
            SELECT audience_type, grade_level_id, class_room_id
            FROM calendar_event_targets
            WHERE event_id = $1
        )
        SELECT users.id
        FROM users
        WHERE users.status = 'active'
          AND users.user_type IN ('staff', 'student', 'parent')
          AND EXISTS (
              SELECT 1 FROM targets
              WHERE targets.audience_type = 'all'
          )

        UNION ALL

        SELECT users.id
        FROM users
        WHERE users.status = 'active'
          AND users.user_type = 'staff'
          AND EXISTS (
              SELECT 1 FROM targets
              WHERE targets.audience_type = 'staff'
          )

        UNION ALL

        SELECT users.id
        FROM users
        WHERE users.status = 'active'
          AND users.user_type = 'student'
          AND EXISTS (
              SELECT 1 FROM targets
              WHERE targets.audience_type = 'student'
                AND targets.grade_level_id IS NULL
                AND targets.class_room_id IS NULL
          )

        UNION ALL

        SELECT users.id
        FROM users
        JOIN student_class_enrollments enrollments
          ON enrollments.student_id = users.id
         AND enrollments.status = 'active'
        JOIN class_rooms
          ON class_rooms.id = enrollments.class_room_id
        JOIN targets
          ON targets.audience_type = 'student'
        WHERE users.status = 'active'
          AND users.user_type = 'student'
          AND (targets.grade_level_id IS NOT NULL OR targets.class_room_id IS NOT NULL)
          AND (targets.grade_level_id IS NULL OR class_rooms.grade_level_id = targets.grade_level_id)
          AND (targets.class_room_id IS NULL OR enrollments.class_room_id = targets.class_room_id)

        UNION ALL

        SELECT parent_users.id
        FROM users parent_users
        JOIN student_parents
          ON student_parents.parent_user_id = parent_users.id
        WHERE parent_users.status = 'active'
          AND parent_users.user_type = 'parent'
          AND EXISTS (
              SELECT 1 FROM targets
              WHERE targets.audience_type = 'parent'
                AND targets.grade_level_id IS NULL
                AND targets.class_room_id IS NULL
          )

        UNION ALL

        SELECT parent_users.id
        FROM users parent_users
        JOIN student_parents
          ON student_parents.parent_user_id = parent_users.id
        JOIN users student_users
          ON student_users.id = student_parents.student_user_id
         AND student_users.status = 'active'
         AND student_users.user_type = 'student'
        JOIN student_class_enrollments enrollments
          ON enrollments.student_id = student_users.id
         AND enrollments.status = 'active'
        JOIN class_rooms
          ON class_rooms.id = enrollments.class_room_id
        JOIN targets
          ON targets.audience_type = 'parent'
        WHERE parent_users.status = 'active'
          AND parent_users.user_type = 'parent'
          AND (targets.grade_level_id IS NOT NULL OR targets.class_room_id IS NOT NULL)
          AND (targets.grade_level_id IS NULL OR class_rooms.grade_level_id = targets.grade_level_id)
          AND (targets.class_room_id IS NULL OR enrollments.class_room_id = targets.class_room_id)
        "#,
    )
    .bind(event_id)
    .fetch_all(pool)
    .await?;

    Ok(dedupe_user_ids(ids))
}

pub async fn send_event_notification(
    pool: &PgPool,
    notification_channel: &broadcast::Sender<TenantNotificationEvent>,
    tenant: &str,
    event: &CalendarEvent,
    notification_kind: CalendarNotificationKind,
) -> Result<CalendarNotificationSendOutcome, AppError> {
    let recipient_ids = resolve_event_recipient_user_ids(pool, event.id).await?;
    if recipient_ids.is_empty() {
        return Ok(CalendarNotificationSendOutcome {
            recipient_count: 0,
            successful_count: 0,
            failed_count: 0,
        });
    }

    let recipients = sqlx::query_as::<_, NotificationRecipientRow>(
        r#"
        SELECT id, user_type
        FROM users
        WHERE id = ANY($1)
        "#,
    )
    .bind(&recipient_ids)
    .fetch_all(pool)
    .await?;

    let (title, message) = calendar_notification_text(event, &notification_kind);
    let publisher = TenantNotificationPublisher::new(tenant, notification_channel);
    let mut successful_count = 0;
    let mut failed_count = 0;
    for recipient in recipients {
        if let Err(error) = NotificationService::send(
            pool,
            &publisher,
            recipient.id,
            &title,
            &message,
            NotificationType::Info,
            calendar_notification_link_for_user_type(&recipient.user_type),
        )
        .await
        {
            tracing::error!(
                event_id = %event.id,
                recipient_user_id = %recipient.id,
                error = %error,
                "Calendar event notification failed for recipient"
            );
            failed_count += 1;
        } else {
            successful_count += 1;
        }
    }

    Ok(CalendarNotificationSendOutcome {
        recipient_count: successful_count + failed_count,
        successful_count,
        failed_count,
    })
}

pub(super) fn calendar_notification_text(
    event: &CalendarEvent,
    kind: &CalendarNotificationKind,
) -> (String, String) {
    match kind {
        CalendarNotificationKind::Created => (
            format!("เพิ่มกำหนดการ: {}", event.title),
            "มีกำหนดการใหม่ในปฏิทินโรงเรียน".to_string(),
        ),
        CalendarNotificationKind::Updated => (
            format!("อัปเดตกำหนดการ: {}", event.title),
            "มีการเปลี่ยนแปลงกำหนดการในปฏิทินโรงเรียน".to_string(),
        ),
        CalendarNotificationKind::Reminder { days_before } => (
            format!("เตือนล่วงหน้า: {}", event.title),
            format!("กำหนดการนี้จะเริ่มในอีก {} วัน", days_before),
        ),
    }
}

pub(super) fn calendar_notification_link_for_user_type(user_type: &str) -> Option<&'static str> {
    match user_type {
        "staff" => Some("/staff/calendar"),
        "student" => Some("/student/calendar"),
        // V1 falls back to the parent workspace; exact child-specific links need child mapping.
        "parent" => Some("/parent"),
        _ => None,
    }
}
