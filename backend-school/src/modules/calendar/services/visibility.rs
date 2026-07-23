use std::collections::HashMap;

use chrono::{NaiveDate, Utc};
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::calendar::models::{
    CalendarAudienceType, CalendarEvent, CalendarEventQuery, CalendarEventReminder,
    CalendarEventRow, CalendarEventTag, CalendarEventTarget, CalendarPublicEvent,
    CalendarViewerEvent, CalendarVisibility,
};

use super::shared::{normalized_event_range, tenant_today, EVENT_NOT_FOUND_MESSAGE};

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

pub(super) async fn get_event_for_response(
    pool: &PgPool,
    event_id: Uuid,
) -> Result<CalendarEvent, AppError> {
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

pub(super) fn target_visible_to_user_type(audience_type: &str, user_type: &str) -> bool {
    match user_type {
        "staff" | "student" => {
            audience_type == CalendarAudienceType::All.as_str() || audience_type == user_type
        }
        _ => false,
    }
}

pub(super) fn target_visible_to_child_view(audience_type: &str) -> bool {
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

pub(super) fn self_calendar_user_type_access(user_type: &str) -> Result<(), AppError> {
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

pub(super) fn calendar_search_pattern(search: &str) -> String {
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
