use sqlx::{PgConnection, PgPool, Postgres, Transaction};
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::models::CalendarEvent;
use crate::services::{dedupe_user_ids, CalendarNotificationKind};
use school_errors::AppError;
use school_notifications::dispatch::{self, NewNotification, NotificationSource, Presentation};
use school_notifications::events::TenantNotificationEvent;
use school_notifications::publisher::NotificationType;

#[derive(sqlx::FromRow)]
struct NotificationRecipientRow {
    id: Uuid,
    user_type: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarNotificationSendOutcome {
    pub recipient_count: usize,
    pub successful_count: usize,
    pub failed_count: usize,
}

pub async fn resolve_event_recipient_user_ids(
    pool: &PgPool,
    event_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let mut connection =
        school_tenancy::TenantPool::observed_checkout_pool(pool, "calendar.notification").await?;
    resolve_event_recipient_user_ids_on_connection(&mut connection, event_id).await
}

async fn resolve_event_recipient_user_ids_on_connection(
    connection: &mut PgConnection,
    event_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    let ids = school_database::DbSession::new(&mut *connection).scalar::<Uuid>("calendar.delivery.notifications.resolve_event_recipient_user_ids_on_connection",
        r#"
        WITH targets AS (
            SELECT target.audience_type, target.grade_level_id, target.homeroom_id, target.academic_year_id, event.start_date
            FROM calendar_event_targets target JOIN calendar_events event ON event.id=target.event_id
            WHERE target.event_id = $1
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

        SELECT users.id FROM users
        WHERE users.status='active' AND users.user_type='student'
          AND EXISTS (SELECT 1 FROM targets WHERE audience_type='student' AND grade_level_id IS NULL AND homeroom_id IS NULL AND academic_year_id IS NULL)

        UNION ALL

        SELECT parent_users.id FROM users parent_users
        JOIN student_parents links ON links.parent_user_id=parent_users.id
        JOIN users student_users ON student_users.id=links.student_user_id AND student_users.status='active'
        WHERE parent_users.status='active' AND parent_users.user_type='parent'
          AND EXISTS (SELECT 1 FROM targets WHERE audience_type='parent' AND grade_level_id IS NULL AND homeroom_id IS NULL AND academic_year_id IS NULL)

        UNION ALL

        SELECT users.id
        FROM users
        JOIN student_academic_years student_year ON student_year.student_id = users.id
        LEFT JOIN homeroom_placements placement
          ON placement.student_academic_year_id = student_year.id
         AND placement.status IN ('planned', 'current')
        JOIN targets
          ON targets.audience_type = 'student'
         AND (targets.academic_year_id = student_year.academic_year_id OR (targets.academic_year_id IS NULL AND targets.grade_level_id IS NULL AND targets.homeroom_id IS NULL))
        WHERE users.status = 'active'
          AND users.user_type = 'student'
          AND (targets.grade_level_id IS NULL OR student_year.grade_level_id = targets.grade_level_id)
          AND (targets.homeroom_id IS NULL OR placement.homeroom_id = targets.homeroom_id)

        UNION ALL

        SELECT parent_users.id
        FROM users parent_users
        JOIN student_parents
          ON student_parents.parent_user_id = parent_users.id
        JOIN users student_users
          ON student_users.id = student_parents.student_user_id
         AND student_users.status = 'active'
         AND student_users.user_type = 'student'
        JOIN student_academic_years student_year
          ON student_year.student_id = student_users.id
        LEFT JOIN homeroom_placements placement
          ON placement.student_academic_year_id = student_year.id
         AND placement.status IN ('planned', 'current')
        JOIN targets
          ON targets.audience_type = 'parent'
         AND (targets.academic_year_id = student_year.academic_year_id OR (targets.academic_year_id IS NULL AND targets.grade_level_id IS NULL AND targets.homeroom_id IS NULL))
        WHERE parent_users.status = 'active'
          AND parent_users.user_type = 'parent'
          AND (targets.grade_level_id IS NULL OR student_year.grade_level_id = targets.grade_level_id)
          AND (targets.homeroom_id IS NULL OR placement.homeroom_id = targets.homeroom_id)
        "#,
    )
    .bind(event_id)
    .fetch_all()
    .await?;

    Ok(dedupe_user_ids(ids))
}

pub async fn send_event_notification(
    pool: &PgPool,
    _notification_channel: &broadcast::Sender<TenantNotificationEvent>,
    tenant: &str,
    event: &CalendarEvent,
    kind: CalendarNotificationKind,
) -> Result<CalendarNotificationSendOutcome, AppError> {
    let mut tx = school_tenancy::TenantPool::begin_observed(pool, "calendar.notification").await?;
    let result = accept_event_in_transaction(&mut tx, event, &kind, None).await?;
    tx.commit().await?;
    school_notifications::runtime::wake_after_commit(tenant);
    Ok(result)
}
pub(crate) async fn accept_event_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    event: &CalendarEvent,
    kind: &CalendarNotificationKind,
    reminder_id: Option<Uuid>,
) -> Result<CalendarNotificationSendOutcome, AppError> {
    let ids = resolve_event_recipient_user_ids_on_connection(tx, event.id).await?;
    let recipients: Vec<NotificationRecipientRow> = school_database::DbSession::new(&mut **tx)
        .query_as(
            "calendar.notifications.snapshot_types",
            "SELECT id,user_type FROM users WHERE id=ANY($1) ORDER BY id",
        )
        .bind(&ids)
        .fetch_all()
        .await?;
    let (title, message) = calendar_notification_text(event, kind);
    let identity = match (kind, reminder_id) {
        (CalendarNotificationKind::Reminder { .. }, Some(id)) => format!("calendar-reminder:{id}"),
        (CalendarNotificationKind::Reminder { .. }, None) => {
            return Err(AppError::Conflict(
                "calendar_reminder_source_required".into(),
            ));
        }
        (CalendarNotificationKind::Created, _) => format!("calendar-event:{}:created", event.id),
        (CalendarNotificationKind::Updated, _) => format!(
            "calendar-event:{}:updated:{}",
            event.id,
            event.updated_at.to_rfc3339()
        ),
    };
    let items: Vec<_> = recipients
        .iter()
        .map(|r| {
            NewNotification::new(
                r.id,
                reminder_id
                    .map(|source_id| NotificationSource::CalendarReminder { source_id })
                    .unwrap_or(NotificationSource::Direct),
                identity.clone(),
                dispatch::NotificationContent {
                    title: title.clone(),
                    message: message.clone(),
                    type_: NotificationType::Info,
                    link: calendar_notification_link_for_user_type(&r.user_type).map(str::to_owned),
                },
                Presentation::AlertOnce,
            )
        })
        .collect();
    dispatch::create_in_transaction(tx, &items).await?;
    Ok(CalendarNotificationSendOutcome {
        recipient_count: items.len(),
        successful_count: items.len(),
        failed_count: 0,
    })
}
/// Event/reminder structural validity only; accepted recipient snapshots never change here.
pub async fn authorize_reminder_on_connection(
    connection: &mut PgConnection,
    source_id: Uuid,
) -> Result<bool, AppError> {
    let event: Option<Uuid> = school_database::DbSession::new(&mut *connection)
        .scalar(
            "calendar.reminder_source.event",
            "SELECT event_id FROM calendar_event_reminders WHERE id=$1",
        )
        .bind(source_id)
        .fetch_optional()
        .await?;
    let Some(event) = event else { return Ok(false) };
    let live: Option<Uuid> = school_database::DbSession::new(&mut *connection)
        .scalar(
            "calendar.reminder_source.lock_event",
            "SELECT id FROM calendar_events WHERE id=$1 AND deleted_at IS NULL FOR SHARE",
        )
        .bind(event)
        .fetch_optional()
        .await?;
    if live.is_none() {
        return Ok(false);
    }
    Ok(school_database::DbSession::new(connection).scalar("calendar.reminder_source.live","SELECT EXISTS(SELECT 1 FROM calendar_event_reminders WHERE id=$1 AND event_id=$2 FOR SHARE)").bind(source_id).bind(event).fetch_one().await?)
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
