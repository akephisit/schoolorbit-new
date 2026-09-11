use chrono::NaiveDate;
use sqlx::{PgPool, Postgres, QueryBuilder, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::core::services::lifecycle_guard;
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
    require_context_writes(
        &mut transaction,
        &[(payload.academic_year_id, payload.academic_term_id)],
    )
    .await?;
    validate_event_context(
        &mut transaction,
        payload.academic_year_id,
        payload.academic_term_id,
        payload.start_date,
        payload.end_date,
    )
    .await?;
    let event_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        INSERT INTO calendar_events (
            academic_year_id, academic_term_id,
            category_id, title, description, location, start_date, end_date,
            all_day, start_time, end_time, is_public, created_by, updated_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $13)
        RETURNING id
        "#,
    )
    .bind(payload.academic_year_id)
    .bind(payload.academic_term_id)
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

    replace_event_targets(
        &mut transaction,
        event_id,
        payload.academic_year_id,
        &payload.targets,
    )
    .await?;
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
    let existing = require_event_write(
        &mut transaction,
        id,
        Some((payload.academic_year_id, payload.academic_term_id)),
    )
    .await?;
    validate_event_context(
        &mut transaction,
        payload.academic_year_id,
        payload.academic_term_id,
        payload.start_date,
        payload.end_date,
    )
    .await?;
    if existing.0 != payload.academic_year_id {
        // Targets have an immediate composite FK to the event's year. Remove
        // them only after all guards; the transaction restores them on failure.
        sqlx::query("DELETE FROM calendar_event_targets WHERE event_id=$1")
            .bind(id)
            .execute(&mut *transaction)
            .await?;
    }
    let event_id = sqlx::query_scalar::<_, Uuid>(
        r#"
        UPDATE calendar_events
        SET
            academic_year_id = $1,
            academic_term_id = $2,
            category_id = $3,
            title = $4,
            description = $5,
            location = $6,
            start_date = $7,
            end_date = $8,
            all_day = $9,
            start_time = $10,
            end_time = $11,
            is_public = $12,
            updated_by = $13
        WHERE id = $14 AND deleted_at IS NULL
        RETURNING id
        "#,
    )
    .bind(payload.academic_year_id)
    .bind(payload.academic_term_id)
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
    replace_event_targets(
        &mut transaction,
        event_id,
        payload.academic_year_id,
        &payload.targets,
    )
    .await?;
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
    require_event_write(&mut transaction, id, None).await?;
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

type EventContext = (Uuid, Option<Uuid>);

async fn require_context_writes(
    tx: &mut Transaction<'_, Postgres>,
    contexts: &[EventContext],
) -> Result<(), AppError> {
    // All years precede all terms. Opposing cross-year moves use the same order.
    let years: std::collections::BTreeSet<_> = contexts.iter().map(|context| context.0).collect();
    for year in years {
        lifecycle_guard::require_year_write_exclusive(tx, year).await?;
    }
    let terms: std::collections::BTreeSet<_> = contexts
        .iter()
        .filter_map(|(year, term)| term.map(|term| (*year, term)))
        .collect();
    for (year, term) in terms {
        lifecycle_guard::require_term_write(tx, year, term).await?;
    }
    Ok(())
}

async fn require_event_write(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    target: Option<EventContext>,
) -> Result<EventContext, AppError> {
    lifecycle_guard::lock_transition_shared(tx).await?;
    let before: EventContext = sqlx::query_as(
        "SELECT academic_year_id, academic_term_id FROM calendar_events WHERE id=$1 AND deleted_at IS NULL",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound(EVENT_NOT_FOUND_MESSAGE.into()))?;
    let contexts = match target {
        Some(target) => vec![before, target],
        None => vec![before],
    };
    require_context_writes(tx, &contexts).await?;
    let current: EventContext = sqlx::query_as(
        "SELECT academic_year_id, academic_term_id FROM calendar_events WHERE id=$1 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound(EVENT_NOT_FOUND_MESSAGE.into()))?;
    if current != before {
        return Err(AppError::Conflict(
            "กิจกรรมถูกย้ายปีหรือภาคเรียนแล้ว กรุณาโหลดข้อมูลใหม่".into(),
        ));
    }
    Ok(current)
}

async fn replace_event_targets(
    transaction: &mut Transaction<'_, Postgres>,
    event_id: Uuid,
    academic_year_id: Uuid,
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
        "INSERT INTO calendar_event_targets (
            event_id, academic_year_id, audience_type, grade_level_id, homeroom_id
        ) ",
    );
    builder.push_values(targets, |mut row, target| {
        row.push_bind(event_id)
            .push_bind(academic_year_id)
            .push_bind(target.audience_type.as_str())
            .push_bind(target.grade_level_id)
            .push_bind(target.homeroom_id);
    });
    builder.build().execute(&mut **transaction).await?;

    Ok(())
}

async fn validate_event_context(
    transaction: &mut Transaction<'_, Postgres>,
    academic_year_id: Uuid,
    academic_term_id: Option<Uuid>,
    start_date: NaiveDate,
    end_date: NaiveDate,
) -> Result<(), AppError> {
    let year_contains_dates: bool = sqlx::query_scalar(
        "SELECT EXISTS (
            SELECT 1 FROM academic_years
            WHERE id = $1 AND start_date <= $2 AND end_date >= $3
        )",
    )
    .bind(academic_year_id)
    .bind(start_date)
    .bind(end_date)
    .fetch_one(&mut **transaction)
    .await?;
    if !year_contains_dates {
        return Err(AppError::BadRequest(
            "ช่วงวันที่กำหนดการไม่อยู่ในปีการศึกษาที่เลือก".to_string(),
        ));
    }

    if let Some(academic_term_id) = academic_term_id {
        let term_contains_dates: bool = sqlx::query_scalar(
            "SELECT EXISTS (
                SELECT 1 FROM academic_terms
                WHERE id = $1
                  AND academic_year_id = $2
                  AND start_date <= $3
                  AND (planned_end_date IS NULL OR planned_end_date >= $4)
            )",
        )
        .bind(academic_term_id)
        .bind(academic_year_id)
        .bind(start_date)
        .bind(end_date)
        .fetch_one(&mut **transaction)
        .await?;
        if !term_contains_dates {
            return Err(AppError::BadRequest(
                "ช่วงวันที่กำหนดการไม่อยู่ในภาคเรียนที่เลือก".to_string(),
            ));
        }
    }

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
