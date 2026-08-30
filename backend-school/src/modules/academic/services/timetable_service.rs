use std::collections::{BTreeSet, HashMap};

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use serde::Serialize;
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::timetable::{
    BatchTimetableResult, ConflictInfo, CreateBatchTimetableEntriesRequest,
    CreateTimetableEntryRequest, MoveValidityCell, SwapTimetableEntriesRequest,
    SwapTimetableEntriesResponse, TimetableEntry, TimetableInstructor, TimetableOccupancyCell,
    TimetableQuery, TimetableValidationResponse, UpdateTimetableEntryRequest,
};
use crate::policies::resource_access_policy::AcademicResourceListFilter;

const VALID_DAYS: &[&str] = &["MON", "TUE", "WED", "THU", "FRI", "SAT", "SUN"];
const VALID_ENTRY_TYPES: &[&str] = &["COURSE", "ACTIVITY", "BREAK", "HOMEROOM", "ACADEMIC"];

#[derive(Debug, Clone, FromRow)]
struct TermContext {
    id: Uuid,
    academic_year_id: Uuid,
    bell_schedule_id: Uuid,
}

#[derive(Debug, Clone, FromRow)]
struct TimetableVersionContext {
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    bell_schedule_id: Uuid,
    effective_from: NaiveDate,
    status: String,
    term_status: String,
}

#[derive(Debug, Clone, FromRow)]
struct GroupContext {
    id: Uuid,
    learning_offering_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    offering_kind: String,
    offering_name: String,
}

