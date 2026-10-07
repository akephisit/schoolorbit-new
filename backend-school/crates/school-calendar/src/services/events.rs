use chrono::NaiveDate;
use sqlx::{PgPool, Postgres, QueryBuilder, Transaction};
use uuid::Uuid;

use crate::models::{CalendarEvent, CalendarEventTargetInput, UpsertCalendarEventRequest};
use school_errors::AppError;

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
    let mut transaction = pool.begin().await?;
    let id = create_event_in_transaction(&mut transaction, actor_user_id, &payload).await?;
    transaction.commit().await?;
    event_outcome(
        pool,
        id,
        payload.notify_audience,
        CalendarNotificationKind::Created,
    )
    .await
}

pub(crate) async fn create_event_in_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    payload: &UpsertCalendarEventRequest,
) -> Result<Uuid, AppError> {
    validate_event_payload(payload)?;
    validate_category(transaction, payload.category_id).await?;
    let event_id = sqlx::query_scalar::<_, Uuid>(
        "INSERT INTO calendar_events (category_id,title,description,location,start_date,end_date,
            all_day,start_time,end_time,is_public,created_by,updated_by)
         VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$11) RETURNING id",
    )
    .bind(payload.category_id)
    .bind(payload.title.trim())
    .bind(&payload.description)
    .bind(&payload.location)
    .bind(payload.start_date)
    .bind(payload.end_date)
    .bind(payload.all_day)
    .bind(if payload.all_day {
        None
    } else {
        payload.start_time
    })
    .bind(if payload.all_day {
        None
    } else {
        payload.end_time
    })
    .bind(payload.is_public)
    .bind(actor_user_id)
    .fetch_one(&mut **transaction)
    .await?;
    replace_event_configuration(transaction, event_id, payload).await?;
    Ok(event_id)
}

fn validate_event_payload(payload: &UpsertCalendarEventRequest) -> Result<(), AppError> {
    super::shared::validate_event_text(
        &payload.title,
        payload.description.as_deref(),
        payload.location.as_deref(),
    )?;
    validate_event_date_time(
        payload.start_date,
        payload.end_date,
        payload.all_day,
        payload.start_time,
        payload.end_time,
    )?;
    validate_targets(&payload.targets)?;
    reminder_schedule(payload.start_date, &payload.reminder_offsets_days)?;
    Ok(())
}

async fn validate_category(
    transaction: &mut Transaction<'_, Postgres>,
    category_id: Option<Uuid>,
) -> Result<(), AppError> {
    if let Some(category) = category_id {
        let valid: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM calendar_categories WHERE id=$1 AND is_active)",
        )
        .bind(category)
        .fetch_one(&mut **transaction)
        .await?;
        if !valid {
            return Err(AppError::BadRequest("หมวดหมู่ไม่พร้อมใช้งาน กรุณาเลือกใหม่".into()));
        }
    }
    Ok(())
}

async fn replace_event_configuration(
    transaction: &mut Transaction<'_, Postgres>,
    event_id: Uuid,
    payload: &UpsertCalendarEventRequest,
) -> Result<(), AppError> {
    replace_event_targets(transaction, event_id, payload.start_date, &payload.targets).await?;
    replace_event_tags(transaction, event_id, &dedupe_uuid_ids(&payload.tag_ids)).await?;
    replace_pending_event_reminders(
        transaction,
        event_id,
        reminder_schedule(payload.start_date, &payload.reminder_offsets_days)?,
    )
    .await
}

async fn event_outcome(
    pool: &PgPool,
    id: Uuid,
    notify_audience: bool,
    notification_kind: CalendarNotificationKind,
) -> Result<CalendarEventMutationOutcome, AppError> {
    Ok(CalendarEventMutationOutcome {
        event: get_event_for_response(pool, id).await?,
        notify_audience,
        notification_kind,
    })
}

