use chrono::{Datelike, Duration, Months, NaiveDate, NaiveTime, Utc};
use sqlx::pool::PoolConnection;
use sqlx::{PgPool, Postgres, QueryBuilder, Transaction};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::db::{admin_client::AdminClient, pool_manager::PoolManager};
use crate::error::AppError;
use crate::modules::calendar::models::{
    CalendarAudienceType, CalendarCategory, CalendarEvent, CalendarEventQuery,
    CalendarEventReminder, CalendarEventRow, CalendarEventTag, CalendarEventTarget,
    CalendarEventTargetInput, CalendarPublicEvent, CalendarTag, CalendarViewerEvent,
    CalendarVisibility, UpsertCalendarCategoryRequest, UpsertCalendarEventRequest,
    UpsertCalendarTagRequest,
};
use crate::modules::notification::events::TenantNotificationEvent;
use crate::services::notification::{
    NotificationService, NotificationType, TenantNotificationPublisher,
};

const DUPLICATE_CATEGORY_MESSAGE: &str = "มีหมวดหมู่นี้อยู่แล้ว";
const DUPLICATE_TAG_MESSAGE: &str = "มีแท็กนี้อยู่แล้ว";
const EVENT_NOT_FOUND_MESSAGE: &str = "ไม่พบกำหนดการ";
const CATEGORY_NOT_FOUND_MESSAGE: &str = "ไม่พบหมวดหมู่";
const TAG_NOT_FOUND_MESSAGE: &str = "ไม่พบแท็ก";
const INVALID_TAGS_MESSAGE: &str = "มีแท็กที่ไม่ถูกต้อง กรุณาเลือกแท็กใหม่";

const SELECT_DUE_CALENDAR_REMINDER_CANDIDATES_SQL: &str = r#"
SELECT id, event_id, days_before
FROM calendar_event_reminders
WHERE remind_on <= $1
  AND sent_at IS NULL
  AND NOT (id = ANY($2::uuid[]))
ORDER BY remind_on ASC, created_at ASC
LIMIT 200
"#;

const REFETCH_DUE_CALENDAR_REMINDER_AFTER_LOCK_SQL: &str = r#"
SELECT id, event_id, days_before
FROM calendar_event_reminders
WHERE id = $1 AND sent_at IS NULL
"#;

const MARK_CALENDAR_REMINDER_SENT_SQL: &str = r#"
UPDATE calendar_event_reminders
SET sent_at = NOW()
WHERE id = $1 AND sent_at IS NULL
"#;

const TRY_CALENDAR_REMINDER_ADVISORY_LOCK_SQL: &str = "SELECT pg_try_advisory_lock($1, $2)";
const RELEASE_CALENDAR_REMINDER_ADVISORY_LOCK_SQL: &str = "SELECT pg_advisory_unlock($1, $2)";

const EVENT_SELECT_WITH_CATEGORY: &str = r#"
SELECT
    e.id,
    e.category_id,
    c.name AS category_name,
    c.color AS category_color,
    e.title,
    e.description,
    e.location,
    e.start_date,
    e.end_date,
    e.all_day,
    e.start_time,
    e.end_time,
    e.is_public,
    e.created_by,
    e.updated_by,
    e.created_at,
    e.updated_at
FROM calendar_events e
LEFT JOIN calendar_categories c ON c.id = e.category_id
"#;

#[derive(sqlx::FromRow)]
struct CalendarEventTargetRow {
    event_id: Uuid,
    id: Uuid,
    audience_type: String,
    grade_level_id: Option<Uuid>,
    class_room_id: Option<Uuid>,
}

#[derive(sqlx::FromRow)]
struct CalendarEventReminderRow {
    event_id: Uuid,
    id: Uuid,
    days_before: i32,
    remind_on: NaiveDate,
    sent_at: Option<chrono::DateTime<Utc>>,
}

#[derive(sqlx::FromRow)]
struct CalendarEventTagRow {
    event_id: Uuid,
    id: Uuid,
    name: String,
}

#[derive(sqlx::FromRow)]
struct DueCalendarEventReminderRow {
    id: Uuid,
    event_id: Uuid,
    days_before: i32,
}

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

#[derive(Debug, Clone)]
pub struct CalendarEventMutationOutcome {
    pub event: CalendarEvent,
    pub notify_audience: bool,
    pub notification_kind: CalendarNotificationKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CalendarNotificationSendOutcome {
    pub recipient_count: usize,
    pub successful_count: usize,
    pub failed_count: usize,
}

impl CalendarNotificationSendOutcome {
    fn should_mark_reminder_sent(self) -> bool {
        self.recipient_count == 0 || self.successful_count > 0
    }
}

pub fn validate_event_date_time(
    start_date: NaiveDate,
    end_date: NaiveDate,
    all_day: bool,
    start_time: Option<NaiveTime>,
    end_time: Option<NaiveTime>,
) -> Result<(), AppError> {
    if end_date < start_date {
        return Err(AppError::BadRequest("วันที่สิ้นสุดต้องไม่ก่อนวันที่เริ่มต้น".to_string()));
    }

    if all_day {
        return Ok(());
    }

    if start_date == end_date {
        match (start_time, end_time) {
            (Some(start), Some(end)) if end > start => Ok(()),
            (Some(_), Some(_)) => Err(AppError::BadRequest("เวลาสิ้นสุดต้องหลังเวลาเริ่มต้น".to_string())),
            _ => Err(AppError::BadRequest(
                "event แบบระบุเวลาต้องมีเวลาเริ่มต้นและสิ้นสุด".to_string(),
            )),
        }
    } else if start_time.is_some() && end_time.is_some() {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "event หลายวันที่ระบุเวลาต้องมีเวลาเริ่มต้นและสิ้นสุด".to_string(),
        ))
    }
}

fn reminder_schedule(
    start_date: NaiveDate,
    offsets: &[i32],
) -> Result<Vec<(i32, NaiveDate)>, AppError> {
    let mut pairs = Vec::with_capacity(offsets.len());
    let mut seen = HashSet::new();

    for days_before in offsets {
        if *days_before <= 0 {
            return Err(AppError::BadRequest(
                "จำนวนวันแจ้งเตือนต้องมากกว่า 0".to_string(),
            ));
        }
        if seen.insert(*days_before) {
            let remind_on = start_date
                .checked_sub_signed(Duration::days(i64::from(*days_before)))
                .ok_or_else(|| AppError::BadRequest("วันที่แจ้งเตือนอยู่นอกช่วงที่รองรับ".to_string()))?;
            pairs.push((*days_before, remind_on));
        }
    }

    pairs.sort_by_key(|(_, remind_on)| *remind_on);
    Ok(pairs)
}

#[cfg(test)]
pub fn reminder_dates(start_date: NaiveDate, offsets: &[i32]) -> Result<Vec<NaiveDate>, AppError> {
    Ok(reminder_schedule(start_date, offsets)?
        .into_iter()
        .map(|(_, remind_on)| remind_on)
        .collect())
}

pub fn validate_targets(targets: &[CalendarEventTargetInput]) -> Result<(), AppError> {
    if targets.is_empty() {
        return Err(AppError::BadRequest("ต้องเลือกผู้เห็นอย่างน้อยหนึ่งกลุ่ม".to_string()));
    }

    let mut seen_targets = HashSet::new();

    for target in targets {
        if target.grade_level_id.is_some() && target.class_room_id.is_some() {
            return Err(AppError::BadRequest(
                "เลือกได้เพียงระดับชั้นหรือห้องเรียนอย่างใดอย่างหนึ่ง".to_string(),
            ));
        }

        match &target.audience_type {
            CalendarAudienceType::All | CalendarAudienceType::Staff => {
                if target.grade_level_id.is_some() || target.class_room_id.is_some() {
                    return Err(AppError::BadRequest(
                        "กลุ่มผู้เห็น all/staff ไม่รองรับการกรองระดับชั้นหรือห้องเรียน".to_string(),
                    ));
                }
            }
            CalendarAudienceType::Student | CalendarAudienceType::Parent => {}
        }

        let target_key = (
            target.audience_type.as_str(),
            target.grade_level_id,
            target.class_room_id,
        );
        if !seen_targets.insert(target_key) {
            return Err(AppError::BadRequest("กลุ่มผู้เห็นซ้ำ".to_string()));
        }
    }

    Ok(())
}