#[derive(Debug, Clone, FromRow)]
struct EntryRow {
    id: Uuid,
    timetable_version_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    bell_schedule_id: Uuid,
    bell_schedule_period_id: Uuid,
    day_of_week: String,
    entry_type: String,
    learning_group_id: Option<Uuid>,
    learning_offering_id: Option<Uuid>,
    homeroom_id: Option<Uuid>,
    room_id: Option<Uuid>,
    note: Option<String>,
    title: Option<String>,
    batch_id: Option<Uuid>,
    row_version: i64,
    is_active: bool,
    offering_code: Option<String>,
    offering_name: Option<String>,
    learning_group_code: Option<String>,
    learning_group_name: Option<String>,
    subject_id: Option<Uuid>,
    subject_group_id: Option<Uuid>,
    subject_group_name: Option<String>,
    subject_group_display_order: Option<i32>,
    subject_version_display_label: Option<String>,
    activity_id: Option<Uuid>,
    activity_version_display_label: Option<String>,
    activity_scheduling_mode:
        Option<crate::modules::academic::delivery::models::ActivitySchedulingMode>,
    homeroom_name: Option<String>,
    room_code: Option<String>,
    period_name: Option<String>,
    start_time: NaiveTime,
    end_time: NaiveTime,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
struct EntryLockRow {
    id: Uuid,
    timetable_version_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    learning_group_id: Option<Uuid>,
    learning_offering_id: Option<Uuid>,
    homeroom_id: Option<Uuid>,
    room_id: Option<Uuid>,
    day_of_week: String,
    bell_schedule_period_id: Uuid,
    row_version: i64,
    is_active: bool,
}

#[derive(Debug, Clone, FromRow)]
struct SlotEntryRow {
    id: Uuid,
    learning_group_id: Option<Uuid>,
    homeroom_id: Option<Uuid>,
    room_id: Option<Uuid>,
    day_of_week: String,
    bell_schedule_period_id: Uuid,
}

#[derive(Debug, Clone)]
struct CandidateScope {
    learning_group_id: Option<Uuid>,
    homeroom_ids: Vec<Uuid>,
    instructor_ids: Vec<Uuid>,
    room_id: Option<Uuid>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TimetableInstructorAudit {
    instructor_id: Uuid,
    role: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TimetableEntryAuditSnapshot {
    day_of_week: String,
    bell_schedule_period_id: Uuid,
    room_id: Option<Uuid>,
    is_active: bool,
    row_version: i64,
    instructor_ids: Vec<Uuid>,
    instructors: Vec<TimetableInstructorAudit>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TimetableEntryAuditPayload {
    entry_id: Uuid,
    timetable_version_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    learning_offering_id: Option<Uuid>,
    learning_group_id: Option<Uuid>,
    actor_user_id: Uuid,
    old_row_version: i64,
    new_row_version: i64,
    before: TimetableEntryAuditSnapshot,
    after: TimetableEntryAuditSnapshot,
}

#[derive(Debug, Default)]
struct RelationshipIndexes {
    homerooms_by_group: HashMap<Uuid, Vec<Uuid>>,
    instructors_by_entry: HashMap<Uuid, Vec<Uuid>>,
}

impl RelationshipIndexes {
    fn homerooms(&self, learning_group_id: Option<Uuid>, homeroom_id: Option<Uuid>) -> Vec<Uuid> {
        match learning_group_id {
            Some(group_id) => self
                .homerooms_by_group
                .get(&group_id)
                .cloned()
                .unwrap_or_default(),
            None => homeroom_id.into_iter().collect(),
        }
    }

    fn instructors(&self, entry_id: Uuid) -> Vec<Uuid> {
        self.instructors_by_entry
            .get(&entry_id)
            .cloned()
            .unwrap_or_default()
    }
}

pub async fn list_entries(
    pool: &PgPool,
    query: &TimetableQuery,
    access: &AcademicResourceListFilter,
) -> Result<Vec<TimetableEntry>, AppError> {
    require_timetable_version(
        pool,
        query.timetable_version_id,
        Some(query.academic_term_id),
        false,
    )
    .await?;
    let entry_type = query
        .entry_type
        .as_deref()
        .map(normalize_entry_type)
        .transpose()?;
    let owner_ids = access.allowed_organization_unit_ids();
    let rows: Vec<EntryRow> = sqlx::query_as(&format!(
        r#"{}
           WHERE entry.timetable_version_id = $1
             AND entry.academic_term_id = $2
             AND entry.is_active
             AND ($3::uuid IS NULL OR entry.learning_group_id = $3)
             AND ($4::uuid IS NULL OR entry.homeroom_id = $4 OR EXISTS (
                 SELECT 1 FROM learning_group_homerooms coverage
                 WHERE coverage.learning_group_id = entry.learning_group_id
                   AND coverage.homeroom_id = $4
             ))
             AND ($5::uuid IS NULL OR EXISTS (
                 SELECT 1 FROM timetable_entry_instructors instructor
                 WHERE instructor.entry_id = entry.id AND instructor.instructor_id = $5
             ))
             AND ($6::uuid IS NULL OR entry.room_id = $6)
             AND ($7::text IS NULL OR entry.day_of_week = $7)
             AND ($8::text IS NULL OR entry.entry_type = $8)
             AND ($9 OR (
                 offering.id IS NOT NULL AND (
                     offering.owning_organization_unit_id = ANY($10)
                     OR EXISTS (
                         SELECT 1 FROM learning_group_teachers access_teacher
                         WHERE access_teacher.learning_group_id = entry.learning_group_id
                           AND access_teacher.teacher_id = $11
                     )
                 )
             ))
           ORDER BY period.order_index, entry.day_of_week, entry.id
           LIMIT 2000"#,
        entry_select()
    ))
    .bind(query.timetable_version_id)
    .bind(query.academic_term_id)
    .bind(query.learning_group_id)
    .bind(query.homeroom_id)
    .bind(query.instructor_id)
    .bind(query.room_id)
    .bind(
        query
            .day_of_week
            .as_deref()
            .map(normalize_day)
            .transpose()?,
    )
    .bind(entry_type)
    .bind(access.includes_school_owned)
    .bind(owner_ids)
    .bind(access.assigned_actor_id)
    .fetch_all(pool)
    .await?;
    hydrate_rows(pool, rows).await
}

pub async fn get_entry(pool: &PgPool, entry_id: Uuid) -> Result<TimetableEntry, AppError> {
    get_entries(pool, &[entry_id])
        .await?
        .pop()
        .ok_or_else(|| AppError::NotFound("ไม่พบรายการตารางสอน".to_string()))
}

pub(super) async fn get_entries(
    pool: &PgPool,
    entry_ids: &[Uuid],
) -> Result<Vec<TimetableEntry>, AppError> {
    if entry_ids.is_empty() {
        return Ok(Vec::new());
    }
    let rows: Vec<EntryRow> =
        sqlx::query_as(&format!("{} WHERE entry.id = ANY($1)", entry_select()))
            .bind(entry_ids)
            .fetch_all(pool)
            .await?;
    let mut entries_by_id: HashMap<Uuid, TimetableEntry> = hydrate_rows(pool, rows)
        .await?
        .into_iter()
        .map(|entry| (entry.id, entry))
        .collect();
    entry_ids
        .iter()
        .map(|entry_id| {
            entries_by_id
                .remove(entry_id)
                .ok_or_else(|| AppError::NotFound("ไม่พบรายการตารางสอน".to_string()))
        })
        .collect()
}

pub async fn list_student_entries(
    pool: &PgPool,
    timetable_version_id: Uuid,
    academic_term_id: Uuid,
    student_id: Uuid,
    on_date: chrono::NaiveDate,
) -> Result<Vec<TimetableEntry>, AppError> {
    require_timetable_version(pool, timetable_version_id, Some(academic_term_id), false).await?;
    let rows: Vec<EntryRow> = sqlx::query_as(&format!(
        r#"{}
           WHERE entry.timetable_version_id = $1
             AND entry.academic_term_id = $2
             AND entry.is_active
             AND (
                 EXISTS (
                     SELECT 1
                     FROM learning_group_students membership
                     JOIN student_academic_years student_year
                       ON student_year.id = membership.student_academic_year_id
                     JOIN learning_groups roster_group
                       ON roster_group.id = membership.learning_group_id
                     WHERE membership.learning_group_id = entry.learning_group_id
                       AND student_year.student_id = $3
                       AND membership.published_at IS NOT NULL
                       AND membership.joined_at <= $4
                       AND (membership.left_at IS NULL OR membership.left_at >= $4)
                       AND roster_group.roster_status IN ('published', 'closed')
                 )
                 OR (
                     entry.learning_group_id IS NULL
                     AND EXISTS (
                         SELECT 1
                         FROM student_academic_years student_year
                         JOIN homeroom_placements placement
                           ON placement.student_academic_year_id = student_year.id
                         WHERE student_year.student_id = $3
                           AND student_year.academic_year_id = entry.academic_year_id
                           AND placement.homeroom_id = entry.homeroom_id
                           AND placement.status = 'current'
                     )
                 )
             )
           ORDER BY period.order_index, entry.day_of_week, entry.id"#,
        entry_select()
    ))
    .bind(timetable_version_id)
    .bind(academic_term_id)
    .bind(student_id)
    .bind(on_date)
    .fetch_all(pool)
    .await?;
    hydrate_rows(pool, rows).await
}

pub async fn create_entry(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: CreateTimetableEntryRequest,
) -> Result<TimetableEntry, AppError> {
    create_entry_impl(pool, actor_user_id, request)
        .await
        .map_err(map_timetable_write_error)
}

async fn create_entry_impl(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: CreateTimetableEntryRequest,
) -> Result<TimetableEntry, AppError> {
    let mut transaction = pool.begin().await?;
    let entry_id = create_entry_in_tx(&mut transaction, actor_user_id, None, &request).await?;
    transaction.commit().await?;
    get_entry(pool, entry_id).await
}

pub async fn create_batch(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: CreateBatchTimetableEntriesRequest,
) -> Result<BatchTimetableResult, AppError> {
    create_batch_impl(pool, actor_user_id, request)
        .await
        .map_err(map_timetable_write_error)
}

async fn create_batch_impl(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: CreateBatchTimetableEntriesRequest,
) -> Result<BatchTimetableResult, AppError> {
    validate_batch_shape(&request)?;
    let batch_id = Uuid::new_v4();
    let mut transaction = pool.begin().await?;
    require_timetable_version_in_tx(
        &mut transaction,
        request.timetable_version_id,
        Some(request.academic_term_id),
        true,
    )
    .await?;

    let mut slot_keys = BTreeSet::new();
    for day in &request.days_of_week {
        let normalized_day = normalize_day(day)?;
        for period_id in &request.bell_schedule_period_ids {
            slot_keys.insert((normalized_day.clone(), *period_id));
        }
    }
    for (day, period_id) in slot_keys {
        lock_slot(
            &mut transaction,
            request.timetable_version_id,
            &day,
            period_id,
        )
        .await?;
    }

    let mut entry_ids = Vec::new();
    let targets: Vec<(Option<Uuid>, Option<Uuid>)> = if !request.learning_group_ids.is_empty() {
        request
            .learning_group_ids
            .iter()
            .copied()
            .map(|id| (Some(id), None))
            .collect()
    } else if !request.homeroom_ids.is_empty() {
        request
            .homeroom_ids
            .iter()
            .copied()
            .map(|id| (None, Some(id)))
            .collect()
    } else {
        vec![(None, None)]
    };
    for (learning_group_id, homeroom_id) in targets {
        for day in &request.days_of_week {
            for period_id in &request.bell_schedule_period_ids {
                let entry_request = CreateTimetableEntryRequest {
                    timetable_version_id: request.timetable_version_id,
                    academic_term_id: request.academic_term_id,
                    learning_group_id,
                    homeroom_id,
                    day_of_week: day.clone(),
                    bell_schedule_period_id: *period_id,
                    room_id: request.room_id,
                    note: request.note.clone(),
                    entry_type: request.entry_type.clone(),
                    title: request.title.clone(),
                    instructor_ids: request.instructor_ids.clone(),
                };
                entry_ids.push(
                    create_entry_in_tx(
                        &mut transaction,
                        actor_user_id,
                        Some(batch_id),
                        &entry_request,
                    )
                    .await?,
                );
            }
        }
    }
    transaction.commit().await?;
    let entries = get_entries(pool, &entry_ids).await?;
    Ok(BatchTimetableResult { batch_id, entries })
}

pub async fn update_entry(
    pool: &PgPool,
    entry_id: Uuid,
    actor_user_id: Uuid,
    request: UpdateTimetableEntryRequest,
) -> Result<TimetableEntry, AppError> {
    update_entry_impl(pool, entry_id, actor_user_id, request)
        .await
        .map_err(map_timetable_write_error)
}

async fn update_entry_impl(
    pool: &PgPool,
    entry_id: Uuid,
    actor_user_id: Uuid,
    request: UpdateTimetableEntryRequest,
) -> Result<TimetableEntry, AppError> {
    let mut transaction = pool.begin().await?;
    let existing = lock_entry(&mut transaction, entry_id).await?;
    if existing.row_version != request.row_version {
        return Err(stale_entry());
    }
    if existing.timetable_version_id != request.timetable_version_id {
        return Err(AppError::ValidationError(
            "รายการตารางสอนไม่อยู่ในรุ่นตารางที่ระบุ".to_string(),
        ));
    }
    let version = require_timetable_version_in_tx(
        &mut transaction,
        request.timetable_version_id,
        Some(existing.academic_term_id),
        true,
    )
    .await?;
    let day = request
        .day_of_week
        .as_deref()
        .map(normalize_day)
        .transpose()?
        .unwrap_or(existing.day_of_week.clone());
    let period_id = request
        .bell_schedule_period_id
        .unwrap_or(existing.bell_schedule_period_id);
    require_period_in_tx(&mut transaction, existing.academic_term_id, period_id).await?;
    lock_slots_stable(
        &mut transaction,
        existing.timetable_version_id,
        [
            (
                existing.day_of_week.clone(),
                existing.bell_schedule_period_id,
            ),
            (day.clone(), period_id),
        ],
    )
    .await?;
    let room_id = if request.clear_room.unwrap_or(false) {
        None
    } else {
        request.room_id.or(existing.room_id)
    };
    if let Some(room_id) = room_id {
        let room_exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM rooms WHERE id = $1 AND status = 'ACTIVE')",
        )
        .bind(room_id)
        .fetch_one(&mut *transaction)
        .await?;
        if !room_exists {
            return Err(AppError::ValidationError("ไม่พบห้องที่เปิดใช้งาน".to_string()));
        }
    }
    let before_instructors = entry_instructor_roles_in_tx(&mut transaction, entry_id).await?;
    let current_instructor_ids = before_instructors
        .iter()
        .map(|(instructor_id, _)| *instructor_id)
        .collect::<Vec<_>>();
    let proposed_instructor_ids = match request.instructor_ids.as_deref() {
        None => current_instructor_ids,
        Some(requested_ids) => match existing.learning_group_id {
            Some(group_id) => {
                eligible_group_instructors_in_tx(
                    &mut transaction,
                    group_id,
                    version.effective_from,
                    requested_ids,
                )
                .await?
            }
            None => eligible_structural_instructors_in_tx(&mut transaction, requested_ids).await?,
        },
    };
    if existing.learning_group_id.is_none()
        && existing.homeroom_id.is_none()
        && proposed_instructor_ids.is_empty()
    {
        return Err(AppError::ValidationError(
            "รายการโครงสร้างต้องระบุ homeroomId หรือ instructorIds".to_string(),
        ));
    }
    let mut scope = entry_candidate_scope(&mut transaction, &existing).await?;
    scope.room_id = room_id;
    scope.instructor_ids = proposed_instructor_ids.clone();
    ensure_no_conflicts(
        &mut transaction,
        existing.timetable_version_id,
        &day,
        period_id,
        &scope,
        &[entry_id],
    )
    .await?;
    let note = if request.clear_note.unwrap_or(false) {
        None
    } else {
        request.note
    };
    if request.instructor_ids.is_some() {
        let deactivated = sqlx::query(
            r#"UPDATE academic_timetable_entries
               SET is_active = false
               WHERE id = $1 AND row_version = $2 AND is_active"#,
        )
        .bind(entry_id)
        .bind(request.row_version)
        .execute(&mut *transaction)
        .await?;
        if deactivated.rows_affected() != 1 {
            return Err(stale_entry());
        }
        replace_entry_instructors_in_tx(&mut transaction, entry_id, &proposed_instructor_ids)
            .await?;
    }
    let updated = sqlx::query(
        r#"UPDATE academic_timetable_entries
           SET day_of_week = $2,
               bell_schedule_period_id = $3,
               room_id = $4,
               note = CASE WHEN $5 THEN NULL ELSE coalesce($6, note) END,
               title = coalesce($7, title),
               updated_by = $8,
               is_active = true,
               row_version = row_version + 1,
               updated_at = now()
           WHERE id = $1 AND row_version = $9"#,
    )
    .bind(entry_id)
    .bind(&day)
    .bind(period_id)
    .bind(room_id)
    .bind(request.clear_note.unwrap_or(false))
    .bind(note)
    .bind(request.title.as_deref())
    .bind(actor_user_id)
    .bind(request.row_version)
    .execute(&mut *transaction)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(stale_entry());
    }
    let after_instructors = if request.instructor_ids.is_some() {
        assigned_instructor_roles(&proposed_instructor_ids)
    } else {
        before_instructors.clone()
    };
    append_timetable_entry_audit_in_tx(
        &mut transaction,
        "academic_timetable_entry.updated",
        entry_id,
        existing.timetable_version_id,
        existing.academic_term_id,
        existing.academic_year_id,
        existing.learning_offering_id,
        existing.learning_group_id,
        actor_user_id,
        entry_audit_snapshot(
            &existing.day_of_week,
            existing.bell_schedule_period_id,
            existing.room_id,
            true,
            existing.row_version,
            &before_instructors,
        ),
        entry_audit_snapshot(
            &day,
            period_id,
            room_id,
            true,
            existing.row_version + 1,
            &after_instructors,
        ),
    )
    .await?;
    transaction.commit().await?;
    get_entry(pool, entry_id).await
}

pub async fn deactivate_entry(
    pool: &PgPool,
    entry_id: Uuid,
    timetable_version_id: Uuid,
    row_version: i64,
    actor_user_id: Uuid,
) -> Result<TimetableEntry, AppError> {
    deactivate_entry_impl(
        pool,
        entry_id,
        timetable_version_id,
        row_version,
        actor_user_id,
    )
    .await
    .map_err(map_timetable_write_error)
}

async fn deactivate_entry_impl(
    pool: &PgPool,
    entry_id: Uuid,
    timetable_version_id: Uuid,
    row_version: i64,
    actor_user_id: Uuid,
) -> Result<TimetableEntry, AppError> {
    let mut transaction = pool.begin().await?;
    let existing = lock_entry(&mut transaction, entry_id).await?;
    if existing.row_version != row_version {
        return Err(stale_entry());
    }
    if existing.timetable_version_id != timetable_version_id {
        return Err(AppError::ValidationError(
            "รายการตารางสอนไม่อยู่ในรุ่นตารางที่ระบุ".to_string(),
        ));
    }
    require_timetable_version_in_tx(
        &mut transaction,
        timetable_version_id,
        Some(existing.academic_term_id),
        true,
    )
    .await?;
    let instructors = entry_instructor_roles_in_tx(&mut transaction, entry_id).await?;
    let updated = sqlx::query(
        r#"UPDATE academic_timetable_entries
           SET is_active = false, updated_by = $2,
               row_version = row_version + 1, updated_at = now()
           WHERE id = $1 AND row_version = $3"#,
    )
    .bind(entry_id)
    .bind(actor_user_id)
    .bind(row_version)
    .execute(&mut *transaction)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(stale_entry());
    }
    append_timetable_entry_audit_in_tx(
        &mut transaction,
        "academic_timetable_entry.deactivated",
        entry_id,
        timetable_version_id,
        existing.academic_term_id,
        existing.academic_year_id,
        existing.learning_offering_id,
        existing.learning_group_id,
        actor_user_id,
        entry_audit_snapshot(
            &existing.day_of_week,
            existing.bell_schedule_period_id,
            existing.room_id,
            true,
            existing.row_version,
            &instructors,
        ),
        entry_audit_snapshot(
            &existing.day_of_week,
            existing.bell_schedule_period_id,
            existing.room_id,
            false,
            existing.row_version + 1,
            &instructors,
        ),
    )
    .await?;
    transaction.commit().await?;
    get_entry(pool, entry_id).await
}

