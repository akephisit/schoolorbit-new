use chrono::{Datelike, Duration, Months, NaiveDate, NaiveTime, Utc};
use sqlx::{PgPool, Postgres, QueryBuilder, Transaction};
use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::calendar::models::{
    CalendarAudienceType, CalendarCategory, CalendarEvent, CalendarEventQuery,
    CalendarEventReminder, CalendarEventRow, CalendarEventTarget, CalendarEventTargetInput,
    CalendarVisibility, UpsertCalendarCategoryRequest, UpsertCalendarEventRequest,
};

const DUPLICATE_CATEGORY_MESSAGE: &str = "มีหมวดหมู่นี้อยู่แล้ว";
const EVENT_NOT_FOUND_MESSAGE: &str = "ไม่พบกำหนดการ";
const CATEGORY_NOT_FOUND_MESSAGE: &str = "ไม่พบหมวดหมู่";

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

async fn hydrate_events(
    pool: &PgPool,
    rows: Vec<CalendarEventRow>,
) -> Result<Vec<CalendarEvent>, AppError> {
    let event_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let targets = list_targets_for_events(pool, &event_ids).await?;
    let reminders = list_reminders_for_events(pool, &event_ids).await?;

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

pub async fn deactivate_category(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    let result = sqlx::query(
        r#"
        UPDATE calendar_categories
        SET is_active = false
        WHERE id = $1 AND is_active = true
        "#,
    )
    .bind(id)
    .execute(pool)
    .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(CATEGORY_NOT_FOUND_MESSAGE.to_string()));
    }

    Ok(())
}

pub async fn create_event(
    pool: &PgPool,
    actor_user_id: Uuid,
    payload: UpsertCalendarEventRequest,
) -> Result<CalendarEvent, AppError> {
    validate_event_date_time(
        payload.start_date,
        payload.end_date,
        payload.all_day,
        payload.start_time,
        payload.end_time,
    )?;
    validate_targets(&payload.targets)?;
    let reminder_pairs = reminder_schedule(payload.start_date, &payload.reminder_offsets_days)?;

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
    replace_pending_event_reminders(&mut transaction, event_id, reminder_pairs).await?;
    transaction.commit().await?;

    get_event_for_response(pool, event_id).await
}

pub async fn update_event(
    pool: &PgPool,
    actor_user_id: Uuid,
    id: Uuid,
    payload: UpsertCalendarEventRequest,
) -> Result<CalendarEvent, AppError> {
    validate_event_date_time(
        payload.start_date,
        payload.end_date,
        payload.all_day,
        payload.start_time,
        payload.end_time,
    )?;
    validate_targets(&payload.targets)?;
    let reminder_pairs = reminder_schedule(payload.start_date, &payload.reminder_offsets_days)?;

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
    replace_pending_event_reminders(&mut transaction, event_id, reminder_pairs).await?;
    transaction.commit().await?;

    get_event_for_response(pool, event_id).await
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

    if let Some(category_id) = query.category_id {
        builder.push(" AND e.category_id = ");
        builder.push_bind(category_id);
    }

    if let Some(audience) = &query.audience {
        builder.push(
            " AND EXISTS (
                SELECT 1 FROM calendar_event_targets target
                WHERE target.event_id = e.id AND target.audience_type = ",
        );
        builder.push_bind(audience.as_str());
        builder.push(")");
    }

    match query.visibility {
        Some(CalendarVisibility::Public) => {
            builder.push(" AND e.is_public = true");
        }
        Some(CalendarVisibility::Private) => {
            builder.push(" AND e.is_public = false");
        }
        None => {}
    }

    push_search_filter(&mut builder, query.q.as_deref());
    push_event_order(&mut builder);

    let rows = builder
        .build_query_as::<CalendarEventRow>()
        .fetch_all(pool)
        .await?;
    hydrate_events(pool, rows).await
}

pub async fn list_public_events(
    pool: &PgPool,
    query: CalendarEventQuery,
) -> Result<Vec<CalendarEvent>, AppError> {
    let (from, to) = normalized_event_range(&query, tenant_today())?;
    let mut builder = QueryBuilder::<Postgres>::new(EVENT_SELECT_WITH_CATEGORY);
    push_base_event_filters(&mut builder, from, to);
    builder.push(" AND e.is_public = true");

    if let Some(category_id) = query.category_id {
        builder.push(" AND e.category_id = ");
        builder.push_bind(category_id);
    }

    push_search_filter(&mut builder, query.q.as_deref());
    push_event_order(&mut builder);

    let rows = builder
        .build_query_as::<CalendarEventRow>()
        .fetch_all(pool)
        .await?;
    let mut events = hydrate_events(pool, rows).await?;
    for event in &mut events {
        event.targets.clear();
        event.reminders.clear();
    }

    Ok(events)
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
    builder.push_bind(pattern);
    builder.push(" ESCAPE '\\')");
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

fn is_duplicate_active_category_name(error: &sqlx::Error) -> bool {
    let sqlx::Error::Database(database_error) = error else {
        return false;
    };

    if database_error.code().as_deref() != Some("23505") {
        return false;
    }

    let constraint_matches = database_error
        .constraint()
        .map(|constraint| constraint == "idx_calendar_categories_active_name_unique")
        .unwrap_or(false);
    let message_matches = database_error
        .message()
        .contains("idx_calendar_categories_active_name_unique");

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
    fn normalized_event_range_uses_complete_query_range() {
        let from = NaiveDate::from_ymd_opt(2026, 5, 2).unwrap();
        let to = NaiveDate::from_ymd_opt(2026, 5, 6).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let query = CalendarEventQuery {
            from: Some(from),
            to: Some(to),
            category_id: None,
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
}
