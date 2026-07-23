use chrono::NaiveDate;
use sqlx::{PgPool, Postgres, QueryBuilder, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::calendar::models::{
    CalendarEvent, CalendarEventTargetInput, UpsertCalendarEventRequest,
};

use super::shared::{
    dedupe_uuid_ids, reminder_schedule, validate_event_date_time, validate_targets,
    EVENT_NOT_FOUND_MESSAGE,
};
use super::visibility::get_event_for_response;
use super::CalendarNotificationKind;

const INVALID_TAGS_MESSAGE: &str = "มีแท็กที่ไม่ถูกต้อง กรุณาเลือกแท็กใหม่";

#[derive(Debug, Clone)]
pub struct CalendarEventMutationOutcome {
    pub event: CalendarEvent,
    pub notify_audience: bool,
    pub notification_kind: CalendarNotificationKind,
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