pub async fn deactivate_batch(
    pool: &PgPool,
    batch_id: Uuid,
    timetable_version_id: Uuid,
    actor_user_id: Uuid,
) -> Result<Vec<TimetableEntry>, AppError> {
    deactivate_batch_impl(pool, batch_id, timetable_version_id, actor_user_id)
        .await
        .map_err(map_timetable_write_error)
}

async fn deactivate_batch_impl(
    pool: &PgPool,
    batch_id: Uuid,
    timetable_version_id: Uuid,
    actor_user_id: Uuid,
) -> Result<Vec<TimetableEntry>, AppError> {
    let mut transaction = pool.begin().await?;
    let entries: Vec<EntryLockRow> = sqlx::query_as(
        r#"SELECT id, timetable_version_id, academic_term_id, academic_year_id, learning_group_id,
                  learning_offering_id, homeroom_id, room_id, day_of_week,
                  bell_schedule_period_id, row_version, is_active
           FROM academic_timetable_entries
           WHERE batch_id = $1 AND timetable_version_id = $2 AND is_active
           ORDER BY id
           FOR UPDATE"#,
    )
    .bind(batch_id)
    .bind(timetable_version_id)
    .fetch_all(&mut *transaction)
    .await?;
    if entries.is_empty() {
        return Err(AppError::NotFound("ไม่พบชุดตารางสอน".to_string()));
    }
    require_timetable_version_in_tx(
        &mut transaction,
        timetable_version_id,
        Some(entries[0].academic_term_id),
        true,
    )
    .await?;
    let entry_ids = entries.iter().map(|entry| entry.id).collect::<Vec<_>>();
    let instructors_by_entry =
        entry_instructor_roles_by_entry_in_tx(&mut transaction, &entry_ids).await?;
    sqlx::query(
        r#"UPDATE academic_timetable_entries
           SET is_active = false, updated_by = $2,
               row_version = row_version + 1, updated_at = now()
           WHERE batch_id = $1 AND timetable_version_id = $3 AND is_active"#,
    )
    .bind(batch_id)
    .bind(actor_user_id)
    .bind(timetable_version_id)
    .execute(&mut *transaction)
    .await?;
    for entry in &entries {
        let instructors = instructors_by_entry
            .get(&entry.id)
            .cloned()
            .unwrap_or_default();
        append_timetable_entry_audit_in_tx(
            &mut transaction,
            "academic_timetable_entry.deactivated",
            entry.id,
            timetable_version_id,
            entry.academic_term_id,
            entry.academic_year_id,
            entry.learning_offering_id,
            entry.learning_group_id,
            actor_user_id,
            entry_audit_snapshot(
                &entry.day_of_week,
                entry.bell_schedule_period_id,
                entry.room_id,
                true,
                entry.row_version,
                &instructors,
            ),
            entry_audit_snapshot(
                &entry.day_of_week,
                entry.bell_schedule_period_id,
                entry.room_id,
                false,
                entry.row_version + 1,
                &instructors,
            ),
        )
        .await?;
    }
    transaction.commit().await?;
    get_entries(pool, &entry_ids).await
}

pub async fn swap_entries(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: SwapTimetableEntriesRequest,
) -> Result<SwapTimetableEntriesResponse, AppError> {
    swap_entries_impl(pool, actor_user_id, request)
        .await
        .map_err(map_timetable_write_error)
}

async fn swap_entries_impl(
    pool: &PgPool,
    actor_user_id: Uuid,
    request: SwapTimetableEntriesRequest,
) -> Result<SwapTimetableEntriesResponse, AppError> {
    if request.entry_a_id == request.entry_b_id {
        return Err(AppError::ValidationError(
            "ต้องเลือกรายการตารางสอนคนละรายการ".to_string(),
        ));
    }
    let mut ids = vec![request.entry_a_id, request.entry_b_id];
    ids.sort_unstable();
    let mut transaction = pool.begin().await?;
    let locked: Vec<EntryLockRow> = sqlx::query_as(
        r#"SELECT id, timetable_version_id, academic_term_id, academic_year_id, learning_group_id,
                  learning_offering_id, homeroom_id, room_id, day_of_week,
                  bell_schedule_period_id, row_version, is_active
           FROM academic_timetable_entries
           WHERE id = ANY($1) AND timetable_version_id = $2 AND is_active
           ORDER BY id
           FOR UPDATE"#,
    )
    .bind(&ids)
    .bind(request.timetable_version_id)
    .fetch_all(&mut *transaction)
    .await?;
    if locked.len() != 2 {
        return Err(AppError::NotFound("ไม่พบรายการตารางสอนสำหรับสลับ".to_string()));
    }
    let entry_a = locked
        .iter()
        .find(|entry| entry.id == request.entry_a_id)
        .unwrap()
        .clone();
    let entry_b = locked
        .iter()
        .find(|entry| entry.id == request.entry_b_id)
        .unwrap()
        .clone();
    if entry_a.academic_term_id != entry_b.academic_term_id
        || entry_a.timetable_version_id != entry_b.timetable_version_id
    {
        return Err(AppError::ValidationError(
            "สลับรายการข้ามภาคเรียนหรือข้ามรุ่นตารางไม่ได้".to_string(),
        ));
    }
    if entry_a.row_version != request.entry_a_row_version
        || entry_b.row_version != request.entry_b_row_version
    {
        return Err(stale_entry());
    }
    require_timetable_version_in_tx(
        &mut transaction,
        request.timetable_version_id,
        Some(entry_a.academic_term_id),
        true,
    )
    .await?;
    let instructors_by_entry =
        entry_instructor_roles_by_entry_in_tx(&mut transaction, &ids).await?;
    lock_slots_stable(
        &mut transaction,
        request.timetable_version_id,
        [
            (entry_a.day_of_week.clone(), entry_a.bell_schedule_period_id),
            (entry_b.day_of_week.clone(), entry_b.bell_schedule_period_id),
        ],
    )
    .await?;
    let scope_a = entry_candidate_scope(&mut transaction, &entry_a).await?;
    let scope_b = entry_candidate_scope(&mut transaction, &entry_b).await?;
    let excluded = [entry_a.id, entry_b.id];
    ensure_no_conflicts(
        &mut transaction,
        request.timetable_version_id,
        &entry_b.day_of_week,
        entry_b.bell_schedule_period_id,
        &scope_a,
        &excluded,
    )
    .await?;
    ensure_no_conflicts(
        &mut transaction,
        request.timetable_version_id,
        &entry_a.day_of_week,
        entry_a.bell_schedule_period_id,
        &scope_b,
        &excluded,
    )
    .await?;

    sqlx::query("UPDATE academic_timetable_entries SET is_active = false WHERE id = ANY($1)")
        .bind(&ids)
        .execute(&mut *transaction)
        .await?;
    for (entry, target_day, target_period) in [
        (
            &entry_a,
            &entry_b.day_of_week,
            entry_b.bell_schedule_period_id,
        ),
        (
            &entry_b,
            &entry_a.day_of_week,
            entry_a.bell_schedule_period_id,
        ),
    ] {
        sqlx::query(
            r#"UPDATE academic_timetable_entries
               SET day_of_week = $2, bell_schedule_period_id = $3,
                   is_active = true, updated_by = $4,
                   row_version = row_version + 1, updated_at = now()
               WHERE id = $1"#,
        )
        .bind(entry.id)
        .bind(target_day)
        .bind(target_period)
        .bind(actor_user_id)
        .execute(&mut *transaction)
        .await?;
    }
    for (entry, target_day, target_period) in [
        (
            &entry_a,
            &entry_b.day_of_week,
            entry_b.bell_schedule_period_id,
        ),
        (
            &entry_b,
            &entry_a.day_of_week,
            entry_a.bell_schedule_period_id,
        ),
    ] {
        let instructors = instructors_by_entry
            .get(&entry.id)
            .cloned()
            .unwrap_or_default();
        append_timetable_entry_audit_in_tx(
            &mut transaction,
            "academic_timetable_entry.updated",
            entry.id,
            request.timetable_version_id,
            entry.academic_term_id,
            entry.academic_year_id,
            entry.learning_offering_id,
            entry.learning_group_id,
            actor_user_id,
            entry_audit_snapshot(
                &entry.day_of_week,
                entry.bell_schedule_period_id,
                entry.room_id,
                true,
                entry.row_version,
                &instructors,
            ),
            entry_audit_snapshot(
                target_day,
                target_period,
                entry.room_id,
                true,
                entry.row_version + 1,
                &instructors,
            ),
        )
        .await?;
    }
    transaction.commit().await?;
    Ok(SwapTimetableEntriesResponse {
        entry_a: get_entry(pool, request.entry_a_id).await?,
        entry_b: get_entry(pool, request.entry_b_id).await?,
    })
}