pub fn dedupe_user_ids(ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();

    for id in ids {
        if seen.insert(id) {
            deduped.push(id);
        }
    }

    deduped
}

fn dedupe_uuid_ids(ids: &[Uuid]) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    ids.iter().copied().filter(|id| seen.insert(*id)).collect()
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

fn calendar_notification_text(
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

fn calendar_notification_link_for_user_type(user_type: &str) -> Option<&'static str> {
    match user_type {
        "staff" => Some("/staff/calendar"),
        "student" => Some("/student/calendar"),
        // V1 falls back to the parent workspace; exact child-specific links need child mapping.
        "parent" => Some("/parent"),
        _ => None,
    }
}

async fn list_targets_for_events(
    pool: &PgPool,
    event_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<CalendarEventTarget>>, AppError> {
    if event_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, CalendarEventTargetRow>(
        r#"
        SELECT event_id, id, audience_type, grade_level_id, class_room_id
        FROM calendar_event_targets
        WHERE event_id = ANY($1)
        ORDER BY event_id, audience_type, grade_level_id NULLS FIRST, class_room_id NULLS FIRST
        "#,
    )
    .bind(event_ids)
    .fetch_all(pool)
    .await?;

    let mut targets = HashMap::new();
    for row in rows {
        targets
            .entry(row.event_id)
            .or_insert_with(Vec::new)
            .push(CalendarEventTarget {
                id: row.id,
                audience_type: row.audience_type,
                grade_level_id: row.grade_level_id,
                class_room_id: row.class_room_id,
            });
    }

    Ok(targets)
}

async fn list_reminders_for_events(
    pool: &PgPool,
    event_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<CalendarEventReminder>>, AppError> {
    if event_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, CalendarEventReminderRow>(
        r#"
        SELECT event_id, id, days_before, remind_on, sent_at
        FROM calendar_event_reminders
        WHERE event_id = ANY($1)
        ORDER BY event_id, remind_on, days_before
        "#,
    )
    .bind(event_ids)
    .fetch_all(pool)
    .await?;

    let mut reminders = HashMap::new();
    for row in rows {
        reminders
            .entry(row.event_id)
            .or_insert_with(Vec::new)
            .push(CalendarEventReminder {
                id: row.id,
                days_before: row.days_before,
                remind_on: row.remind_on,
                sent_at: row.sent_at,
            });
    }

    Ok(reminders)
}

async fn list_tags_for_events(
    pool: &PgPool,
    event_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<CalendarEventTag>>, AppError> {
    if event_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, CalendarEventTagRow>(
        r#"
        SELECT event_tags.event_id, tags.id, tags.name
        FROM calendar_event_tags event_tags
        JOIN calendar_tags tags ON tags.id = event_tags.tag_id
        WHERE event_tags.event_id = ANY($1)
        ORDER BY event_tags.event_id, LOWER(tags.name), tags.id
        "#,
    )
    .bind(event_ids)
    .fetch_all(pool)
    .await?;

    let mut tags = HashMap::new();
    for row in rows {
        tags.entry(row.event_id)
            .or_insert_with(Vec::new)
            .push(CalendarEventTag {
                id: row.id,
                name: row.name,
            });
    }

    Ok(tags)
}

async fn hydrate_events(
    pool: &PgPool,
    rows: Vec<CalendarEventRow>,
) -> Result<Vec<CalendarEvent>, AppError> {
    let event_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let targets = list_targets_for_events(pool, &event_ids).await?;
    let reminders = list_reminders_for_events(pool, &event_ids).await?;
    let tags = list_tags_for_events(pool, &event_ids).await?;

    Ok(rows
        .into_iter()
        .map(|row| CalendarEvent {
            id: row.id,
            category_id: row.category_id,
            category_name: row.category_name,
            category_color: row.category_color,
            title: row.title,
            description: row.description,
            location: row.location,
            start_date: row.start_date,
            end_date: row.end_date,
            all_day: row.all_day,
            start_time: row.start_time,
            end_time: row.end_time,
            is_public: row.is_public,
            tags: tags.get(&row.id).cloned().unwrap_or_default(),
            targets: targets.get(&row.id).cloned().unwrap_or_default(),
            reminders: reminders.get(&row.id).cloned().unwrap_or_default(),
            created_by: row.created_by,
            updated_by: row.updated_by,
            created_at: row.created_at,
            updated_at: row.updated_at,
        })
        .collect())
}

async fn get_event_for_response(pool: &PgPool, event_id: Uuid) -> Result<CalendarEvent, AppError> {
    let mut builder = QueryBuilder::<Postgres>::new(EVENT_SELECT_WITH_CATEGORY);
    builder.push(" WHERE e.id = ");
    builder.push_bind(event_id);
    builder.push(" AND e.deleted_at IS NULL");

    let rows = builder
        .build_query_as::<CalendarEventRow>()
        .fetch_all(pool)
        .await?;
    let mut events = hydrate_events(pool, rows).await?;

    events
        .pop()
        .ok_or_else(|| AppError::NotFound(EVENT_NOT_FOUND_MESSAGE.to_string()))
}

pub async fn list_categories(pool: &PgPool) -> Result<Vec<CalendarCategory>, AppError> {
    sqlx::query_as::<_, CalendarCategory>(
        r#"
        SELECT id, name, color, order_index, is_active, created_at, updated_at
        FROM calendar_categories
        WHERE is_active = true
        ORDER BY order_index, name
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

pub async fn create_category(
    pool: &PgPool,
    payload: UpsertCalendarCategoryRequest,
) -> Result<CalendarCategory, AppError> {
    sqlx::query_as::<_, CalendarCategory>(
        r#"
        INSERT INTO calendar_categories (name, color, order_index, is_active)
        VALUES (
            $1,
            $2,
            COALESCE($3, (SELECT COALESCE(MAX(order_index), 0) + 1 FROM calendar_categories)),
            COALESCE($4, true)
        )
        RETURNING id, name, color, order_index, is_active, created_at, updated_at
        "#,
    )
    .bind(payload.name)
    .bind(payload.color)
    .bind(payload.order_index)
    .bind(payload.is_active)
    .fetch_one(pool)
    .await
    .map_err(map_category_write_error)
}

pub async fn update_category(
    pool: &PgPool,
    id: Uuid,
    payload: UpsertCalendarCategoryRequest,
) -> Result<CalendarCategory, AppError> {
    let category = sqlx::query_as::<_, CalendarCategory>(
        r#"
        UPDATE calendar_categories
        SET
            name = $1,
            color = $2,
            order_index = COALESCE($3, order_index),
            is_active = COALESCE($4, is_active)
        WHERE id = $5 AND is_active = true
        RETURNING id, name, color, order_index, is_active, created_at, updated_at
        "#,
    )
    .bind(payload.name)
    .bind(payload.color)
    .bind(payload.order_index)
    .bind(payload.is_active)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(map_category_write_error)?;

    category.ok_or_else(|| AppError::NotFound(CATEGORY_NOT_FOUND_MESSAGE.to_string()))
}

pub async fn hard_delete_category(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM calendar_categories WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(CATEGORY_NOT_FOUND_MESSAGE.to_string()));
    }

    Ok(())
}

pub async fn list_tags(pool: &PgPool) -> Result<Vec<CalendarTag>, AppError> {
    sqlx::query_as::<_, CalendarTag>(
        r#"
        SELECT id, name, created_at, updated_at
        FROM calendar_tags
        ORDER BY LOWER(name), id
        "#,
    )
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

pub async fn create_tag(
    pool: &PgPool,
    payload: UpsertCalendarTagRequest,
) -> Result<CalendarTag, AppError> {
    let name = normalized_tag_name(payload.name)?;
    sqlx::query_as::<_, CalendarTag>(
        r#"
        INSERT INTO calendar_tags (name)
        VALUES ($1)
        RETURNING id, name, created_at, updated_at
        "#,
    )
    .bind(name)
    .fetch_one(pool)
    .await
    .map_err(map_tag_write_error)
}

pub async fn update_tag(
    pool: &PgPool,
    id: Uuid,
    payload: UpsertCalendarTagRequest,
) -> Result<CalendarTag, AppError> {
    let name = normalized_tag_name(payload.name)?;
    let tag = sqlx::query_as::<_, CalendarTag>(
        r#"
        UPDATE calendar_tags
        SET name = $1
        WHERE id = $2
        RETURNING id, name, created_at, updated_at
        "#,
    )
    .bind(name)
    .bind(id)
    .fetch_optional(pool)
    .await
    .map_err(map_tag_write_error)?;

    tag.ok_or_else(|| AppError::NotFound(TAG_NOT_FOUND_MESSAGE.to_string()))
}

pub async fn hard_delete_tag(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query("DELETE FROM calendar_tags WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(TAG_NOT_FOUND_MESSAGE.to_string()));
    }

    Ok(())
}

pub async fn create_event(
    pool: &PgPool,
    actor_user_id: Uuid,
    payload: UpsertCalendarEventRequest,
) -> Result<CalendarEventMutationOutcome, AppError> {
    validate_event_date_time(
        payload.start_date,
        payload.end_date,
        payload.all_day,
        payload.start_time,
        payload.end_time,
    )?;
    validate_targets(&payload.targets)?;
    let reminder_pairs = reminder_schedule(payload.start_date, &payload.reminder_offsets_days)?;
    let tag_ids = dedupe_uuid_ids(&payload.tag_ids);
    let notify_audience = payload.notify_audience;

    let mut transaction = pool.begin().await?;
    let event_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO calendar_events (
            category_id, title, description, location, start_date, end_date,
            all_day, start_time, end_time, is_public, created_by, updated_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $11)
        RETURNING id
        "#,
    )
    .bind(payload.category_id)
    .bind(&payload.title)
    .bind(&payload.description)
    .bind(&payload.location)
    .bind(payload.start_date)
    .bind(payload.end_date)
    .bind(payload.all_day)
    .bind(payload.start_time)
    .bind(payload.end_time)
    .bind(payload.is_public)
    .bind(actor_user_id)
    .fetch_one(&mut *transaction)
    .await?;

    replace_event_targets(&mut transaction, event_id, &payload.targets).await?;
    replace_event_tags(&mut transaction, event_id, &tag_ids).await?;
    replace_pending_event_reminders(&mut transaction, event_id, reminder_pairs).await?;
    transaction.commit().await?;

    let event = get_event_for_response(pool, event_id).await?;
    Ok(CalendarEventMutationOutcome {
        event,
        notify_audience,
        notification_kind: CalendarNotificationKind::Created,
    })
}

pub async fn update_event(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    payload: UpsertCalendarEventRequest,
) -> Result<CalendarEventMutationOutcome, AppError> {
    validate_event_date_time(
        payload.start_date,
        payload.end_date,
        payload.all_day,
        payload.start_time,
        payload.end_time,
    )?;
    validate_targets(&payload.targets)?;
    let reminder_pairs = reminder_schedule(payload.start_date, &payload.reminder_offsets_days)?;
    let tag_ids = dedupe_uuid_ids(&payload.tag_ids);
    let notify_audience = payload.notify_audience;

    let mut transaction = pool.begin().await?;
    let event_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        UPDATE calendar_events
        SET
            category_id = $1,
            title = $2,
            description = $3,
            location = $4,
            start_date = $5,
            end_date = $6,
            all_day = $7,
            start_time = $8,
            end_time = $9,
            is_public = $10,
            updated_by = $11
        WHERE id = $12 AND deleted_at IS NULL
        RETURNING id
        "#,
    )
    .bind(payload.category_id)
    .bind(&payload.title)
    .bind(&payload.description)
    .bind(&payload.location)
    .bind(payload.start_date)
    .bind(payload.end_date)
    .bind(payload.all_day)
    .bind(payload.start_time)
    .bind(payload.end_time)
    .bind(payload.is_public)
    .bind(actor_user_id)
    .bind(id)
    .fetch_optional(&mut *transaction)
    .await?;

    let event_id =
        event_id.ok_or_else(|| AppError::NotFound(EVENT_NOT_FOUND_MESSAGE.to_string()))?;
    replace_event_targets(&mut transaction, event_id, &payload.targets).await?;
    replace_event_tags(&mut transaction, event_id, &tag_ids).await?;
    replace_pending_event_reminders(&mut transaction, event_id, reminder_pairs).await?;
    transaction.commit().await?;

    let event = get_event_for_response(pool, event_id).await?;
    Ok(CalendarEventMutationOutcome {
        event,
        notify_audience,
        notification_kind: CalendarNotificationKind::Updated,
    })
}

pub async fn soft_delete_event(
    pool: &PgPool,
    id: Uuid,
    actor_user_id: Uuid,
) -> Result<(), AppError> {
    let mut transaction = pool.begin().await?;
    let result = sqlx::query(
        r#"
        UPDATE calendar_events
        SET deleted_at = NOW(), updated_by = $2
        WHERE id = $1 AND deleted_at IS NULL
        "#,
    )
    .bind(id)
    .bind(actor_user_id)
    .execute(&mut *transaction)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(EVENT_NOT_FOUND_MESSAGE.to_string()));
    }

    sqlx::query(
        r#"
        DELETE FROM calendar_event_reminders
        WHERE event_id = $1 AND sent_at IS NULL
        "#,
    )
    .bind(id)
    .execute(&mut *transaction)
    .await?;

    transaction.commit().await?;

    Ok(())
}

