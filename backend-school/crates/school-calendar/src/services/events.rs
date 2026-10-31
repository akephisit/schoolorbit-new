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
    let mut transaction =
        school_tenancy::TenantPool::begin_observed(&pool, "calendar.events.create_event").await?;
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
    let event_id = school_database::DbSession::new(&mut **transaction).scalar::<Uuid>("calendar.services.events.create_event_in_transaction",
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
    .fetch_one()
    .await?;
    replace_event_configuration(transaction, event_id, payload).await?;
    accept_mutation_notifications(
        transaction,
        event_id,
        payload.notify_audience,
        CalendarNotificationKind::Created,
    )
    .await?;
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
        let valid: bool = school_database::DbSession::new(&mut **transaction)
            .scalar(
                "calendar.services.events.validate_category",
                "SELECT EXISTS(SELECT 1 FROM calendar_categories WHERE id=$1 AND is_active)",
            )
            .bind(category)
            .fetch_one()
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
    replace_event_targets(transaction, event_id, &payload.targets).await?;
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
    let mut transaction =
        school_tenancy::TenantPool::begin_observed(&pool, "calendar.events.update_event").await?;
    let exists = school_database::DbSession::new(&mut *transaction)
        .scalar::<Uuid>(
            "calendar.services.events.update_event",
            "SELECT id FROM calendar_events WHERE id=$1 AND deleted_at IS NULL FOR UPDATE",
        )
        .bind(id)
        .fetch_optional()
        .await?;
    if exists.is_none() {
        return Err(AppError::NotFound(EVENT_NOT_FOUND_MESSAGE.into()));
    }
    validate_category(&mut transaction, payload.category_id).await?;
    school_database::DbSession::new(&mut *transaction).query("calendar.services.events.update_event", "UPDATE calendar_events SET category_id=$1,title=$2,description=$3,location=$4,start_date=$5,end_date=$6,
        all_day=$7,start_time=$8,end_time=$9,is_public=$10,updated_by=$11 WHERE id=$12")
        .bind(payload.category_id).bind(payload.title.trim()).bind(&payload.description).bind(&payload.location)
        .bind(payload.start_date).bind(payload.end_date).bind(payload.all_day)
        .bind(if payload.all_day { None } else { payload.start_time })
        .bind(if payload.all_day { None } else { payload.end_time }).bind(payload.is_public).bind(actor_user_id).bind(id)
        .execute().await?;
    replace_event_configuration(&mut transaction, id, &payload).await?;
    accept_mutation_notifications(
        &mut transaction,
        id,
        payload.notify_audience,
        CalendarNotificationKind::Updated,
    )
    .await?;
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
    let mut transaction =
        school_tenancy::TenantPool::begin_observed(&pool, "calendar.events.soft_delete_event")
            .await?;
    let result=school_database::DbSession::new(&mut *transaction).query("calendar.services.events.soft_delete_event", "UPDATE calendar_events SET deleted_at=NOW(),updated_by=$2 WHERE id=$1 AND deleted_at IS NULL")
        .bind(id).bind(actor_user_id).execute().await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(EVENT_NOT_FOUND_MESSAGE.into()));
    }
    school_database::DbSession::new(&mut *transaction)
        .query(
            "calendar.services.events.soft_delete_event",
            "DELETE FROM calendar_event_reminders WHERE event_id=$1 AND sent_at IS NULL",
        )
        .bind(id)
        .execute()
        .await?;
    transaction.commit().await?;
    Ok(())
}