pub async fn occupancy(
    pool: &PgPool,
    timetable_version_id: Uuid,
    academic_term_id: Uuid,
) -> Result<Vec<TimetableOccupancyCell>, AppError> {
    require_timetable_version(pool, timetable_version_id, Some(academic_term_id), false).await?;
    let entries: Vec<EntryLockRow> = sqlx::query_as(
        r#"SELECT id, timetable_version_id, academic_term_id, academic_year_id, learning_group_id,
                  learning_offering_id, homeroom_id, room_id, day_of_week,
                  bell_schedule_period_id, row_version, is_active
           FROM academic_timetable_entries
           WHERE timetable_version_id = $1 AND is_active
           ORDER BY day_of_week, bell_schedule_period_id, id"#,
    )
    .bind(timetable_version_id)
    .fetch_all(pool)
    .await?;
    let owners: Vec<(Uuid, Option<Uuid>)> = entries
        .iter()
        .map(|entry| (entry.id, entry.learning_group_id))
        .collect();
    let relationships = load_relationship_indexes(pool, &owners).await?;
    let mut cells = Vec::with_capacity(entries.len());
    for entry in entries {
        cells.push(TimetableOccupancyCell {
            entry_id: entry.id,
            learning_group_id: entry.learning_group_id,
            homeroom_ids: relationships.homerooms(entry.learning_group_id, entry.homeroom_id),
            room_id: entry.room_id,
            instructor_ids: relationships.instructors(entry.id),
            day_of_week: entry.day_of_week,
            bell_schedule_period_id: entry.bell_schedule_period_id,
        });
    }
    Ok(cells)
}

pub async fn validate_moves(
    pool: &PgPool,
    timetable_version_id: Uuid,
    academic_term_id: Uuid,
    entry_id: Uuid,
) -> Result<Vec<MoveValidityCell>, AppError> {
    let version =
        require_timetable_version(pool, timetable_version_id, Some(academic_term_id), false)
            .await?;
    let entry: EntryLockRow = sqlx::query_as(
        r#"SELECT id, timetable_version_id, academic_term_id, academic_year_id, learning_group_id,
                  learning_offering_id, homeroom_id, room_id, day_of_week,
                  bell_schedule_period_id, row_version, is_active
           FROM academic_timetable_entries
           WHERE id = $1 AND timetable_version_id = $2 AND is_active"#,
    )
    .bind(entry_id)
    .bind(timetable_version_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรายการตารางสอน".to_string()))?;
    if entry.academic_term_id != academic_term_id {
        return Err(AppError::ValidationError(
            "รายการตารางสอนไม่อยู่ในภาคเรียนที่ระบุ".to_string(),
        ));
    }
    let periods: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM bell_schedule_periods WHERE bell_schedule_id = $1 AND is_active ORDER BY order_index, id",
    )
    .bind(version.bell_schedule_id)
    .fetch_all(pool)
    .await?;
    let candidate_entries: Vec<SlotEntryRow> = sqlx::query_as(
        r#"SELECT id, learning_group_id, homeroom_id, room_id,
                  day_of_week, bell_schedule_period_id
           FROM academic_timetable_entries
           WHERE timetable_version_id = $1 AND is_active AND id <> $2
           ORDER BY day_of_week, bell_schedule_period_id, id"#,
    )
    .bind(timetable_version_id)
    .bind(entry_id)
    .fetch_all(pool)
    .await?;
    let mut owners = Vec::with_capacity(candidate_entries.len() + 1);
    owners.push((entry.id, entry.learning_group_id));
    owners.extend(
        candidate_entries
            .iter()
            .map(|candidate| (candidate.id, candidate.learning_group_id)),
    );
    let relationships = load_relationship_indexes(pool, &owners).await?;
    let scope = CandidateScope {
        learning_group_id: entry.learning_group_id,
        homeroom_ids: relationships.homerooms(entry.learning_group_id, entry.homeroom_id),
        instructor_ids: relationships.instructors(entry.id),
        room_id: entry.room_id,
    };
    let mut entries_by_slot: HashMap<(String, Uuid), Vec<&SlotEntryRow>> = HashMap::new();
    for candidate in &candidate_entries {
        entries_by_slot
            .entry((
                candidate.day_of_week.clone(),
                candidate.bell_schedule_period_id,
            ))
            .or_default()
            .push(candidate);
    }
    let mut cells = Vec::new();
    for day in VALID_DAYS {
        for period_id in &periods {
            if *day == entry.day_of_week && *period_id == entry.bell_schedule_period_id {
                cells.push(MoveValidityCell {
                    day_of_week: (*day).to_string(),
                    bell_schedule_period_id: *period_id,
                    state: "source".to_string(),
                    target_entry_id: None,
                    valid: true,
                    reason: String::new(),
                });
                continue;
            }
            let mut conflicts = Vec::new();
            if let Some(slot_entries) = entries_by_slot.get(&((*day).to_string(), *period_id)) {
                for candidate in slot_entries {
                    let homerooms =
                        relationships.homerooms(candidate.learning_group_id, candidate.homeroom_id);
                    let instructors = relationships.instructors(candidate.id);
                    append_conflicts(&mut conflicts, candidate, &homerooms, &instructors, &scope);
                }
            }
            cells.push(MoveValidityCell {
                day_of_week: (*day).to_string(),
                bell_schedule_period_id: *period_id,
                state: if conflicts.is_empty() {
                    "empty"
                } else {
                    "occupied"
                }
                .to_string(),
                target_entry_id: conflicts.first().map(|conflict| conflict.existing_entry_id),
                valid: conflicts.is_empty(),
                reason: conflicts
                    .first()
                    .map(|conflict| conflict.message.clone())
                    .unwrap_or_default(),
            });
        }
    }
    Ok(cells)
}

pub async fn validate_candidate(
    pool: &PgPool,
    request: &CreateTimetableEntryRequest,
) -> Result<TimetableValidationResponse, AppError> {
    let mut transaction = pool.begin().await?;
    let (term, _, entry_type, scope) = resolve_create_scope(&mut transaction, request).await?;
    let _ = entry_type;
    require_period_in_tx(&mut transaction, term.id, request.bell_schedule_period_id).await?;
    let day = normalize_day(&request.day_of_week)?;
    let conflicts = find_conflicts_in_tx(
        &mut transaction,
        request.timetable_version_id,
        &day,
        request.bell_schedule_period_id,
        &scope,
        &[],
    )
    .await?;
    transaction.rollback().await?;
    Ok(TimetableValidationResponse {
        is_valid: conflicts.is_empty(),
        conflicts,
    })
}

pub(super) async fn create_entry_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    actor_user_id: Uuid,
    batch_id: Option<Uuid>,
    request: &CreateTimetableEntryRequest,
) -> Result<Uuid, AppError> {
    let (term, group, entry_type, scope) = resolve_create_scope(transaction, request).await?;
    require_period_in_tx(transaction, term.id, request.bell_schedule_period_id).await?;
    let day = normalize_day(&request.day_of_week)?;
    lock_slot(
        transaction,
        request.timetable_version_id,
        &day,
        request.bell_schedule_period_id,
    )
    .await?;
    ensure_no_conflicts(
        transaction,
        request.timetable_version_id,
        &day,
        request.bell_schedule_period_id,
        &scope,
        &[],
    )
    .await?;
    let entry_id = Uuid::new_v4();
    let (learning_group_id, offering_id, title) = match group {
        Some(group) => (
            Some(group.id),
            Some(group.learning_offering_id),
            request.title.clone().or(Some(group.offering_name)),
        ),
        None => (None, None, request.title.clone()),
    };
    sqlx::query(
        r#"INSERT INTO academic_timetable_entries (
               id, day_of_week, bell_schedule_period_id,
               room_id, note, is_active, created_by, updated_by, entry_type, title,
               homeroom_id, academic_term_id, batch_id,
               academic_year_id, learning_offering_id, learning_group_id,
               bell_schedule_id, migration_provenance, row_version,
               timetable_version_id
           ) VALUES (
               $1, $2, $3, $4, $5, true, $6, $6, $7, $8,
               $9, $10, $11, $12, $13, $14, $15, '{}'::jsonb, 1, $16
           )"#,
    )
    .bind(entry_id)
    .bind(&day)
    .bind(request.bell_schedule_period_id)
    .bind(request.room_id)
    .bind(request.note.as_deref())
    .bind(actor_user_id)
    .bind(&entry_type)
    .bind(title.as_deref())
    .bind(request.homeroom_id)
    .bind(term.id)
    .bind(batch_id)
    .bind(term.academic_year_id)
    .bind(offering_id)
    .bind(learning_group_id)
    .bind(term.bell_schedule_id)
    .bind(request.timetable_version_id)
    .execute(&mut **transaction)
    .await?;
    for (index, instructor_id) in scope.instructor_ids.iter().enumerate() {
        sqlx::query(
            r#"INSERT INTO timetable_entry_instructors (id, entry_id, instructor_id, role)
               VALUES ($1, $2, $3, $4)"#,
        )
        .bind(Uuid::new_v4())
        .bind(entry_id)
        .bind(instructor_id)
        .bind(if index == 0 { "primary" } else { "secondary" })
        .execute(&mut **transaction)
        .await?;
    }
    let after_instructors = assigned_instructor_roles(&scope.instructor_ids);
    append_timetable_entry_audit_in_tx(
        transaction,
        "academic_timetable_entry.created",
        entry_id,
        request.timetable_version_id,
        term.id,
        term.academic_year_id,
        offering_id,
        learning_group_id,
        actor_user_id,
        entry_audit_snapshot(
            &day,
            request.bell_schedule_period_id,
            request.room_id,
            false,
            0,
            &[],
        ),
        entry_audit_snapshot(
            &day,
            request.bell_schedule_period_id,
            request.room_id,
            true,
            1,
            &after_instructors,
        ),
    )
    .await?;
    Ok(entry_id)
}