pub async fn list_management_events(
    pool: &PgPool,
    query: CalendarEventQuery,
) -> Result<Vec<CalendarEvent>, AppError> {
    let (from, to) = normalized_event_range(&query, tenant_today())?;
    let mut builder = QueryBuilder::<Postgres>::new(EVENT_SELECT_WITH_CATEGORY);
    push_base_event_filters(&mut builder, from, to);
    push_event_query_filters(&mut builder, &query);

    if let Some(audience) = &query.audience {
        builder.push(
            " AND EXISTS (
                SELECT 1 FROM calendar_event_targets target
                WHERE target.event_id = e.id AND target.audience_type = ",
        );
        builder.push_bind(audience.as_str());
        builder.push(")");
    }

    push_search_filter(&mut builder, query.q.as_deref());
    push_event_order(&mut builder);

    let rows = builder
        .build_query_as::<CalendarEventRow>()
        .fetch_all(pool)
        .await?;
    hydrate_events(pool, rows).await
}

pub async fn list_my_events(
    pool: &PgPool,
    user_id: Uuid,
    query: CalendarEventQuery,
) -> Result<Vec<CalendarViewerEvent>, AppError> {
    let user_type = active_user_type(pool, user_id).await?;
    self_calendar_user_type_access(&user_type)?;
    let (from, to) = normalized_event_range(&query, tenant_today())?;
    let mut builder = QueryBuilder::<Postgres>::new(EVENT_SELECT_WITH_CATEGORY);
    push_base_event_filters(&mut builder, from, to);
    push_event_query_filters(&mut builder, &query);
    push_my_event_target_filter(&mut builder, user_id, &user_type, query.audience.as_ref());
    push_search_filter(&mut builder, query.q.as_deref());
    push_event_order(&mut builder);

    let rows = builder
        .build_query_as::<CalendarEventRow>()
        .fetch_all(pool)
        .await?;
    let mut events = hydrate_events(pool, rows).await?;
    retain_visible_targets_for_user_type(&mut events, &user_type);
    Ok(events.into_iter().map(CalendarViewerEvent::from).collect())
}