pub async fn update_event(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    payload: UpsertCalendarEventRequest,
) -> Result<CalendarEventMutationOutcome, AppError> {
    validate_event_payload(&payload)?;
    let mut transaction = pool.begin().await?;
    let exists = sqlx::query_scalar::<_, Uuid>(
        "SELECT id FROM calendar_events WHERE id=$1 AND deleted_at IS NULL FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *transaction)
    .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(EVENT_NOT_FOUND_MESSAGE.into()));
    }
    validate_category(&mut transaction, payload.category_id).await?;
    sqlx::query("UPDATE calendar_events SET category_id=$1,title=$2,description=$3,location=$4,start_date=$5,end_date=$6,
        all_day=$7,start_time=$8,end_time=$9,is_public=$10,updated_by=$11 WHERE id=$12")
        .bind(payload.category_id).bind(payload.title.trim()).bind(&payload.description).bind(&payload.location)
        .bind(payload.start_date).bind(payload.end_date).bind(payload.all_day)
        .bind(if payload.all_day { None } else { payload.start_time })
        .bind(if payload.all_day { None } else { payload.end_time }).bind(payload.is_public).bind(actor_user_id).bind(id)
        .execute(&mut *transaction).await?;
    replace_event_configuration(&mut transaction, id, &payload).await?;
    transaction.commit().await?;
    event_outcome(
        pool,
        id,
        payload.notify_audience,
        CalendarNotificationKind::Updated,
    )
    .await
}

pub async fn soft_delete_event(
    pool: &PgPool,
    id: Uuid,
    actor_user_id: Uuid,
) -> Result<(), AppError> {
    let mut transaction = pool.begin().await?;
    let result=sqlx::query("UPDATE calendar_events SET deleted_at=NOW(),updated_by=$2 WHERE id=$1 AND deleted_at IS NULL")
        .bind(id).bind(actor_user_id).execute(&mut *transaction).await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(EVENT_NOT_FOUND_MESSAGE.into()));
    }
    sqlx::query("DELETE FROM calendar_event_reminders WHERE event_id=$1 AND sent_at IS NULL")
        .bind(id)
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    Ok(())
}

async fn replace_event_targets(
    transaction: &mut Transaction<'_, Postgres>,
    event_id: Uuid,
    event_date: NaiveDate,
    targets: &[CalendarEventTargetInput],
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM calendar_event_targets WHERE event_id = $1")
        .bind(event_id)
        .execute(&mut **transaction)
        .await?;

    if targets.is_empty() {
        return Ok(());
    }

    let year_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM academic_years WHERE start_date <= $1 AND end_date >= $1 ORDER BY id",
    )
    .bind(event_date)
    .fetch_all(&mut **transaction)
    .await?;
    let mut scoped_targets = Vec::new();
    for target in targets {
        let year = if let Some(room) = target.homeroom_id {
            Some(
                sqlx::query_scalar::<_, Uuid>(
                    "SELECT room.academic_year_id FROM homerooms room JOIN academic_years year ON year.id=room.academic_year_id WHERE room.id=$1 AND room.is_active AND year.start_date <= $2 AND year.end_date >= $2",
                )
                .bind(room)
                .bind(event_date)
                .fetch_optional(&mut **transaction)
                .await?
                .ok_or_else(|| AppError::BadRequest("ห้องเรียนไม่พร้อมใช้งาน".into()))?,
            )
        } else if let Some(grade) = target.grade_level_id {
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM grade_levels WHERE id=$1 AND is_active)",
            )
            .bind(grade)
            .fetch_one(&mut **transaction)
            .await?;
            if !valid {
                return Err(AppError::BadRequest(
                    "ไม่พบปีของระดับชั้นในวันที่กิจกรรม กรุณาเลือกห้องเรียนหรือทุกระดับชั้น".into(),
                ));
            }
            if year_ids.len() == 1 {
                year_ids.first().copied()
            } else {
                None
            }
        } else {
            None
        };
        scoped_targets.push((target, year));
    }
    let mut builder = QueryBuilder::<Postgres>::new(
        "INSERT INTO calendar_event_targets (
            event_id, academic_year_id, audience_type, grade_level_id, homeroom_id
        ) ",
    );
    builder.push_values(scoped_targets, |mut row, (target, year)| {
        row.push_bind(event_id)
            .push_bind(year)
            .push_bind(target.audience_type.as_str())
            .push_bind(target.grade_level_id)
            .push_bind(target.homeroom_id);
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