async fn resolve_create_scope(
    transaction: &mut Transaction<'_, Postgres>,
    request: &CreateTimetableEntryRequest,
) -> Result<(TermContext, Option<GroupContext>, String, CandidateScope), AppError> {
    let version = require_timetable_version_in_tx(
        transaction,
        request.timetable_version_id,
        Some(request.academic_term_id),
        true,
    )
    .await?;
    let term = TermContext {
        id: version.academic_term_id,
        academic_year_id: version.academic_year_id,
        bell_schedule_id: version.bell_schedule_id,
    };
    let entry_type = normalize_entry_type(&request.entry_type)?;
    let room_id = request.room_id;
    if let Some(room_id) = room_id {
        let exists: bool = sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM rooms WHERE id = $1 AND status = 'ACTIVE')",
        )
        .bind(room_id)
        .fetch_one(&mut **transaction)
        .await?;
        if !exists {
            return Err(AppError::ValidationError("ไม่พบห้องที่เปิดใช้งาน".to_string()));
        }
    }

    if let Some(group_id) = request.learning_group_id {
        let group: GroupContext = sqlx::query_as(
            r#"SELECT learning_group.id,
                      learning_group.learning_offering_id,
                      learning_group.academic_term_id,
                      learning_group.academic_year_id,
                      offering.kind AS offering_kind,
                      offering.name_snapshot AS offering_name
               FROM learning_groups learning_group
               JOIN learning_offerings offering ON offering.id = learning_group.learning_offering_id
               WHERE learning_group.id = $1"#,
        )
        .bind(group_id)
        .fetch_optional(&mut **transaction)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบกลุ่มเรียน".to_string()))?;
        if group.academic_term_id != term.id || group.academic_year_id != term.academic_year_id {
            return Err(AppError::ValidationError(
                "กลุ่มเรียนไม่ได้อยู่ในภาคเรียนที่ระบุ".to_string(),
            ));
        }
        let expected_entry_type = if group.offering_kind == "course" {
            "COURSE"
        } else {
            "ACTIVITY"
        };
        if entry_type != expected_entry_type {
            return Err(AppError::ValidationError(
                "ชนิดรายการไม่ตรงกับชนิดการเปิดสอนของกลุ่มเรียน".to_string(),
            ));
        }
        let homeroom_ids = effective_homerooms_in_tx(transaction, Some(group.id), None).await?;
        if request
            .homeroom_id
            .is_some_and(|id| !homeroom_ids.contains(&id))
        {
            return Err(AppError::ValidationError(
                "ห้องประจำชั้นไม่อยู่ในขอบเขตของกลุ่มเรียน".to_string(),
            ));
        }
        let instructor_ids = eligible_group_instructors_in_tx(
            transaction,
            group.id,
            version.effective_from,
            &request.instructor_ids,
        )
        .await?;
        let scope = CandidateScope {
            learning_group_id: Some(group.id),
            homeroom_ids,
            instructor_ids,
            room_id,
        };
        Ok((term, Some(group), entry_type, scope))
    } else {
        if matches!(entry_type.as_str(), "COURSE" | "ACTIVITY") {
            return Err(AppError::ValidationError(
                "รายการรายวิชาหรือกิจกรรมต้องระบุ learningGroupId".to_string(),
            ));
        }
        if request.homeroom_id.is_none() && request.instructor_ids.is_empty() {
            return Err(AppError::ValidationError(
                "รายการโครงสร้างต้องระบุ homeroomId หรือ instructorIds".to_string(),
            ));
        }
        if let Some(homeroom_id) = request.homeroom_id {
            let valid: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM homerooms WHERE id = $1 AND academic_year_id = $2)",
            )
            .bind(homeroom_id)
            .bind(term.academic_year_id)
            .fetch_one(&mut **transaction)
            .await?;
            if !valid {
                return Err(AppError::ValidationError(
                    "ห้องประจำชั้นไม่ได้อยู่ในปีการศึกษาของภาคเรียน".to_string(),
                ));
            }
        }
        let instructor_ids = canonical_ids(&request.instructor_ids);
        if !instructor_ids.is_empty() {
            let active_count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM users WHERE id = ANY($1) AND user_type = 'staff' AND status = 'active'",
            )
            .bind(&instructor_ids)
            .fetch_one(&mut **transaction)
            .await?;
            if active_count != instructor_ids.len() as i64 {
                return Err(AppError::ValidationError(
                    "มีครูที่ไม่พบหรือไม่ได้เปิดใช้งาน".to_string(),
                ));
            }
        }
        let scope = CandidateScope {
            learning_group_id: None,
            homeroom_ids: request.homeroom_id.into_iter().collect(),
            instructor_ids,
            room_id,
        };
        Ok((term, None, entry_type, scope))
    }
}

fn validate_batch_shape(request: &CreateBatchTimetableEntriesRequest) -> Result<(), AppError> {
    if !request.learning_group_ids.is_empty() && !request.homeroom_ids.is_empty() {
        return Err(AppError::ValidationError(
            "ชุดตารางสอนต้องเลือกกลุ่มเรียนหรือห้องประจำชั้นอย่างใดอย่างหนึ่ง".to_string(),
        ));
    }
    if request.days_of_week.is_empty() || request.bell_schedule_period_ids.is_empty() {
        return Err(AppError::ValidationError(
            "ต้องระบุวันและคาบอย่างน้อยหนึ่งรายการ".to_string(),
        ));
    }
    let target_count = request
        .learning_group_ids
        .len()
        .max(request.homeroom_ids.len())
        .max(1);
    let total = target_count
        .saturating_mul(request.days_of_week.len())
        .saturating_mul(request.bell_schedule_period_ids.len());
    if total > 500 {
        return Err(AppError::ValidationError(
            "สร้างตารางสอนได้ไม่เกิน 500 รายการต่อคำขอ".to_string(),
        ));
    }
    Ok(())
}

async fn require_timetable_version(
    pool: &PgPool,
    timetable_version_id: Uuid,
    expected_term_id: Option<Uuid>,
    writable: bool,
) -> Result<TimetableVersionContext, AppError> {
    let version: TimetableVersionContext = sqlx::query_as(
        r#"SELECT version.academic_term_id, version.academic_year_id,
                  version.bell_schedule_id, version.effective_from, version.status,
                  term.status AS term_status
           FROM academic_timetable_versions version
           JOIN academic_terms term ON term.id = version.academic_term_id
           WHERE version.id = $1"#,
    )
    .bind(timetable_version_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นตารางสอน".to_string()))?;
    ensure_timetable_version_usable(&version, expected_term_id, writable)?;
    Ok(version)
}

async fn require_timetable_version_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    timetable_version_id: Uuid,
    expected_term_id: Option<Uuid>,
    writable: bool,
) -> Result<TimetableVersionContext, AppError> {
    let version: TimetableVersionContext = sqlx::query_as(
        r#"SELECT version.academic_term_id, version.academic_year_id,
                  version.bell_schedule_id, version.effective_from, version.status,
                  term.status AS term_status
           FROM academic_timetable_versions version
           JOIN academic_terms term ON term.id = version.academic_term_id
           WHERE version.id = $1
           FOR SHARE OF version, term"#,
    )
    .bind(timetable_version_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรุ่นตารางสอน".to_string()))?;
    ensure_timetable_version_usable(&version, expected_term_id, writable)?;
    Ok(version)
}

pub(super) async fn require_draft_version_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    timetable_version_id: Uuid,
    academic_term_id: Uuid,
) -> Result<(), AppError> {
    require_timetable_version_in_tx(
        transaction,
        timetable_version_id,
        Some(academic_term_id),
        true,
    )
    .await?;
    Ok(())
}

fn ensure_timetable_version_usable(
    version: &TimetableVersionContext,
    expected_term_id: Option<Uuid>,
    writable: bool,
) -> Result<(), AppError> {
    if expected_term_id.is_some_and(|term_id| term_id != version.academic_term_id) {
        return Err(AppError::ValidationError(
            "รุ่นตารางสอนไม่อยู่ในภาคเรียนที่ระบุ".to_string(),
        ));
    }
    if version.status == "cancelled" {
        return Err(AppError::Conflict("รุ่นตารางสอนนี้ถูกยกเลิกแล้ว".to_string()));
    }
    if writable && version.status != "draft" {
        return Err(AppError::Conflict(
            "แก้ไขได้เฉพาะรุ่นตารางสอนแบบร่าง".to_string(),
        ));
    }
    if writable
        && matches!(
            version.term_status.as_str(),
            "closing" | "closed" | "archived" | "cancelled"
        )
    {
        return Err(AppError::Conflict(
            "ภาคเรียนนี้ปิดรับการแก้ไขตารางสอนแล้ว".to_string(),
        ));
    }
    Ok(())
}