pub async fn list_child_events(
    pool: &PgPool,
    parent_id: Uuid,
    student_id: Uuid,
    query: CalendarEventQuery,
) -> Result<Vec<CalendarViewerEvent>, AppError> {
    let (from, to) = normalized_event_range(&query, tenant_today())?;
    let mut builder = QueryBuilder::<Postgres>::new(EVENT_SELECT_WITH_CATEGORY);
    push_base_event_filters(&mut builder, from, to);
    push_event_query_filters(&mut builder, &query);
    push_child_event_target_filter(&mut builder, parent_id, student_id, query.audience.as_ref());
    push_search_filter(&mut builder, query.q.as_deref());
    push_event_order(&mut builder);

    let rows = builder
        .build_query_as::<CalendarEventRow>()
        .fetch_all(pool)
        .await?;
    let mut events = hydrate_events(pool, rows).await?;
    retain_child_visible_targets(&mut events);
    Ok(events.into_iter().map(CalendarViewerEvent::from).collect())
}

pub async fn list_public_events(
    pool: &PgPool,
    query: CalendarEventQuery,
) -> Result<Vec<CalendarPublicEvent>, AppError> {
    let (from, to) = normalized_event_range(&query, tenant_today())?;
    let mut builder = QueryBuilder::<Postgres>::new(EVENT_SELECT_WITH_CATEGORY);
    push_base_event_filters(&mut builder, from, to);
    builder.push(" AND e.is_public = true");
    push_category_and_tag_query_filters(&mut builder, &query);

    push_search_filter(&mut builder, query.q.as_deref());
    push_event_order(&mut builder);

    let rows = builder
        .build_query_as::<CalendarEventRow>()
        .fetch_all(pool)
        .await?;
    let events = hydrate_events(pool, rows).await?;

    Ok(events.into_iter().map(CalendarPublicEvent::from).collect())
}

pub async fn process_due_reminders(
    pool: &PgPool,
    notification_channel: &broadcast::Sender<TenantNotificationEvent>,
    tenant: &str,
    tenant_current_date: NaiveDate,
) -> Result<i64, AppError> {
    let mut marked_sent_count = 0;
    let mut attempted_ids = Vec::new();

    loop {
        let candidates =
            fetch_due_reminder_candidates(pool, tenant_current_date, &attempted_ids).await?;
        if candidates.is_empty() {
            break;
        }

        for candidate in candidates {
            attempted_ids.push(candidate.id);
            if process_due_reminder_candidate(pool, notification_channel, tenant, candidate).await?
            {
                marked_sent_count += 1;
            }
        }
    }

    Ok(marked_sent_count)
}

async fn fetch_due_reminder_candidates(
    pool: &PgPool,
    tenant_current_date: NaiveDate,
    attempted_ids: &[Uuid],
) -> Result<Vec<DueCalendarEventReminderRow>, AppError> {
    Ok(sqlx::query_as::<_, DueCalendarEventReminderRow>(
        select_due_calendar_reminder_candidates_sql(),
    )
    .bind(tenant_current_date)
    .bind(attempted_ids)
    .fetch_all(pool)
    .await?)
}

async fn process_due_reminder_candidate(
    pool: &PgPool,
    notification_channel: &broadcast::Sender<TenantNotificationEvent>,
    tenant: &str,
    candidate: DueCalendarEventReminderRow,
) -> Result<bool, AppError> {
    let lock_keys = calendar_reminder_advisory_lock_keys(candidate.id);
    let mut connection = pool.acquire().await?;

    let acquired = sqlx::query_scalar::<_, bool>(try_calendar_reminder_advisory_lock_sql())
        .bind(lock_keys.0)
        .bind(lock_keys.1)
        .fetch_one(&mut *connection)
        .await?;

    if !acquired {
        return Ok(false);
    }

    let processing_result = process_advisory_locked_reminder(
        pool,
        notification_channel,
        tenant,
        &mut connection,
        candidate.id,
    )
    .await;
    let release_result =
        release_calendar_reminder_advisory_lock(&mut connection, candidate.id, lock_keys).await;

    if let Err(error) = release_result {
        tracing::error!(
            reminder_id = %candidate.id,
            error = %error,
            "Failed to release calendar reminder advisory lock"
        );
        if processing_result.is_ok() {
            return Err(error);
        }
    }

    processing_result
}

async fn process_advisory_locked_reminder(
    pool: &PgPool,
    notification_channel: &broadcast::Sender<TenantNotificationEvent>,
    tenant: &str,
    connection: &mut PoolConnection<Postgres>,
    reminder_id: Uuid,
) -> Result<bool, AppError> {
    // The session-level advisory lock is held on this connection only for one reminder.
    // No row transaction is kept open while event loading and notification fan-out run.
    let reminder = sqlx::query_as::<_, DueCalendarEventReminderRow>(
        refetch_due_calendar_reminder_after_lock_sql(),
    )
    .bind(reminder_id)
    .fetch_optional(&mut **connection)
    .await?;

    let Some(reminder) = reminder else {
        return Ok(false);
    };

    let event = match get_event_for_response(pool, reminder.event_id).await {
        Ok(event) => event,
        Err(AppError::NotFound(_)) => {
            tracing::warn!(
                reminder_id = %reminder.id,
                event_id = %reminder.event_id,
                "Skipping calendar reminder for missing or deleted event"
            );
            return mark_calendar_reminder_sent(connection, reminder.id).await;
        }
        Err(error) => {
            tracing::error!(
                reminder_id = %reminder.id,
                event_id = %reminder.event_id,
                error = %error,
                "Failed to load calendar event for reminder"
            );
            return Ok(false);
        }
    };

    let send_outcome = match send_event_notification(
        pool,
        notification_channel,
        tenant,
        &event,
        CalendarNotificationKind::Reminder {
            days_before: reminder.days_before,
        },
    )
    .await
    {
        Ok(outcome) => outcome,
        Err(error) => {
            tracing::error!(
                reminder_id = %reminder.id,
                event_id = %reminder.event_id,
                error = %error,
                "Calendar reminder notification failed"
            );
            return Ok(false);
        }
    };

    if !send_outcome.should_mark_reminder_sent() {
        tracing::warn!(
            reminder_id = %reminder.id,
            event_id = %reminder.event_id,
            recipient_count = send_outcome.recipient_count,
            failed_count = send_outcome.failed_count,
            "Calendar reminder notification had no successful sends; leaving reminder pending"
        );
        return Ok(false);
    }

    mark_calendar_reminder_sent(connection, reminder.id).await
}

async fn mark_calendar_reminder_sent(
    connection: &mut PoolConnection<Postgres>,
    reminder_id: Uuid,
) -> Result<bool, AppError> {
    let result = sqlx::query(mark_calendar_reminder_sent_sql())
        .bind(reminder_id)
        .execute(&mut **connection)
        .await?;

    Ok(result.rows_affected() > 0)
}