async fn replace_event_targets(
    transaction: &mut Transaction<'_, Postgres>,
    event_id: Uuid,
    targets: &[CalendarEventTargetInput],
) -> Result<(), AppError> {
    school_database::DbSession::new(&mut **transaction)
        .query(
            "calendar.services.events.replace_event_targets",
            "DELETE FROM calendar_event_targets WHERE event_id = $1",
        )
        .bind(event_id)
        .execute()
        .await?;

    if targets.is_empty() {
        return Ok(());
    }

    let invalid:bool=school_database::DbSession::new(&mut **transaction).scalar("calendar.services.events.validate_target_contexts",r#"
        SELECT EXISTS(SELECT 1 FROM jsonb_to_recordset($1) target("academicYearId" uuid,"gradeLevelId" uuid,"homeroomId" uuid)
        WHERE ((target."gradeLevelId" IS NOT NULL OR target."homeroomId" IS NOT NULL) AND target."academicYearId" IS NULL)
           OR (target."academicYearId" IS NOT NULL AND NOT EXISTS(SELECT 1 FROM academic_years WHERE id=target."academicYearId"))
           OR (target."gradeLevelId" IS NOT NULL AND NOT EXISTS(SELECT 1 FROM grade_levels WHERE id=target."gradeLevelId" AND is_active))
           OR (target."homeroomId" IS NOT NULL AND NOT EXISTS(SELECT 1 FROM homerooms room WHERE room.id=target."homeroomId" AND room.is_active AND room.academic_year_id=target."academicYearId")))
    "#).bind(sqlx::types::Json(targets)).fetch_one().await?;
    if invalid {
        return Err(AppError::BadRequest(
            "ระบุปีของกลุ่มเป้าหมายให้ตรงกับระดับชั้นหรือห้องเรียน".into(),
        ));
    }
    let mut builder = QueryBuilder::<Postgres>::new(
        "INSERT INTO calendar_event_targets (
            event_id, academic_year_id, audience_type, grade_level_id, homeroom_id
        ) ",
    );
    builder.push_values(targets, |mut row, target| {
        row.push_bind(event_id)
            .push_bind(target.academic_year_id)
            .push_bind(target.audience_type.as_str())
            .push_bind(target.grade_level_id)
            .push_bind(target.homeroom_id);
    });
    school_database::DbSession::new(&mut **transaction)
        .prepared_query(
            "calendar.services.events.replace_event_targets",
            builder.build(),
        )
        .execute()
        .await?;

    Ok(())
}

async fn replace_event_tags(
    transaction: &mut Transaction<'_, Postgres>,
    event_id: Uuid,
    tag_ids: &[Uuid],
) -> Result<(), AppError> {
    school_database::DbSession::new(&mut **transaction)
        .query(
            "calendar.services.events.replace_event_tags",
            "DELETE FROM calendar_event_tags WHERE event_id = $1",
        )
        .bind(event_id)
        .execute()
        .await?;

    if tag_ids.is_empty() {
        return Ok(());
    }

    let result = school_database::DbSession::new(&mut **transaction)
        .query(
            "calendar.services.events.replace_event_tags",
            r#"
        INSERT INTO calendar_event_tags (event_id, tag_id)
        SELECT $1, tags.id
        FROM calendar_tags tags
        WHERE tags.id = ANY($2::uuid[])
        "#,
        )
        .bind(event_id)
        .bind(tag_ids)
        .execute()
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
    school_database::DbSession::new(&mut **transaction)
        .query(
            "calendar.services.events.replace_pending_event_reminders",
            r#"
        DELETE FROM calendar_event_reminders
        WHERE event_id = $1 AND sent_at IS NULL
        "#,
        )
        .bind(event_id)
        .execute()
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
    school_database::DbSession::new(&mut **transaction)
        .prepared_query(
            "calendar.services.events.replace_pending_event_reminders",
            builder.build(),
        )
        .execute()
        .await?;

    Ok(())
}

async fn accept_mutation_notifications(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    notify: bool,
    kind: CalendarNotificationKind,
) -> Result<(), AppError> {
    if notify {
        let event = super::get_event_for_response_on_connection(tx, id).await?;
        crate::delivery::notifications::accept_event_in_transaction(tx, &event, &kind, None)
            .await?;
    }
    Ok(())
}