async fn require_period_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    academic_term_id: Uuid,
    period_id: Uuid,
) -> Result<(), AppError> {
    let valid: bool = sqlx::query_scalar(
        r#"SELECT EXISTS(
               SELECT 1
               FROM academic_terms term
               JOIN bell_schedule_periods period
                 ON period.bell_schedule_id = term.bell_schedule_id
               WHERE term.id = $1 AND period.id = $2 AND period.is_active
           )"#,
    )
    .bind(academic_term_id)
    .bind(period_id)
    .fetch_one(&mut **transaction)
    .await?;
    if valid {
        Ok(())
    } else {
        Err(AppError::ValidationError(
            "คาบเวลาไม่ได้อยู่ใน bell schedule ของภาคเรียน".to_string(),
        ))
    }
}

async fn lock_entry(
    transaction: &mut Transaction<'_, Postgres>,
    entry_id: Uuid,
) -> Result<EntryLockRow, AppError> {
    sqlx::query_as(
        r#"SELECT id, timetable_version_id, academic_term_id, academic_year_id, learning_group_id,
                  learning_offering_id, homeroom_id, room_id, day_of_week,
                  bell_schedule_period_id, row_version, is_active
           FROM academic_timetable_entries
           WHERE id = $1 AND is_active
           FOR UPDATE"#,
    )
    .bind(entry_id)
    .fetch_optional(&mut **transaction)
    .await?
    .ok_or_else(|| AppError::NotFound("ไม่พบรายการตารางสอน".to_string()))
}

async fn lock_slot(
    transaction: &mut Transaction<'_, Postgres>,
    timetable_version_id: Uuid,
    day: &str,
    period_id: Uuid,
) -> Result<(), AppError> {
    let key = format!("timetable:{timetable_version_id}:{day}:{period_id}");
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(key)
        .execute(&mut **transaction)
        .await?;
    Ok(())
}

async fn lock_slots_stable<const N: usize>(
    transaction: &mut Transaction<'_, Postgres>,
    timetable_version_id: Uuid,
    slots: [(String, Uuid); N],
) -> Result<(), AppError> {
    let mut slots = slots.to_vec();
    slots.sort_unstable();
    slots.dedup();
    for (day, period_id) in slots {
        lock_slot(transaction, timetable_version_id, &day, period_id).await?;
    }
    Ok(())
}

async fn entry_candidate_scope(
    transaction: &mut Transaction<'_, Postgres>,
    entry: &EntryLockRow,
) -> Result<CandidateScope, AppError> {
    Ok(CandidateScope {
        learning_group_id: entry.learning_group_id,
        homeroom_ids: effective_homerooms_in_tx(
            transaction,
            entry.learning_group_id,
            entry.homeroom_id,
        )
        .await?,
        instructor_ids: exact_entry_instructors_in_tx(transaction, entry.id).await?,
        room_id: entry.room_id,
    })
}

async fn ensure_no_conflicts(
    transaction: &mut Transaction<'_, Postgres>,
    timetable_version_id: Uuid,
    day: &str,
    period_id: Uuid,
    scope: &CandidateScope,
    excluded_entry_ids: &[Uuid],
) -> Result<(), AppError> {
    let conflicts = find_conflicts_in_tx(
        transaction,
        timetable_version_id,
        day,
        period_id,
        scope,
        excluded_entry_ids,
    )
    .await?;
    if let Some(conflict) = conflicts.first() {
        Err(AppError::Conflict(conflict.message.clone()))
    } else {
        Ok(())
    }
}

async fn find_conflicts_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    timetable_version_id: Uuid,
    day: &str,
    period_id: Uuid,
    scope: &CandidateScope,
    excluded_entry_ids: &[Uuid],
) -> Result<Vec<ConflictInfo>, AppError> {
    let entries: Vec<SlotEntryRow> = sqlx::query_as(
        r#"SELECT id, learning_group_id, homeroom_id, room_id,
                  day_of_week, bell_schedule_period_id
           FROM academic_timetable_entries
           WHERE timetable_version_id = $1
             AND day_of_week = $2
             AND bell_schedule_period_id = $3
             AND is_active
             AND NOT (id = ANY($4))
           ORDER BY id
           FOR UPDATE"#,
    )
    .bind(timetable_version_id)
    .bind(day)
    .bind(period_id)
    .bind(excluded_entry_ids)
    .fetch_all(&mut **transaction)
    .await?;
    compare_conflicts_in_tx(transaction, entries, scope).await
}

async fn compare_conflicts_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    entries: Vec<SlotEntryRow>,
    scope: &CandidateScope,
) -> Result<Vec<ConflictInfo>, AppError> {
    let owners: Vec<(Uuid, Option<Uuid>)> = entries
        .iter()
        .map(|entry| (entry.id, entry.learning_group_id))
        .collect();
    let relationships = load_relationship_indexes_in_tx(transaction, &owners).await?;
    let mut conflicts = Vec::new();
    for entry in entries {
        let homerooms = relationships.homerooms(entry.learning_group_id, entry.homeroom_id);
        let instructors = relationships.instructors(entry.id);
        append_conflicts(&mut conflicts, &entry, &homerooms, &instructors, scope);
    }
    Ok(conflicts)
}

fn append_conflicts(
    conflicts: &mut Vec<ConflictInfo>,
    entry: &SlotEntryRow,
    homerooms: &[Uuid],
    instructors: &[Uuid],
    candidate: &CandidateScope,
) {
    if candidate.learning_group_id.is_some()
        && candidate.learning_group_id == entry.learning_group_id
    {
        conflicts.push(conflict(
            "GROUP_CONFLICT",
            "กลุ่มเรียนมีตารางในคาบนี้แล้ว",
            entry.id,
        ));
    }
    if !candidate.homeroom_ids.is_empty()
        && homerooms
            .iter()
            .any(|homeroom| candidate.homeroom_ids.contains(homeroom))
    {
        conflicts.push(conflict(
            "HOMEROOM_CONFLICT",
            "ขอบเขตห้องประจำชั้นมีตารางในคาบนี้แล้ว",
            entry.id,
        ));
    }
    if candidate.room_id.is_some() && candidate.room_id == entry.room_id {
        conflicts.push(conflict("ROOM_CONFLICT", "ห้องเรียนถูกใช้ในคาบนี้แล้ว", entry.id));
    }
    if instructors
        .iter()
        .any(|instructor| candidate.instructor_ids.contains(instructor))
    {
        conflicts.push(conflict(
            "INSTRUCTOR_CONFLICT",
            "ครูมีตารางในคาบนี้แล้ว",
            entry.id,
        ));
    }
}

fn conflict(kind: &str, message: &str, entry_id: Uuid) -> ConflictInfo {
    ConflictInfo {
        conflict_type: kind.to_string(),
        message: message.to_string(),
        existing_entry_id: entry_id,
    }
}

async fn load_relationship_indexes(
    pool: &PgPool,
    owners: &[(Uuid, Option<Uuid>)],
) -> Result<RelationshipIndexes, AppError> {
    let (group_ids, entry_ids) = relationship_owner_ids(owners);
    let homeroom_rows: Vec<(Uuid, Uuid)> = if group_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
            r#"SELECT learning_group_id, homeroom_id
               FROM learning_group_homerooms
               WHERE learning_group_id = ANY($1)
               ORDER BY learning_group_id, homeroom_id"#,
        )
        .bind(&group_ids)
        .fetch_all(pool)
        .await?
    };
    let entry_instructor_rows: Vec<(Uuid, Uuid)> = if entry_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
            r#"SELECT entry_id, instructor_id
               FROM timetable_entry_instructors
               WHERE entry_id = ANY($1)
               ORDER BY entry_id, instructor_id"#,
        )
        .bind(&entry_ids)
        .fetch_all(pool)
        .await?
    };
    Ok(RelationshipIndexes {
        homerooms_by_group: group_uuid_rows(homeroom_rows),
        instructors_by_entry: group_uuid_rows(entry_instructor_rows),
    })
}

async fn load_relationship_indexes_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    owners: &[(Uuid, Option<Uuid>)],
) -> Result<RelationshipIndexes, AppError> {
    let (group_ids, entry_ids) = relationship_owner_ids(owners);
    let homeroom_rows: Vec<(Uuid, Uuid)> = if group_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
            r#"SELECT learning_group_id, homeroom_id
               FROM learning_group_homerooms
               WHERE learning_group_id = ANY($1)
               ORDER BY learning_group_id, homeroom_id"#,
        )
        .bind(&group_ids)
        .fetch_all(&mut **transaction)
        .await?
    };
    let entry_instructor_rows: Vec<(Uuid, Uuid)> = if entry_ids.is_empty() {
        Vec::new()
    } else {
        sqlx::query_as(
            r#"SELECT entry_id, instructor_id
               FROM timetable_entry_instructors
               WHERE entry_id = ANY($1)
               ORDER BY entry_id, instructor_id"#,
        )
        .bind(&entry_ids)
        .fetch_all(&mut **transaction)
        .await?
    };
    Ok(RelationshipIndexes {
        homerooms_by_group: group_uuid_rows(homeroom_rows),
        instructors_by_entry: group_uuid_rows(entry_instructor_rows),
    })
}

fn relationship_owner_ids(owners: &[(Uuid, Option<Uuid>)]) -> (Vec<Uuid>, Vec<Uuid>) {
    let group_ids = owners
        .iter()
        .filter_map(|(_, group_id)| *group_id)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let entry_ids = owners.iter().map(|(entry_id, _)| *entry_id).collect();
    (group_ids, entry_ids)
}