async fn release_calendar_reminder_advisory_lock(
    connection: &mut PoolConnection<Postgres>,
    reminder_id: Uuid,
    lock_keys: (i32, i32),
) -> Result<(), AppError> {
    let released = sqlx::query_scalar::<_, bool>(release_calendar_reminder_advisory_lock_sql())
        .bind(lock_keys.0)
        .bind(lock_keys.1)
        .fetch_one(&mut **connection)
        .await?;

    if !released {
        tracing::warn!(
            reminder_id = %reminder_id,
            "Calendar reminder advisory lock was not held during release"
        );
    }

    Ok(())
}

fn calendar_reminder_advisory_lock_keys(reminder_id: Uuid) -> (i32, i32) {
    let bytes = reminder_id.as_bytes();
    (
        i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
        i32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
    )
}

pub async fn process_due_calendar_reminders_for_all_tenants(
    admin_client: Arc<AdminClient>,
    pool_manager: Arc<PoolManager>,
    notification_channel: broadcast::Sender<TenantNotificationEvent>,
) {
    let tenant_current_date = tenant_today();

    let schools = match admin_client.list_active_schools().await {
        Ok(schools) => schools,
        Err(error) => {
            tracing::error!("Failed to fetch schools for calendar reminders: {}", error);
            return;
        }
    };

    for school in schools {
        let Some(db_url) = school
            .db_connection_string
            .filter(|value| !value.is_empty())
        else {
            tracing::warn!(
                "Skipping calendar reminders for {}: no database URL",
                school.subdomain
            );
            continue;
        };

        match pool_manager.get_pool(&db_url, &school.subdomain).await {
            Ok(pool) => {
                if let Err(error) = process_due_reminders(
                    &pool,
                    &notification_channel,
                    &school.subdomain,
                    tenant_current_date,
                )
                .await
                {
                    tracing::error!(
                        "Calendar reminder processing failed for {}: {}",
                        school.subdomain,
                        error
                    );
                }
            }
            Err(error) => {
                tracing::error!(
                    "Failed to open tenant pool for calendar reminders {}: {}",
                    school.subdomain,
                    error
                );
            }
        }
    }
}

async fn replace_event_targets(
    transaction: &mut Transaction<'_, Postgres>,
    event_id: Uuid,
    targets: &[CalendarEventTargetInput],
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM calendar_event_targets WHERE event_id = $1")
        .bind(event_id)
        .execute(&mut **transaction)
        .await?;

    if targets.is_empty() {
        return Ok(());
    }

    let mut builder = QueryBuilder::<Postgres>::new(
        "INSERT INTO calendar_event_targets (event_id, audience_type, grade_level_id, class_room_id) ",
    );
    builder.push_values(targets, |mut row, target| {
        row.push_bind(event_id)
            .push_bind(target.audience_type.as_str())
            .push_bind(target.grade_level_id)
            .push_bind(target.class_room_id);
    });
    builder.build().execute(&mut **transaction).await?;

    Ok(())
}

async fn replace_event_tags(
    transaction: &mut Transaction<'_, Postgres>,
    event_id: Uuid,
    tag_ids: &[Uuid],
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM calendar_event_tags WHERE event_id = $1")
        .bind(event_id)
        .execute(&mut **transaction)
        .await?;

    if tag_ids.is_empty() {
        return Ok(());
    }

    let result = sqlx::query(
        r#"
        INSERT INTO calendar_event_tags (event_id, tag_id)
        SELECT $1, tags.id
        FROM calendar_tags tags
        WHERE tags.id = ANY($2::uuid[])
        "#,
    )
    .bind(event_id)
    .bind(tag_ids)
    .execute(&mut **transaction)
    .await?;

    if result.rows_affected() != tag_ids.len() as u64 {
        return Err(AppError::BadRequest(INVALID_TAGS_MESSAGE.to_string()));
    }

    Ok(())
}

async fn replace_pending_event_reminders(
    transaction: &mut Transaction<'_, Postgres>,
    event_id: Uuid,
    reminder_pairs: Vec<(i32, NaiveDate)>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        DELETE FROM calendar_event_reminders
        WHERE event_id = $1 AND sent_at IS NULL
        "#,
    )
    .bind(event_id)
    .execute(&mut **transaction)
    .await?;

    if reminder_pairs.is_empty() {
        return Ok(());
    }

    let mut builder = QueryBuilder::<Postgres>::new(
        "INSERT INTO calendar_event_reminders (event_id, days_before, remind_on) ",
    );
    builder.push_values(reminder_pairs, |mut row, (days_before, remind_on)| {
        row.push_bind(event_id)
            .push_bind(days_before)
            .push_bind(remind_on);
    });
    builder.push(
        r#"
        ON CONFLICT (event_id, days_before) DO UPDATE SET
            remind_on = EXCLUDED.remind_on,
            updated_at = NOW()
        "#,
    );
    builder.build().execute(&mut **transaction).await?;

    Ok(())
}

fn select_due_calendar_reminder_candidates_sql() -> &'static str {
    SELECT_DUE_CALENDAR_REMINDER_CANDIDATES_SQL
}

fn refetch_due_calendar_reminder_after_lock_sql() -> &'static str {
    REFETCH_DUE_CALENDAR_REMINDER_AFTER_LOCK_SQL
}

fn mark_calendar_reminder_sent_sql() -> &'static str {
    MARK_CALENDAR_REMINDER_SENT_SQL
}

fn try_calendar_reminder_advisory_lock_sql() -> &'static str {
    TRY_CALENDAR_REMINDER_ADVISORY_LOCK_SQL
}

fn release_calendar_reminder_advisory_lock_sql() -> &'static str {
    RELEASE_CALENDAR_REMINDER_ADVISORY_LOCK_SQL
}

fn normalized_event_range(
    query: &CalendarEventQuery,
    today: NaiveDate,
) -> Result<(NaiveDate, NaiveDate), AppError> {
    match (query.from, query.to) {
        (Some(from), Some(to)) if from > to => {
            Err(AppError::BadRequest("วันที่เริ่มต้นต้องไม่หลังวันที่สิ้นสุด".to_string()))
        }
        (Some(from), Some(to)) => Ok((from, to)),
        _ => Ok(current_month_range(today)),
    }
}

fn tenant_today() -> NaiveDate {
    (Utc::now() + Duration::hours(7)).date_naive()
}

fn current_month_range(today: NaiveDate) -> (NaiveDate, NaiveDate) {
    let first_day = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
    let last_day = first_day
        .checked_add_months(Months::new(1))
        .and_then(|next_month| next_month.checked_sub_signed(Duration::days(1)))
        .unwrap_or(first_day);

    (first_day, last_day)
}

fn push_base_event_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    from: NaiveDate,
    to: NaiveDate,
) {
    builder.push(" WHERE e.deleted_at IS NULL AND e.start_date <= ");
    builder.push_bind(to);
    builder.push(" AND e.end_date >= ");
    builder.push_bind(from);
}

fn push_event_query_filters(builder: &mut QueryBuilder<'_, Postgres>, query: &CalendarEventQuery) {
    push_category_and_tag_query_filters(builder, query);

    match query.visibility.as_ref() {
        Some(CalendarVisibility::Public) => {
            builder.push(" AND e.is_public = true");
        }
        Some(CalendarVisibility::Private) => {
            builder.push(" AND e.is_public = false");
        }
        None => {}
    }
}

fn push_category_and_tag_query_filters(
    builder: &mut QueryBuilder<'_, Postgres>,
    query: &CalendarEventQuery,
) {
    if let Some(category_id) = query.category_id {
        builder.push(" AND e.category_id = ");
        builder.push_bind(category_id);
    }

    if let Some(tag_id) = query.tag_id {
        builder.push(
            " AND EXISTS (
                SELECT 1 FROM calendar_event_tags event_tags
                WHERE event_tags.event_id = e.id AND event_tags.tag_id = ",
        );
        builder.push_bind(tag_id);
        builder.push(")");
    }
}