fn group_uuid_rows(rows: Vec<(Uuid, Uuid)>) -> HashMap<Uuid, Vec<Uuid>> {
    let mut grouped: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for (owner_id, related_id) in rows {
        grouped.entry(owner_id).or_default().push(related_id);
    }
    grouped
}

async fn effective_homerooms_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    learning_group_id: Option<Uuid>,
    homeroom_id: Option<Uuid>,
) -> Result<Vec<Uuid>, AppError> {
    if let Some(group_id) = learning_group_id {
        Ok(sqlx::query_scalar(
            "SELECT homeroom_id FROM learning_group_homerooms WHERE learning_group_id = $1 ORDER BY homeroom_id",
        )
        .bind(group_id)
        .fetch_all(&mut **transaction)
        .await?)
    } else {
        Ok(homeroom_id.into_iter().collect())
    }
}

async fn exact_entry_instructors_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    entry_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    if entry_id.is_nil() {
        Ok(Vec::new())
    } else {
        Ok(sqlx::query_scalar(
            "SELECT instructor_id FROM timetable_entry_instructors WHERE entry_id = $1 ORDER BY instructor_id",
        )
        .bind(entry_id)
        .fetch_all(&mut **transaction)
        .await?)
    }
}

async fn eligible_group_instructors_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    learning_group_id: Uuid,
    effective_from: NaiveDate,
    requested_ids: &[Uuid],
) -> Result<Vec<Uuid>, AppError> {
    let requested_ids = canonical_ids(requested_ids);
    if requested_ids.is_empty() {
        return Ok(Vec::new());
    }
    let eligible_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT teacher.teacher_id
           FROM learning_group_teachers teacher
           WHERE teacher.learning_group_id = $1
             AND teacher.starts_on <= $2
             AND (teacher.ends_on IS NULL OR teacher.ends_on >= $2)
             AND teacher.teacher_id = ANY($3)
           ORDER BY CASE teacher.role
                        WHEN 'primary' THEN 1
                        WHEN 'secondary' THEN 2
                        ELSE 3
                    END,
                    teacher.starts_on, teacher.id"#,
    )
    .bind(learning_group_id)
    .bind(effective_from)
    .bind(&requested_ids)
    .fetch_all(&mut **transaction)
    .await?;
    if eligible_ids.len() != requested_ids.len() {
        return Err(AppError::ValidationError(
            "ครูที่เลือกไม่ได้รับมอบหมายให้กลุ่มเรียนในวันที่รุ่นตารางเริ่มใช้".to_string(),
        ));
    }
    Ok(eligible_ids)
}

async fn eligible_structural_instructors_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    requested_ids: &[Uuid],
) -> Result<Vec<Uuid>, AppError> {
    let requested_ids = canonical_ids(requested_ids);
    if requested_ids.is_empty() {
        return Ok(Vec::new());
    }
    let eligible_count: i64 = sqlx::query_scalar(
        r#"SELECT count(*)
           FROM users
           WHERE id = ANY($1) AND user_type = 'staff' AND status = 'active'"#,
    )
    .bind(&requested_ids)
    .fetch_one(&mut **transaction)
    .await?;
    if eligible_count != requested_ids.len() as i64 {
        return Err(AppError::ValidationError(
            "มีครูที่ไม่พบหรือไม่ได้เปิดใช้งาน".to_string(),
        ));
    }
    Ok(requested_ids)
}

async fn replace_entry_instructors_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    entry_id: Uuid,
    instructor_ids: &[Uuid],
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM timetable_entry_instructors WHERE entry_id = $1")
        .bind(entry_id)
        .execute(&mut **transaction)
        .await?;
    for (index, instructor_id) in instructor_ids.iter().enumerate() {
        sqlx::query(
            r#"INSERT INTO timetable_entry_instructors (id, entry_id, instructor_id, role)
               VALUES ($1, $2, $3, $4)"#,
        )
        .bind(Uuid::new_v4())
        .bind(entry_id)
        .bind(instructor_id)
        .bind(if index == 0 { "primary" } else { "secondary" })
        .execute(&mut **transaction)
        .await?;
    }
    Ok(())
}

async fn entry_instructor_roles_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    entry_id: Uuid,
) -> Result<Vec<(Uuid, String)>, AppError> {
    Ok(sqlx::query_as(
        r#"SELECT instructor_id, role::text
           FROM timetable_entry_instructors
           WHERE entry_id = $1
           ORDER BY instructor_id"#,
    )
    .bind(entry_id)
    .fetch_all(&mut **transaction)
    .await?)
}

async fn entry_instructor_roles_by_entry_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    entry_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<(Uuid, String)>>, AppError> {
    let rows: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
        r#"SELECT entry_id, instructor_id, role::text
           FROM timetable_entry_instructors
           WHERE entry_id = ANY($1)
           ORDER BY entry_id, instructor_id"#,
    )
    .bind(entry_ids)
    .fetch_all(&mut **transaction)
    .await?;
    let mut grouped = HashMap::new();
    for (entry_id, instructor_id, role) in rows {
        grouped
            .entry(entry_id)
            .or_insert_with(Vec::new)
            .push((instructor_id, role));
    }
    Ok(grouped)
}

fn assigned_instructor_roles(instructor_ids: &[Uuid]) -> Vec<(Uuid, String)> {
    instructor_ids
        .iter()
        .enumerate()
        .map(|(index, instructor_id)| {
            (
                *instructor_id,
                if index == 0 { "primary" } else { "secondary" }.to_string(),
            )
        })
        .collect()
}

fn entry_audit_snapshot(
    day_of_week: &str,
    bell_schedule_period_id: Uuid,
    room_id: Option<Uuid>,
    is_active: bool,
    row_version: i64,
    instructors: &[(Uuid, String)],
) -> TimetableEntryAuditSnapshot {
    let mut instructors = instructors.to_vec();
    instructors.sort_by_key(|(instructor_id, _)| *instructor_id);
    let instructor_ids = instructors
        .iter()
        .map(|(instructor_id, _)| *instructor_id)
        .collect::<Vec<_>>();
    let instructor_roles = instructors
        .into_iter()
        .map(|(instructor_id, role)| TimetableInstructorAudit {
            instructor_id,
            role,
        })
        .collect::<Vec<_>>();
    TimetableEntryAuditSnapshot {
        day_of_week: day_of_week.to_string(),
        bell_schedule_period_id,
        room_id,
        is_active,
        row_version,
        instructor_ids,
        instructors: instructor_roles,
    }
}

#[allow(clippy::too_many_arguments)]
async fn append_timetable_entry_audit_in_tx(
    transaction: &mut Transaction<'_, Postgres>,
    event_code: &str,
    entry_id: Uuid,
    timetable_version_id: Uuid,
    academic_term_id: Uuid,
    academic_year_id: Uuid,
    learning_offering_id: Option<Uuid>,
    learning_group_id: Option<Uuid>,
    actor_user_id: Uuid,
    before: TimetableEntryAuditSnapshot,
    after: TimetableEntryAuditSnapshot,
) -> Result<(), AppError> {
    let payload = TimetableEntryAuditPayload {
        entry_id,
        timetable_version_id,
        academic_term_id,
        academic_year_id,
        learning_offering_id,
        learning_group_id,
        actor_user_id,
        old_row_version: before.row_version,
        new_row_version: after.row_version,
        before,
        after,
    };
    sqlx::query(
        r#"INSERT INTO academic_audit_events (
               event_code, entity_type, entity_id, academic_year_id,
               academic_term_id, actor_user_id, payload
           ) VALUES ($1, 'academic_timetable_entry', $2, $3, $4, $5, $6)"#,
    )
    .bind(event_code)
    .bind(entry_id)
    .bind(academic_year_id)
    .bind(academic_term_id)
    .bind(actor_user_id)
    .bind(sqlx::types::Json(payload))
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

fn entry_select() -> &'static str {
    r#"SELECT entry.id,
              entry.timetable_version_id,
              entry.academic_term_id,
              entry.academic_year_id,
              entry.bell_schedule_id,
              entry.bell_schedule_period_id,
              entry.day_of_week,
              entry.entry_type,
              entry.learning_group_id,
              entry.learning_offering_id,
              entry.homeroom_id,
              entry.room_id,
              entry.note,
              entry.title,
              entry.batch_id,
              entry.row_version,
              entry.is_active,
              offering.code_snapshot AS offering_code,
              offering.name_snapshot AS offering_name,
              learning_group.code AS learning_group_code,
              learning_group.name AS learning_group_name,
              course_detail.subject_id,
              subject_group.id AS subject_group_id,
              subject_group.name_th AS subject_group_name,
              subject_group.display_order AS subject_group_display_order,
              CASE WHEN course_version.id IS NULL THEN NULL ELSE concat(
                  coalesce(course_version.name_th, course_version.name_en, offering.name_snapshot),
                  ' · v', course_version.version_no
              ) END AS subject_version_display_label,
              activity_detail.activity_id,
              CASE WHEN activity_version.id IS NULL THEN NULL ELSE concat(
                  activity_version.name, ' · v', activity_version.version_no
              ) END AS activity_version_display_label,
              activity_detail.scheduling_mode AS activity_scheduling_mode,
              homeroom.name AS homeroom_name,
              room.code AS room_code,
              period.name AS period_name,
              period.start_time,
              period.end_time,
              entry.created_at,
              entry.updated_at
       FROM academic_timetable_entries entry
       JOIN bell_schedule_periods period ON period.id = entry.bell_schedule_period_id
       LEFT JOIN learning_groups learning_group ON learning_group.id = entry.learning_group_id
       LEFT JOIN learning_offerings offering ON offering.id = entry.learning_offering_id
       LEFT JOIN course_offering_details course_detail
         ON course_detail.learning_offering_id = offering.id
       LEFT JOIN subject_versions course_version
         ON course_version.id = course_detail.subject_version_id
       LEFT JOIN subject_groups subject_group
         ON subject_group.id = course_version.group_id
       LEFT JOIN activity_offering_details activity_detail
         ON activity_detail.learning_offering_id = offering.id
       LEFT JOIN activity_versions activity_version
         ON activity_version.id = activity_detail.activity_version_id
       LEFT JOIN homerooms homeroom ON homeroom.id = entry.homeroom_id
       LEFT JOIN rooms room ON room.id = entry.room_id"#
}