fn target_visible_to_user_type(audience_type: &str, user_type: &str) -> bool {
    match user_type {
        "staff" | "student" => {
            audience_type == CalendarAudienceType::All.as_str() || audience_type == user_type
        }
        _ => false,
    }
}

fn target_visible_to_child_view(audience_type: &str) -> bool {
    audience_type == CalendarAudienceType::All.as_str()
        || audience_type == CalendarAudienceType::Parent.as_str()
}

fn retain_visible_targets_for_user_type(events: &mut [CalendarEvent], user_type: &str) {
    for event in events {
        event
            .targets
            .retain(|target| target_visible_to_user_type(&target.audience_type, user_type));
    }
}

fn retain_child_visible_targets(events: &mut [CalendarEvent]) {
    for event in events {
        event
            .targets
            .retain(|target| target_visible_to_child_view(&target.audience_type));
    }
}

fn self_calendar_user_type_access(user_type: &str) -> Result<(), AppError> {
    match user_type {
        "staff" | "student" => Ok(()),
        "parent" => Err(AppError::Forbidden(
            "ผู้ปกครองต้องดูปฏิทินผ่าน /api/parent/students/{student_id}/calendar/events".to_string(),
        )),
        _ => Err(AppError::Forbidden(
            "ผู้ใช้ประเภทนี้ไม่สามารถดูปฏิทินส่วนตัวได้".to_string(),
        )),
    }
}

async fn active_user_type(pool: &PgPool, user_id: Uuid) -> Result<String, AppError> {
    let user_type = sqlx::query_scalar::<_, String>(
        r#"
        SELECT user_type
        FROM users
        WHERE id = $1 AND status = 'active'
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    user_type.ok_or_else(|| AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string()))
}

fn push_target_audience_query_filter(
    builder: &mut QueryBuilder<'_, Postgres>,
    audience: Option<&CalendarAudienceType>,
) {
    if let Some(audience) = audience {
        builder.push(" AND target.audience_type = ");
        builder.push_bind(audience.as_str());
    }
}

fn push_my_event_target_filter(
    builder: &mut QueryBuilder<'_, Postgres>,
    user_id: Uuid,
    user_type: &str,
    audience: Option<&CalendarAudienceType>,
) {
    builder.push(
        " AND EXISTS (
            SELECT 1
            FROM calendar_event_targets target
            WHERE target.event_id = e.id",
    );
    push_target_audience_query_filter(builder, audience);
    builder.push(
        " AND (
                target.audience_type = 'all'",
    );

    match user_type {
        "staff" => {
            builder.push(" OR target.audience_type = 'staff'");
        }
        "student" => {
            builder.push(
                " OR (
                    target.audience_type = 'student'
                    AND (
                        (target.grade_level_id IS NULL AND target.class_room_id IS NULL)
                        OR EXISTS (
                            SELECT 1
                            FROM student_class_enrollments enrollments
                            JOIN class_rooms
                              ON class_rooms.id = enrollments.class_room_id
                            WHERE enrollments.student_id = ",
            );
            builder.push_bind(user_id);
            builder.push(
                "             AND enrollments.status = 'active'
                              AND (
                                  target.grade_level_id IS NULL
                                  OR class_rooms.grade_level_id = target.grade_level_id
                              )
                              AND (
                                  target.class_room_id IS NULL
                                  OR enrollments.class_room_id = target.class_room_id
                              )
                        )
                    )
                )",
            );
        }
        _ => {}
    }

    builder.push(
        "
            )
        )",
    );
}

fn push_child_event_target_filter(
    builder: &mut QueryBuilder<'_, Postgres>,
    parent_id: Uuid,
    student_id: Uuid,
    audience: Option<&CalendarAudienceType>,
) {
    builder.push(
        " AND EXISTS (
            SELECT 1
            FROM student_parents child_links
            WHERE child_links.parent_user_id = ",
    );
    builder.push_bind(parent_id);
    builder.push(" AND child_links.student_user_id = ");
    builder.push_bind(student_id);
    builder.push(")");

    builder.push(
        " AND EXISTS (
            SELECT 1
            FROM calendar_event_targets target
            WHERE target.event_id = e.id",
    );
    push_target_audience_query_filter(builder, audience);
    builder.push(
        " AND (
                target.audience_type = 'all'
                OR (
                    target.audience_type = 'parent'
                    AND EXISTS (
                        SELECT 1
                        FROM student_parents parent_links
                        WHERE parent_links.parent_user_id = ",
    );
    builder.push_bind(parent_id);
    builder.push(" AND parent_links.student_user_id = ");
    builder.push_bind(student_id);
    builder.push(
        "       )
                    AND (
                        (target.grade_level_id IS NULL AND target.class_room_id IS NULL)
                        OR EXISTS (
                            SELECT 1
                            FROM student_class_enrollments enrollments
                            JOIN class_rooms
                              ON class_rooms.id = enrollments.class_room_id
                            WHERE enrollments.student_id = ",
    );
    builder.push_bind(student_id);
    builder.push(
        "                 AND enrollments.status = 'active'
                              AND (
                                  target.grade_level_id IS NULL
                                  OR class_rooms.grade_level_id = target.grade_level_id
                              )
                              AND (
                                  target.class_room_id IS NULL
                                  OR enrollments.class_room_id = target.class_room_id
                              )
                        )
                    )
                )
            )
        )",
    );
}

fn push_search_filter(builder: &mut QueryBuilder<'_, Postgres>, query: Option<&str>) {
    let Some(search) = query.map(str::trim).filter(|value| !value.is_empty()) else {
        return;
    };
    let pattern = calendar_search_pattern(search);

    builder.push(" AND (e.title ILIKE ");
    builder.push_bind(pattern.clone());
    builder.push(" ESCAPE '\\' OR e.location ILIKE ");
    builder.push_bind(pattern.clone());
    builder.push(" ESCAPE '\\' OR e.description ILIKE ");
    builder.push_bind(pattern.clone());
    builder.push(
        " ESCAPE '\\' OR EXISTS (
            SELECT 1
            FROM calendar_event_tags event_tags
            JOIN calendar_tags tags ON tags.id = event_tags.tag_id
            WHERE event_tags.event_id = e.id AND tags.name ILIKE ",
    );
    builder.push_bind(pattern);
    builder.push(" ESCAPE '\\'))");
}

fn calendar_search_pattern(search: &str) -> String {
    let trimmed = search.trim();
    let mut pattern = String::with_capacity(trimmed.len() + 2);
    pattern.push('%');

    for character in trimmed.chars() {
        match character {
            '\\' => pattern.push_str("\\\\"),
            '%' => pattern.push_str("\\%"),
            '_' => pattern.push_str("\\_"),
            _ => pattern.push(character),
        }
    }

    pattern.push('%');
    pattern
}

fn push_event_order(builder: &mut QueryBuilder<'_, Postgres>) {
    builder.push(" ORDER BY e.start_date, e.start_time NULLS FIRST, e.created_at");
}

fn map_category_write_error(error: sqlx::Error) -> AppError {
    if is_duplicate_active_category_name(&error) {
        AppError::BadRequest(DUPLICATE_CATEGORY_MESSAGE.to_string())
    } else {
        AppError::from(error)
    }
}

fn normalized_tag_name(name: String) -> Result<String, AppError> {
    let normalized = name.trim();
    if normalized.is_empty() {
        return Err(AppError::BadRequest("กรุณาระบุชื่อแท็ก".to_string()));
    }
    if normalized.chars().count() > 80 {
        return Err(AppError::BadRequest("ชื่อแท็กต้องไม่เกิน 80 ตัวอักษร".to_string()));
    }

    Ok(normalized.to_string())
}

fn map_tag_write_error(error: sqlx::Error) -> AppError {
    if is_unique_violation_for_constraint(&error, "idx_calendar_tags_name_unique") {
        AppError::BadRequest(DUPLICATE_TAG_MESSAGE.to_string())
    } else {
        AppError::from(error)
    }
}

fn is_duplicate_active_category_name(error: &sqlx::Error) -> bool {
    is_unique_violation_for_constraint(error, "idx_calendar_categories_active_name_unique")
}

fn is_unique_violation_for_constraint(error: &sqlx::Error, constraint_name: &str) -> bool {
    let sqlx::Error::Database(database_error) = error else {
        return false;
    };

    if database_error.code().as_deref() != Some("23505") {
        return false;
    }

    let constraint_matches = database_error
        .constraint()
        .map(|constraint| constraint == constraint_name)
        .unwrap_or(false);
    let message_matches = database_error.message().contains(constraint_name);

    constraint_matches || message_matches
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_event_date_time_rejects_end_date_before_start() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 7, 9).unwrap();

        assert!(validate_event_date_time(start, end, true, None, None).is_err());
    }

    #[test]
    fn validate_event_date_time_rejects_same_day_end_time_before_start_time() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let start_time = NaiveTime::from_hms_opt(10, 0, 0).unwrap();
        let end_time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();

        assert!(
            validate_event_date_time(date, date, false, Some(start_time), Some(end_time)).is_err()
        );
    }

    #[test]
    fn validate_event_date_time_rejects_same_day_equal_start_and_end_time() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let time = NaiveTime::from_hms_opt(10, 0, 0).unwrap();

        assert!(validate_event_date_time(date, date, false, Some(time), Some(time)).is_err());
    }

    #[test]
    fn validate_event_date_time_accepts_multi_day_timed_event() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 7, 12).unwrap();
        let start_time = NaiveTime::from_hms_opt(15, 0, 0).unwrap();
        let end_time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();

        assert!(
            validate_event_date_time(start, end, false, Some(start_time), Some(end_time)).is_ok()
        );
    }

    #[test]
    fn reminder_dates_are_day_based_and_sorted() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let result = reminder_dates(start, &[1, 7, 3]).unwrap();

        assert_eq!(
            result,
            vec![
                NaiveDate::from_ymd_opt(2026, 7, 3).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 7).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 9).unwrap()
            ]
        );
    }

    #[test]
    fn reminder_dates_dedupes_duplicate_offsets() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let result = reminder_dates(start, &[3, 1, 3, 1]).unwrap();

        assert_eq!(
            result,
            vec![
                NaiveDate::from_ymd_opt(2026, 7, 7).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 9).unwrap()
            ]
        );
    }

    #[test]
    fn reminder_schedule_pairs_dedupe_offsets_and_sort_by_reminder_date() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let result = reminder_schedule(start, &[1, 7, 1, 3]).unwrap();

        assert_eq!(
            result,
            vec![
                (7, NaiveDate::from_ymd_opt(2026, 7, 3).unwrap()),
                (3, NaiveDate::from_ymd_opt(2026, 7, 7).unwrap()),
                (1, NaiveDate::from_ymd_opt(2026, 7, 9).unwrap()),
            ]
        );
    }

    #[test]
    fn reminder_schedule_pairs_reject_overflowing_offsets() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();

        assert!(reminder_schedule(start, &[i32::MAX]).is_err());
    }

    #[test]
    fn reminder_dates_reject_zero_or_negative_offsets() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();

        assert!(reminder_dates(start, &[0]).is_err());
        assert!(reminder_dates(start, &[-1]).is_err());
    }

    #[test]
    fn reminder_dates_rejects_offsets_outside_representable_date_range() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();

        assert!(reminder_dates(start, &[i32::MAX]).is_err());
    }

    #[test]
    fn validate_targets_rejects_empty_targets() {
        assert!(validate_targets(&[]).is_err());
    }

    #[test]
    fn validate_targets_accepts_student_grade_target() {
        let grade_level_id = Uuid::new_v4();
        let targets = vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::Student,
            grade_level_id: Some(grade_level_id),
            class_room_id: None,
        }];

        assert!(validate_targets(&targets).is_ok());
    }

    #[test]
    fn validate_targets_rejects_student_or_parent_with_grade_and_classroom_filter() {
        let targets = vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::Parent,
            grade_level_id: Some(Uuid::new_v4()),
            class_room_id: Some(Uuid::new_v4()),
        }];

        assert!(validate_targets(&targets).is_err());
    }

    #[test]
    fn validate_targets_rejects_all_with_grade_or_classroom_filter() {
        let grade_targets = vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::All,
            grade_level_id: Some(Uuid::new_v4()),
            class_room_id: None,
        }];
        let classroom_targets = vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::All,
            grade_level_id: None,
            class_room_id: Some(Uuid::new_v4()),
        }];

        assert!(validate_targets(&grade_targets).is_err());
        assert!(validate_targets(&classroom_targets).is_err());
    }

    #[test]
    fn target_visible_to_user_type_allows_all_and_matching_user_type_only() {
        assert!(target_visible_to_user_type("all", "staff"));
        assert!(target_visible_to_user_type("all", "student"));
        assert!(!target_visible_to_user_type("all", "parent"));
        assert!(!target_visible_to_user_type("parent", "parent"));
        assert!(!target_visible_to_user_type("student", "parent"));
        assert!(!target_visible_to_user_type("parent", "student"));
    }

    #[test]
    fn self_calendar_user_type_access_rejects_parent_users() {
        assert!(self_calendar_user_type_access("staff").is_ok());
        assert!(self_calendar_user_type_access("student").is_ok());
        assert!(matches!(
            self_calendar_user_type_access("parent"),
            Err(AppError::Forbidden(message))
                if message.contains("/api/parent/students/{student_id}/calendar/events")
        ));
    }

    #[test]
    fn target_visible_to_child_view_allows_all_and_parent_only() {
        assert!(target_visible_to_child_view("all"));
        assert!(target_visible_to_child_view("parent"));
        assert!(!target_visible_to_child_view("student"));
        assert!(!target_visible_to_child_view("staff"));
    }

    #[test]
    fn validate_targets_rejects_duplicate_global_targets() {
        let targets = vec![
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Student,
                grade_level_id: None,
                class_room_id: None,
            },
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Student,
                grade_level_id: None,
                class_room_id: None,
            },
        ];

        assert!(validate_targets(&targets).is_err());
    }

    #[test]
    fn validate_targets_rejects_duplicate_grade_targets() {
        let grade_level_id = Uuid::new_v4();
        let targets = vec![
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Parent,
                grade_level_id: Some(grade_level_id),
                class_room_id: None,
            },
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Parent,
                grade_level_id: Some(grade_level_id),
                class_room_id: None,
            },
        ];

        assert!(validate_targets(&targets).is_err());
    }

    #[test]
    fn validate_targets_rejects_duplicate_classroom_targets() {
        let class_room_id = Uuid::new_v4();
        let targets = vec![
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Student,
                grade_level_id: None,
                class_room_id: Some(class_room_id),
            },
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Student,
                grade_level_id: None,
                class_room_id: Some(class_room_id),
            },
        ];

        assert!(validate_targets(&targets).is_err());
    }

    #[test]
    fn validate_targets_rejects_staff_with_classroom_filter() {
        let targets = vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::Staff,
            grade_level_id: None,
            class_room_id: Some(Uuid::new_v4()),
        }];

        assert!(validate_targets(&targets).is_err());
    }

    #[test]
    fn dedupe_user_ids_preserves_first_seen_order() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        assert_eq!(dedupe_user_ids(vec![a, b, a]), vec![a, b]);
    }

    #[test]
    fn dedupe_uuid_ids_keeps_each_tag_once_in_selection_order() {
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();

        assert_eq!(
            dedupe_uuid_ids(&[first, second, first]),
            vec![first, second]
        );
    }

    #[test]
    fn normalized_tag_name_trims_and_rejects_blank_values() {
        assert_eq!(
            normalized_tag_name("  กิจกรรมเด่น  ".to_string()).unwrap(),
            "กิจกรรมเด่น"
        );
        assert!(normalized_tag_name("   ".to_string()).is_err());
    }

    #[test]
    fn normalized_event_range_uses_complete_query_range() {
        let from = NaiveDate::from_ymd_opt(2026, 5, 2).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let query = CalendarEventQuery {
            from: Some(from),
            to: Some(to),
            category_id: None,
            tag_id: None,
            audience: None,
            visibility: None,
            q: None,
        };

        assert_eq!(normalized_event_range(&query, today).unwrap(), (from, to));
    }

    #[test]
    fn normalized_event_range_rejects_reversed_complete_query_range() {
        let from = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 5, 2).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let query = CalendarEventQuery {
            from: Some(from),
            to: Some(to),
            category_id: None,
            tag_id: None,
            audience: None,
            visibility: None,
            q: None,
        };

        assert!(matches!(
            normalized_event_range(&query, today),
            Err(AppError::BadRequest(_))
        ));
    }

    #[test]
    fn normalized_event_range_defaults_to_current_month_when_bound_is_missing() {
        let today = NaiveDate::from_ymd_opt(2026, 2, 10).unwrap();
        let query = CalendarEventQuery {
            from: Some(NaiveDate::from_ymd_opt(2026, 1, 5).unwrap()),
            to: None,
            category_id: None,
            tag_id: None,
            audience: None,
            visibility: None,
            q: None,
        };

        assert_eq!(
            normalized_event_range(&query, today).unwrap(),
            (
                NaiveDate::from_ymd_opt(2026, 2, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 2, 28).unwrap(),
            )
        );
    }

    #[test]
    fn normalized_event_range_handles_december_current_month() {
        let today = NaiveDate::from_ymd_opt(2026, 12, 15).unwrap();
        let query = CalendarEventQuery {
            from: None,
            to: None,
            category_id: None,
            tag_id: None,
            audience: None,
            visibility: None,
            q: None,
        };

        assert_eq!(
            normalized_event_range(&query, today).unwrap(),
            (
                NaiveDate::from_ymd_opt(2026, 12, 1).unwrap(),
                NaiveDate::from_ymd_opt(2026, 12, 31).unwrap(),
            )
        );
    }

    #[test]
    fn calendar_search_pattern_escapes_like_wildcards() {
        assert_eq!(
            calendar_search_pattern(" 100%_activity\\plan "),
            "%100\\%\\_activity\\\\plan%"
        );
    }

    #[test]
    fn calendar_notification_text_formats_created_updated_and_reminder_messages() {
        let event = calendar_event_for_notification("สอบกลางภาค");

        assert_eq!(
            calendar_notification_text(&event, &CalendarNotificationKind::Created),
            (
                "เพิ่มกำหนดการ: สอบกลางภาค".to_string(),
                "มีกำหนดการใหม่ในปฏิทินโรงเรียน".to_string(),
            )
        );
        assert_eq!(
            calendar_notification_text(&event, &CalendarNotificationKind::Updated),
            (
                "อัปเดตกำหนดการ: สอบกลางภาค".to_string(),
                "มีการเปลี่ยนแปลงกำหนดการในปฏิทินโรงเรียน".to_string(),
            )
        );
        assert_eq!(
            calendar_notification_text(
                &event,
                &CalendarNotificationKind::Reminder { days_before: 3 },
            ),
            (
                "เตือนล่วงหน้า: สอบกลางภาค".to_string(),
                "กำหนดการนี้จะเริ่มในอีก 3 วัน".to_string(),
            )
        );
    }

    #[test]
    fn calendar_notification_link_matches_supported_user_types() {
        assert_eq!(
            calendar_notification_link_for_user_type("staff"),
            Some("/staff/calendar")
        );
        assert_eq!(
            calendar_notification_link_for_user_type("student"),
            Some("/student/calendar")
        );
        assert_eq!(
            calendar_notification_link_for_user_type("parent"),
            Some("/parent")
        );
        assert_eq!(calendar_notification_link_for_user_type("guest"), None);
    }

    #[test]
    fn reminder_advisory_lock_keys_are_stable_from_uuid_bytes() {
        let reminder_id = Uuid::parse_str("01020304-0506-0708-090a-0b0c0d0e0f10").unwrap();

        assert_eq!(
            calendar_reminder_advisory_lock_keys(reminder_id),
            (16_909_060, 84_281_096)
        );
    }

    #[test]
    fn due_reminder_candidate_query_does_not_lock_or_mark_sent() {
        let sql = select_due_calendar_reminder_candidates_sql();

        assert!(sql.contains("LIMIT 200"));
        assert!(sql.contains("sent_at IS NULL"));
        assert!(sql.contains("$2::uuid[]"));
        assert!(!sql.contains("FOR UPDATE"));
        assert!(!sql.contains("SET sent_at"));
    }

    #[test]
    fn due_reminder_candidate_query_excludes_attempted_ids_for_batching() {
        let sql = select_due_calendar_reminder_candidates_sql();

        assert!(sql.contains("NOT (id = ANY($2::uuid[]))"));
        assert!(sql.contains("LIMIT 200"));
    }

    #[test]
    fn due_reminder_mark_query_sets_sent_after_attempt() {
        let sql = mark_calendar_reminder_sent_sql();

        assert!(sql.contains("UPDATE calendar_event_reminders"));
        assert!(sql.contains("SET sent_at = NOW()"));
        assert!(sql.contains("WHERE id = $1 AND sent_at IS NULL"));
    }

    #[test]
    fn notification_outcome_marks_reminders_sent_only_when_none_or_some_success() {
        assert!(CalendarNotificationSendOutcome {
            recipient_count: 0,
            successful_count: 0,
            failed_count: 0,
        }
        .should_mark_reminder_sent());
        assert!(CalendarNotificationSendOutcome {
            recipient_count: 2,
            successful_count: 1,
            failed_count: 1,
        }
        .should_mark_reminder_sent());
        assert!(!CalendarNotificationSendOutcome {
            recipient_count: 2,
            successful_count: 0,
            failed_count: 2,
        }
        .should_mark_reminder_sent());
    }

    #[test]
    fn advisory_lock_queries_use_two_integer_keys() {
        assert!(try_calendar_reminder_advisory_lock_sql().contains("pg_try_advisory_lock($1, $2)"));
        assert!(
            release_calendar_reminder_advisory_lock_sql().contains("pg_advisory_unlock($1, $2)")
        );
    }

    fn calendar_event_for_notification(title: &str) -> CalendarEvent {
        let now = Utc::now();
        let date = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();

        CalendarEvent {
            id: Uuid::new_v4(),
            category_id: None,
            category_name: None,
            category_color: None,
            title: title.to_string(),
            description: None,
            location: None,
            start_date: date,
            end_date: date,
            all_day: true,
            start_time: None,
            end_time: None,
            is_public: false,
            tags: Vec::new(),
            targets: Vec::new(),
            reminders: Vec::new(),
            created_by: None,
            updated_by: None,
            created_at: now,
            updated_at: now,
        }
    }
}