async fn hydrate_rows(pool: &PgPool, rows: Vec<EntryRow>) -> Result<Vec<TimetableEntry>, AppError> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }

    let entry_ids: Vec<Uuid> = rows.iter().map(|row| row.id).collect();
    let entry_instructor_rows: Vec<InstructorQueryRow> = sqlx::query_as(
        r#"SELECT instructor.entry_id,
                  instructor.instructor_id,
                  concat_ws(' ', nullif(concat(coalesce(user_account.title, ''), user_account.first_name), ''), nullif(user_account.last_name, '')) AS display_name,
                  instructor.role::text,
                  teacher_subject_group.id AS subject_group_id,
                  teacher_subject_group.name_th AS subject_group_name,
                  teacher_subject_group.display_order AS subject_group_display_order
           FROM timetable_entry_instructors instructor
           JOIN users user_account ON user_account.id = instructor.instructor_id
           LEFT JOIN LATERAL (
               SELECT subject_group.id, subject_group.name_th, subject_group.display_order
               FROM organization_members membership
               JOIN organization_units unit
                 ON unit.id = membership.organization_unit_id
               JOIN subject_groups subject_group
                 ON subject_group.id = unit.subject_group_id
               WHERE membership.user_id = instructor.instructor_id
                 AND membership.started_at <= CURRENT_DATE
                 AND (membership.ended_at IS NULL OR membership.ended_at >= CURRENT_DATE)
               ORDER BY membership.is_primary DESC,
                        subject_group.display_order,
                        membership.started_at,
                        membership.id
               LIMIT 1
           ) teacher_subject_group ON true
           WHERE instructor.entry_id = ANY($1)
           ORDER BY instructor.entry_id,
                    CASE instructor.role WHEN 'primary' THEN 1 ELSE 2 END,
                    instructor.created_at, instructor.instructor_id"#,
    )
    .bind(&entry_ids)
    .fetch_all(pool)
    .await?;

    let mut entry_instructors = group_instructors_by_owner(entry_instructor_rows);
    Ok(rows
        .into_iter()
        .map(|row| {
            let instructors = entry_instructors.remove(&row.id).unwrap_or_default();
            hydrate_entry(row, instructors)
        })
        .collect())
}

type InstructorQueryRow = (
    Uuid,
    Uuid,
    String,
    String,
    Option<Uuid>,
    Option<String>,
    Option<i32>,
);

fn group_instructors_by_owner(
    rows: Vec<InstructorQueryRow>,
) -> HashMap<Uuid, Vec<TimetableInstructor>> {
    let mut values: HashMap<Uuid, Vec<TimetableInstructor>> = HashMap::new();
    for (
        owner_id,
        user_id,
        display_name,
        role,
        subject_group_id,
        subject_group_name,
        subject_group_display_order,
    ) in rows
    {
        values
            .entry(owner_id)
            .or_default()
            .push(TimetableInstructor {
                user_id,
                display_name,
                role,
                subject_group_id,
                subject_group_name,
                subject_group_display_order,
            });
    }
    values
}

fn hydrate_entry(row: EntryRow, instructors: Vec<TimetableInstructor>) -> TimetableEntry {
    TimetableEntry {
        id: row.id,
        timetable_version_id: row.timetable_version_id,
        academic_term_id: row.academic_term_id,
        academic_year_id: row.academic_year_id,
        bell_schedule_id: row.bell_schedule_id,
        bell_schedule_period_id: row.bell_schedule_period_id,
        day_of_week: row.day_of_week,
        entry_type: wire_entry_type(&row.entry_type),
        learning_group_id: row.learning_group_id,
        offering_id: row.learning_offering_id,
        homeroom_id: row.homeroom_id,
        room_id: row.room_id,
        note: row.note,
        title: row.title,
        batch_id: row.batch_id,
        row_version: row.row_version,
        is_active: row.is_active,
        offering_code: row.offering_code,
        offering_name: row.offering_name,
        learning_group_code: row.learning_group_code,
        learning_group_name: row.learning_group_name,
        subject_id: row.subject_id,
        subject_group_id: row.subject_group_id,
        subject_group_name: row.subject_group_name,
        subject_group_display_order: row.subject_group_display_order,
        subject_version_display_label: row.subject_version_display_label,
        activity_id: row.activity_id,
        activity_version_display_label: row.activity_version_display_label,
        activity_scheduling_mode: row.activity_scheduling_mode,
        homeroom_name: row.homeroom_name,
        room_code: row.room_code,
        period_name: row.period_name,
        start_time: row.start_time,
        end_time: row.end_time,
        instructors,
        created_at: row.created_at,
        updated_at: row.updated_at,
    }
}

fn normalize_day(day: &str) -> Result<String, AppError> {
    let day = day.trim().to_ascii_uppercase();
    if VALID_DAYS.contains(&day.as_str()) {
        Ok(day)
    } else {
        Err(AppError::ValidationError("วันในสัปดาห์ไม่ถูกต้อง".to_string()))
    }
}

fn normalize_entry_type(entry_type: &str) -> Result<String, AppError> {
    let entry_type = entry_type.trim().to_ascii_uppercase();
    if VALID_ENTRY_TYPES.contains(&entry_type.as_str()) {
        Ok(entry_type)
    } else {
        Err(AppError::ValidationError(
            "ชนิดรายการตารางสอนไม่ถูกต้อง".to_string(),
        ))
    }
}

fn wire_entry_type(entry_type: &str) -> String {
    entry_type.trim().to_ascii_uppercase()
}

fn canonical_ids(ids: &[Uuid]) -> Vec<Uuid> {
    let mut ids = ids.to_vec();
    ids.sort_unstable();
    ids.dedup();
    ids
}

fn stale_entry() -> AppError {
    AppError::Conflict("ตารางสอนมีการแก้ไขจากผู้ใช้อื่น กรุณาโหลดใหม่".to_string())
}

pub(super) fn map_timetable_write_error(error: AppError) -> AppError {
    let guard_message = match &error {
        AppError::DbError(sqlx::Error::Database(database)) => {
            timetable_guard_conflict_message(database.code().as_deref(), database.message())
        }
        _ => None,
    };
    guard_message
        .map(|message| AppError::Conflict(message.to_string()))
        .unwrap_or(error)
}

fn timetable_guard_conflict_message<'a>(code: Option<&str>, message: &str) -> Option<&'a str> {
    if code != Some("23514") {
        return None;
    }
    match message {
        "ACADEMIC_TIMETABLE_GROUP_CONFLICT" => Some("กลุ่มเรียนนี้มีรายการในวันและคาบดังกล่าวแล้ว"),
        "ACADEMIC_TIMETABLE_HOMEROOM_CONFLICT" => Some("ห้องประจำชั้นนี้มีรายการในวันและคาบดังกล่าวแล้ว"),
        "ACADEMIC_TIMETABLE_ROOM_CONFLICT" => Some("ห้องเรียนนี้ถูกใช้ในวันและคาบดังกล่าวแล้ว"),
        "ACADEMIC_TIMETABLE_INSTRUCTOR_DOUBLE_BOOKED" => Some("ครูมีรายการอื่นในวันและคาบดังกล่าวแล้ว"),
        "ACADEMIC_PUBLISHED_TIMETABLE_VERSION_CHILD_IMMUTABLE" => {
            Some("รุ่นตารางสอนที่เผยแพร่แล้วแก้ไขไม่ได้")
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        canonical_ids, normalize_day, normalize_entry_type, timetable_guard_conflict_message,
        wire_entry_type,
    };
    use crate::error::AppError;
    use uuid::Uuid;

    #[test]
    fn canonical_contract_normalizes_day_and_entry_kind() {
        assert_eq!(normalize_day("mon").unwrap(), "MON");
        assert_eq!(normalize_entry_type("activity").unwrap(), "ACTIVITY");
        assert!(matches!(
            normalize_day("holiday"),
            Err(AppError::ValidationError(_))
        ));
    }

    #[test]
    fn timetable_database_guard_codes_have_stable_conflict_messages() {
        for code in [
            "ACADEMIC_TIMETABLE_GROUP_CONFLICT",
            "ACADEMIC_TIMETABLE_HOMEROOM_CONFLICT",
            "ACADEMIC_TIMETABLE_ROOM_CONFLICT",
            "ACADEMIC_TIMETABLE_INSTRUCTOR_DOUBLE_BOOKED",
            "ACADEMIC_PUBLISHED_TIMETABLE_VERSION_CHILD_IMMUTABLE",
        ] {
            assert!(timetable_guard_conflict_message(Some("23514"), code).is_some());
        }
        assert!(timetable_guard_conflict_message(
            Some("23505"),
            "ACADEMIC_TIMETABLE_GROUP_CONFLICT"
        )
        .is_none());
        assert!(timetable_guard_conflict_message(Some("23514"), "UNRELATED_CHECK").is_none());
    }

    #[test]
    fn timetable_wire_kind_uses_the_canonical_uppercase_values() {
        assert_eq!(wire_entry_type("course"), "COURSE");
        assert_eq!(wire_entry_type("ACTIVITY"), "ACTIVITY");
    }

    #[test]
    fn instructor_ids_are_deduplicated_in_stable_order() {
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);
        assert_eq!(canonical_ids(&[second, first, second]), vec![first, second]);
    }
}
