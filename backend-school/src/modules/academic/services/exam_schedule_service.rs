#![allow(dead_code)]

use std::collections::{HashMap, HashSet};

use chrono::{Duration, NaiveDate, NaiveTime};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::{
    BlockedWindow, BlockedWindowInput, CreateExamRoundRequest, ExamDay, ExamDayDetail,
    ExamDayRoomAssignmentView, ExamInvigilatorView, ExamRound, ExamScheduleItemView,
    ExamScheduleReadiness, ExamScheduleWorkspace, ExamSessionView, UpdateExamRoundRequest,
    UpsertExamDayRequest,
};

#[derive(Debug, Clone, Copy)]
pub struct WorkspaceCounts {
    pub day_count: i64,
    pub item_count: i64,
    pub unscheduled_count: i64,
    pub missing_room_assignment_count: i64,
    pub missing_seat_assignment_count: i64,
}

#[derive(Debug, sqlx::FromRow)]
struct ExamDayGradeLevelRow {
    exam_day_id: Uuid,
    grade_level_id: Uuid,
}

#[derive(Debug, sqlx::FromRow)]
struct BlockedWindowRow {
    exam_day_id: Uuid,
    id: Uuid,
    label: String,
    start_time: NaiveTime,
    end_time: NaiveTime,
}

#[derive(Debug, sqlx::FromRow)]
struct ExamDayRoomAssignmentRow {
    id: Uuid,
    exam_day_id: Uuid,
    classroom_id: Uuid,
    room_id: Uuid,
    capacity_override: Option<i32>,
    classroom_name: Option<String>,
    room_name: Option<String>,
    room_capacity: Option<i32>,
}

#[derive(Debug, sqlx::FromRow)]
struct ExamSessionRow {
    id: Uuid,
    exam_schedule_item_id: Uuid,
    exam_round_id: Uuid,
    exam_day_id: Uuid,
    starts_at: NaiveTime,
    ends_at: NaiveTime,
    academic_semester_id: Uuid,
    assessment_category_id: Uuid,
    assessment_plan_id: Uuid,
    classroom_course_id: Uuid,
    classroom_id: Uuid,
    subject_id: Uuid,
    grade_level_id: Uuid,
    duration_minutes: i32,
    exam_date: Option<NaiveDate>,
    assessment_category_name: Option<String>,
    subject_code: Option<String>,
    subject_name_th: Option<String>,
    subject_name_en: Option<String>,
    classroom_name: Option<String>,
    day_room_assignment_id: Option<Uuid>,
    room_id: Option<Uuid>,
    room_name: Option<String>,
    building_name: Option<String>,
}

struct NormalizedUpdateRoundRequest {
    name: Option<String>,
    description: Option<String>,
}

impl ExamDayRoomAssignmentRow {
    fn into_view(self, invigilators: Vec<ExamInvigilatorView>) -> ExamDayRoomAssignmentView {
        ExamDayRoomAssignmentView {
            id: self.id,
            exam_day_id: self.exam_day_id,
            classroom_id: self.classroom_id,
            room_id: self.room_id,
            capacity_override: self.capacity_override,
            classroom_name: self.classroom_name,
            room_name: self.room_name,
            room_capacity: self.room_capacity,
            invigilators,
        }
    }
}

impl ExamSessionRow {
    fn into_view(self, invigilators: Vec<ExamInvigilatorView>) -> ExamSessionView {
        ExamSessionView {
            id: self.id,
            exam_schedule_item_id: self.exam_schedule_item_id,
            exam_round_id: self.exam_round_id,
            exam_day_id: self.exam_day_id,
            starts_at: self.starts_at,
            ends_at: self.ends_at,
            academic_semester_id: self.academic_semester_id,
            assessment_category_id: self.assessment_category_id,
            assessment_plan_id: self.assessment_plan_id,
            classroom_course_id: self.classroom_course_id,
            classroom_id: self.classroom_id,
            subject_id: self.subject_id,
            grade_level_id: self.grade_level_id,
            duration_minutes: self.duration_minutes,
            exam_date: self.exam_date,
            assessment_category_name: self.assessment_category_name,
            subject_code: self.subject_code,
            subject_name_th: self.subject_name_th,
            subject_name_en: self.subject_name_en,
            classroom_name: self.classroom_name,
            room_id: self.room_id,
            room_name: self.room_name,
            building_name: self.building_name,
            invigilators,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum SessionValidationError {
    InvalidDuration,
    EndTimeOverflow,
    BeforeDayStart,
    AfterDayEnd,
    BlockedWindow(String),
}

pub fn add_minutes(start: NaiveTime, minutes: i32) -> Result<NaiveTime, SessionValidationError> {
    if minutes <= 0 {
        return Err(SessionValidationError::InvalidDuration);
    }

    let (end_time, day_delta) = start.overflowing_add_signed(Duration::minutes(i64::from(minutes)));
    if day_delta == 0 {
        Ok(end_time)
    } else {
        Err(SessionValidationError::EndTimeOverflow)
    }
}

pub fn time_ranges_overlap(
    left_start: NaiveTime,
    left_end: NaiveTime,
    right_start: NaiveTime,
    right_end: NaiveTime,
) -> bool {
    left_start < right_end && right_start < left_end
}

pub fn validate_session_window(
    starts_at: NaiveTime,
    duration_minutes: i32,
    day_start: NaiveTime,
    day_end: NaiveTime,
    blocked_windows: &[BlockedWindow],
) -> Result<NaiveTime, SessionValidationError> {
    let ends_at = add_minutes(starts_at, duration_minutes)?;
    if starts_at < day_start {
        return Err(SessionValidationError::BeforeDayStart);
    }
    if ends_at > day_end {
        return Err(SessionValidationError::AfterDayEnd);
    }
    for blocked in blocked_windows {
        if time_ranges_overlap(starts_at, ends_at, blocked.start_time, blocked.end_time) {
            return Err(SessionValidationError::BlockedWindow(blocked.label.clone()));
        }
    }
    Ok(ends_at)
}

pub fn validation_error_to_app_error(error: SessionValidationError) -> AppError {
    match error {
        SessionValidationError::InvalidDuration => {
            AppError::BadRequest("Exam duration must be greater than zero".into())
        }
        SessionValidationError::EndTimeOverflow => {
            AppError::BadRequest("Exam end time is outside the valid day range".into())
        }
        SessionValidationError::BeforeDayStart => {
            AppError::BadRequest("Exam starts before the exam day begins".into())
        }
        SessionValidationError::AfterDayEnd => {
            AppError::BadRequest("Exam ends after the exam day ends".into())
        }
        SessionValidationError::BlockedWindow(label) => {
            AppError::BadRequest(format!("Exam overlaps blocked window: {label}"))
        }
    }
}

pub fn build_readiness(counts: WorkspaceCounts) -> ExamScheduleReadiness {
    let mut blockers = Vec::new();
    if counts.day_count == 0 {
        blockers.push("Add at least one exam day".to_string());
    }
    if counts.item_count == 0 {
        blockers.push("Import in-timetable assessment categories".to_string());
    }
    if counts.unscheduled_count > 0 {
        blockers.push(format!(
            "Schedule {} remaining unscheduled exam item(s)",
            counts.unscheduled_count
        ));
    }
    if counts.missing_room_assignment_count > 0 {
        blockers.push(format!(
            "Assign rooms for {} classroom-day group(s)",
            counts.missing_room_assignment_count
        ));
    }
    if counts.missing_seat_assignment_count > 0 {
        blockers.push(format!(
            "Generate seats for {} classroom-day group(s)",
            counts.missing_seat_assignment_count
        ));
    }
    ExamScheduleReadiness {
        can_publish: blockers.is_empty(),
        blockers,
    }
}

pub async fn list_rounds(
    pool: &PgPool,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<ExamRound>, AppError> {
    let rows = sqlx::query_as::<_, ExamRound>(
        r#"
        SELECT id,
               academic_semester_id,
               name,
               description,
               status,
               published_at,
               created_at,
               updated_at
        FROM academic_exam_rounds
        WHERE ($1::uuid IS NULL OR academic_semester_id = $1)
        ORDER BY created_at DESC, name ASC
        "#,
    )
    .bind(academic_semester_id)
    .fetch_all(pool)
    .await?;

    Ok(rows)
}

pub async fn create_round(
    pool: &PgPool,
    request: CreateExamRoundRequest,
    actor_user_id: Uuid,
) -> Result<ExamRound, AppError> {
    let name = request.name.trim().to_string();
    if name.is_empty() {
        return Err(AppError::BadRequest(
            "Exam round name is required".to_string(),
        ));
    }

    let row = sqlx::query_as::<_, ExamRound>(
        r#"
        INSERT INTO academic_exam_rounds (
            academic_semester_id,
            name,
            description,
            created_by,
            updated_by
        )
        VALUES ($1, $2, $3, $4, $4)
        RETURNING id,
                  academic_semester_id,
                  name,
                  description,
                  status,
                  published_at,
                  created_at,
                  updated_at
        "#,
    )
    .bind(request.academic_semester_id)
    .bind(name)
    .bind(request.description)
    .bind(actor_user_id)
    .fetch_one(pool)
    .await?;

    Ok(row)
}

pub async fn update_round(
    pool: &PgPool,
    round_id: Uuid,
    request: UpdateExamRoundRequest,
    actor_user_id: Uuid,
) -> Result<ExamRound, AppError> {
    let normalized = normalize_update_round_request(request)?;

    let mut tx = pool.begin().await?;
    mark_round_draft_after_mutation(&mut tx, round_id, Some(actor_user_id)).await?;

    let row = sqlx::query_as::<_, ExamRound>(
        r#"
        UPDATE academic_exam_rounds
        SET name = COALESCE($2, name),
            description = COALESCE($3, description),
            updated_by = $4,
            updated_at = now()
        WHERE id = $1
        RETURNING id,
                  academic_semester_id,
                  name,
                  description,
                  status,
                  published_at,
                  created_at,
                  updated_at
        "#,
    )
    .bind(round_id)
    .bind(normalized.name)
    .bind(normalized.description)
    .bind(actor_user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam round not found".to_string()))?;

    tx.commit().await?;
    Ok(row)
}

pub async fn upsert_exam_day(
    pool: &PgPool,
    round_id: Uuid,
    request: UpsertExamDayRequest,
) -> Result<ExamDayDetail, AppError> {
    validate_exam_day_window(request.start_time, request.end_time)?;
    let blocked_windows = normalize_blocked_windows(
        request.start_time,
        request.end_time,
        request.blocked_windows,
    )?;
    let grade_level_ids = unique_uuids(request.grade_level_ids);

    let mut tx = pool.begin().await?;
    let round_exists: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS (
            SELECT 1
            FROM academic_exam_rounds
            WHERE id = $1
        )
        "#,
    )
    .bind(round_id)
    .fetch_one(&mut *tx)
    .await?;

    if !round_exists {
        return Err(AppError::NotFound("Exam round not found".to_string()));
    }

    let day = sqlx::query_as::<_, ExamDay>(
        r#"
        INSERT INTO academic_exam_days (
            exam_round_id,
            exam_date,
            label,
            start_time,
            end_time,
            sort_order
        )
        VALUES ($1, $2, $3, $4, $5, $6)
        ON CONFLICT (exam_round_id, exam_date)
        DO UPDATE SET
            label = EXCLUDED.label,
            start_time = EXCLUDED.start_time,
            end_time = EXCLUDED.end_time,
            sort_order = EXCLUDED.sort_order,
            updated_at = now()
        RETURNING id,
                  exam_round_id,
                  exam_date,
                  label,
                  start_time,
                  end_time,
                  sort_order
        "#,
    )
    .bind(round_id)
    .bind(request.exam_date)
    .bind(request.label)
    .bind(request.start_time)
    .bind(request.end_time)
    .bind(request.sort_order)
    .fetch_one(&mut *tx)
    .await?;

    sqlx::query("DELETE FROM academic_exam_day_grade_levels WHERE exam_day_id = $1")
        .bind(day.id)
        .execute(&mut *tx)
        .await?;

    if !grade_level_ids.is_empty() {
        sqlx::query(
            r#"
            INSERT INTO academic_exam_day_grade_levels (exam_day_id, grade_level_id)
            SELECT $1, grade_level_id
            FROM unnest($2::uuid[]) AS grade_level_id
            ON CONFLICT DO NOTHING
            "#,
        )
        .bind(day.id)
        .bind(&grade_level_ids)
        .execute(&mut *tx)
        .await?;
    }

    sqlx::query("DELETE FROM academic_exam_day_blocked_windows WHERE exam_day_id = $1")
        .bind(day.id)
        .execute(&mut *tx)
        .await?;

    if !blocked_windows.is_empty() {
        let labels: Vec<String> = blocked_windows
            .iter()
            .map(|window| window.label.clone())
            .collect();
        let start_times: Vec<NaiveTime> = blocked_windows
            .iter()
            .map(|window| window.start_time)
            .collect();
        let end_times: Vec<NaiveTime> = blocked_windows
            .iter()
            .map(|window| window.end_time)
            .collect();

        sqlx::query(
            r#"
            INSERT INTO academic_exam_day_blocked_windows (
                exam_day_id,
                label,
                start_time,
                end_time
            )
            SELECT $1, label, start_time, end_time
            FROM unnest($2::text[], $3::time[], $4::time[])
                AS blocked_window(label, start_time, end_time)
            "#,
        )
        .bind(day.id)
        .bind(&labels)
        .bind(&start_times)
        .bind(&end_times)
        .execute(&mut *tx)
        .await?;
    }

    mark_round_draft_after_mutation(&mut tx, round_id, None).await?;
    tx.commit().await?;

    fetch_exam_day_detail(pool, day.id).await
}

pub async fn delete_exam_day(
    pool: &PgPool,
    round_id: Uuid,
    exam_day_id: Uuid,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    let deleted_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        DELETE FROM academic_exam_days
        WHERE exam_round_id = $1
          AND id = $2
        RETURNING id
        "#,
    )
    .bind(round_id)
    .bind(exam_day_id)
    .fetch_optional(&mut *tx)
    .await?;

    if deleted_id.is_none() {
        return Err(AppError::NotFound("Exam day not found".to_string()));
    }

    mark_round_draft_after_mutation(&mut tx, round_id, None).await?;
    tx.commit().await?;

    Ok(())
}

pub async fn get_workspace(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<ExamScheduleWorkspace, AppError> {
    let round = fetch_round(pool, round_id).await?;
    let days = fetch_exam_day_details_for_round(pool, round_id).await?;
    let unscheduled_items = fetch_unscheduled_items(pool, round_id).await?;
    let scheduled_sessions = fetch_scheduled_sessions(pool, round_id).await?;
    let counts = fetch_workspace_counts(pool, round_id).await?;

    Ok(ExamScheduleWorkspace {
        round,
        days,
        unscheduled_items,
        scheduled_sessions,
        readiness: build_readiness(counts),
    })
}

async fn mark_round_draft_after_mutation(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    round_id: Uuid,
    actor_user_id: Option<Uuid>,
) -> Result<(), AppError> {
    sqlx::query(
        r#"
        UPDATE academic_exam_rounds
        SET status = 'draft',
            published_at = NULL,
            published_by = NULL,
            updated_by = COALESCE($2, updated_by),
            updated_at = now()
        WHERE id = $1
        "#,
    )
    .bind(round_id)
    .bind(actor_user_id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

fn validate_exam_day_window(start_time: NaiveTime, end_time: NaiveTime) -> Result<(), AppError> {
    if start_time >= end_time {
        return Err(AppError::BadRequest(
            "Exam day start time must be before end time".to_string(),
        ));
    }
    Ok(())
}

fn normalize_update_round_request(
    request: UpdateExamRoundRequest,
) -> Result<NormalizedUpdateRoundRequest, AppError> {
    if request.name.is_none() && request.description.is_none() {
        return Err(AppError::BadRequest("No fields to update".to_string()));
    }

    let name = match request.name {
        Some(value) => {
            let trimmed = value.trim().to_string();
            if trimmed.is_empty() {
                return Err(AppError::BadRequest(
                    "Exam round name is required".to_string(),
                ));
            }
            Some(trimmed)
        }
        None => None,
    };

    Ok(NormalizedUpdateRoundRequest {
        name,
        description: request.description,
    })
}

fn normalize_blocked_windows(
    day_start_time: NaiveTime,
    day_end_time: NaiveTime,
    blocked_windows: Vec<BlockedWindowInput>,
) -> Result<Vec<BlockedWindowInput>, AppError> {
    let mut normalized = Vec::with_capacity(blocked_windows.len());
    for window in blocked_windows {
        if window.start_time >= window.end_time {
            return Err(AppError::BadRequest(
                "Blocked window start time must be before end time".to_string(),
            ));
        }
        if window.start_time < day_start_time || window.end_time > day_end_time {
            return Err(AppError::BadRequest(
                "Blocked windows must be within the exam day".to_string(),
            ));
        }
        let label = window.label.trim().to_string();
        if label.is_empty() {
            return Err(AppError::BadRequest(
                "Blocked window label is required".to_string(),
            ));
        }
        normalized.push(BlockedWindowInput {
            label,
            start_time: window.start_time,
            end_time: window.end_time,
        });
    }
    Ok(normalized)
}

fn unique_uuids(ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    ids.into_iter().filter(|id| seen.insert(*id)).collect()
}

async fn fetch_round(pool: &PgPool, round_id: Uuid) -> Result<ExamRound, AppError> {
    sqlx::query_as::<_, ExamRound>(
        r#"
        SELECT id,
               academic_semester_id,
               name,
               description,
               status,
               published_at,
               created_at,
               updated_at
        FROM academic_exam_rounds
        WHERE id = $1
        "#,
    )
    .bind(round_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam round not found".to_string()))
}

async fn fetch_exam_day_detail(
    pool: &PgPool,
    exam_day_id: Uuid,
) -> Result<ExamDayDetail, AppError> {
    let day = sqlx::query_as::<_, ExamDay>(
        r#"
        SELECT id,
               exam_round_id,
               exam_date,
               label,
               start_time,
               end_time,
               sort_order
        FROM academic_exam_days
        WHERE id = $1
        "#,
    )
    .bind(exam_day_id)
    .fetch_optional(pool)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam day not found".to_string()))?;

    let mut details = hydrate_exam_day_details(pool, vec![day]).await?;
    details
        .pop()
        .ok_or_else(|| AppError::NotFound("Exam day not found".to_string()))
}

async fn fetch_exam_day_details_for_round(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<ExamDayDetail>, AppError> {
    let days = sqlx::query_as::<_, ExamDay>(
        r#"
        SELECT id,
               exam_round_id,
               exam_date,
               label,
               start_time,
               end_time,
               sort_order
        FROM academic_exam_days
        WHERE exam_round_id = $1
        ORDER BY sort_order ASC, exam_date ASC, id ASC
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await?;

    hydrate_exam_day_details(pool, days).await
}

async fn hydrate_exam_day_details(
    pool: &PgPool,
    days: Vec<ExamDay>,
) -> Result<Vec<ExamDayDetail>, AppError> {
    if days.is_empty() {
        return Ok(Vec::new());
    }

    let day_ids: Vec<Uuid> = days.iter().map(|day| day.id).collect();

    let grade_rows = sqlx::query_as::<_, ExamDayGradeLevelRow>(
        r#"
        SELECT scope.exam_day_id,
               scope.grade_level_id
        FROM academic_exam_day_grade_levels scope
        JOIN grade_levels grade_level ON grade_level.id = scope.grade_level_id
        WHERE scope.exam_day_id = ANY($1)
        ORDER BY scope.exam_day_id,
                 CASE grade_level.level_type
                     WHEN 'kindergarten' THEN 1
                     WHEN 'primary' THEN 2
                     WHEN 'secondary' THEN 3
                     ELSE 4
                 END,
                 grade_level.year,
                 scope.grade_level_id
        "#,
    )
    .bind(&day_ids)
    .fetch_all(pool)
    .await?;

    let blocked_rows = sqlx::query_as::<_, BlockedWindowRow>(
        r#"
        SELECT exam_day_id,
               id,
               label,
               start_time,
               end_time
        FROM academic_exam_day_blocked_windows
        WHERE exam_day_id = ANY($1)
        ORDER BY exam_day_id, start_time, end_time, label, id
        "#,
    )
    .bind(&day_ids)
    .fetch_all(pool)
    .await?;

    let assignment_rows = sqlx::query_as::<_, ExamDayRoomAssignmentRow>(
        r#"
        SELECT assignment.id,
               assignment.exam_day_id,
               assignment.classroom_id,
               assignment.room_id,
               assignment.capacity_override,
               classroom.name AS classroom_name,
               room.name_th AS room_name,
               room.capacity AS room_capacity
        FROM academic_exam_day_room_assignments assignment
        JOIN class_rooms classroom ON classroom.id = assignment.classroom_id
        JOIN rooms room ON room.id = assignment.room_id
        WHERE assignment.exam_day_id = ANY($1)
        ORDER BY assignment.exam_day_id, classroom.name, room.name_th, assignment.id
        "#,
    )
    .bind(&day_ids)
    .fetch_all(pool)
    .await?;

    let assignment_ids: Vec<Uuid> = assignment_rows
        .iter()
        .map(|assignment| assignment.id)
        .collect();
    let mut invigilators_by_assignment =
        fetch_invigilators_by_assignment_ids(pool, &assignment_ids).await?;

    let mut grade_ids_by_day: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for row in grade_rows {
        grade_ids_by_day
            .entry(row.exam_day_id)
            .or_default()
            .push(row.grade_level_id);
    }

    let mut blocked_windows_by_day: HashMap<Uuid, Vec<BlockedWindow>> = HashMap::new();
    for row in blocked_rows {
        blocked_windows_by_day
            .entry(row.exam_day_id)
            .or_default()
            .push(BlockedWindow {
                id: Some(row.id),
                label: row.label,
                start_time: row.start_time,
                end_time: row.end_time,
            });
    }

    let mut assignments_by_day: HashMap<Uuid, Vec<ExamDayRoomAssignmentView>> = HashMap::new();
    for row in assignment_rows {
        let invigilators = invigilators_by_assignment
            .remove(&row.id)
            .unwrap_or_default();
        assignments_by_day
            .entry(row.exam_day_id)
            .or_default()
            .push(row.into_view(invigilators));
    }

    Ok(days
        .into_iter()
        .map(|day| {
            let day_id = day.id;
            ExamDayDetail {
                id: day.id,
                exam_round_id: day.exam_round_id,
                exam_date: day.exam_date,
                label: day.label,
                start_time: day.start_time,
                end_time: day.end_time,
                sort_order: day.sort_order,
                grade_level_ids: grade_ids_by_day.remove(&day_id).unwrap_or_default(),
                blocked_windows: blocked_windows_by_day.remove(&day_id).unwrap_or_default(),
                room_assignments: assignments_by_day.remove(&day_id).unwrap_or_default(),
            }
        })
        .collect())
}

async fn fetch_invigilators_by_assignment_ids(
    pool: &PgPool,
    assignment_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<ExamInvigilatorView>>, AppError> {
    let assignment_ids = unique_uuids(assignment_ids.to_vec());
    if assignment_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, ExamInvigilatorView>(
        r#"
        SELECT invigilator.id,
               invigilator.exam_day_id,
               invigilator.day_room_assignment_id,
               invigilator.staff_id,
               NULLIF(
                   concat_ws(' ', user_account.title, user_account.first_name, user_account.last_name),
                   ''
               ) AS staff_name,
               invigilator.role_label
        FROM academic_exam_day_invigilators invigilator
        JOIN users user_account ON user_account.id = invigilator.staff_id
        WHERE invigilator.day_room_assignment_id = ANY($1)
        ORDER BY invigilator.day_room_assignment_id,
                 invigilator.role_label NULLS LAST,
                 user_account.first_name,
                 user_account.last_name,
                 invigilator.id
        "#,
    )
    .bind(&assignment_ids)
    .fetch_all(pool)
    .await?;

    let mut grouped = HashMap::new();
    for row in rows {
        grouped
            .entry(row.day_room_assignment_id)
            .or_insert_with(Vec::new)
            .push(row);
    }
    Ok(grouped)
}

fn invigilators_for_assignment(
    assignment_id: Option<Uuid>,
    invigilators_by_assignment: &HashMap<Uuid, Vec<ExamInvigilatorView>>,
) -> Vec<ExamInvigilatorView> {
    assignment_id
        .and_then(|assignment_id| invigilators_by_assignment.get(&assignment_id).cloned())
        .unwrap_or_default()
}

async fn fetch_unscheduled_items(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<ExamScheduleItemView>, AppError> {
    sqlx::query_as::<_, ExamScheduleItemView>(
        r#"
        SELECT item.id,
               item.exam_round_id,
               item.academic_semester_id,
               item.assessment_category_id,
               item.assessment_plan_id,
               item.classroom_course_id,
               item.classroom_id,
               item.subject_id,
               item.grade_level_id,
               item.duration_minutes,
               item.imported_at,
               category.name AS assessment_category_name,
               subject.code AS subject_code,
               subject.name_th AS subject_name_th,
               subject.name_en AS subject_name_en,
               classroom.name AS classroom_name,
               CASE grade_level.level_type
                   WHEN 'kindergarten' THEN CONCAT('อ.', grade_level.year)
                   WHEN 'primary' THEN CONCAT('ป.', grade_level.year)
                   WHEN 'secondary' THEN CONCAT('ม.', grade_level.year)
                   ELSE CONCAT('?.', grade_level.year)
               END AS grade_level_name
        FROM academic_exam_schedule_items item
        JOIN academic_assessment_categories category
          ON category.id = item.assessment_category_id
        JOIN subjects subject ON subject.id = item.subject_id
        JOIN class_rooms classroom ON classroom.id = item.classroom_id
        JOIN grade_levels grade_level ON grade_level.id = item.grade_level_id
        WHERE item.exam_round_id = $1
          AND NOT EXISTS (
              SELECT 1
              FROM academic_exam_sessions session
              WHERE session.exam_schedule_item_id = item.id
          )
        ORDER BY CASE grade_level.level_type
                     WHEN 'kindergarten' THEN 1
                     WHEN 'primary' THEN 2
                     WHEN 'secondary' THEN 3
                     ELSE 4
                 END,
                 grade_level.year,
                 classroom.room_number NULLS LAST,
                 classroom.name,
                 subject.code,
                 category.display_order,
                 category.name,
                 item.id
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

async fn fetch_scheduled_sessions(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<ExamSessionView>, AppError> {
    let rows = sqlx::query_as::<_, ExamSessionRow>(
        r#"
        SELECT session.id,
               session.exam_schedule_item_id,
               session.exam_round_id,
               session.exam_day_id,
               session.starts_at,
               session.ends_at,
               item.academic_semester_id,
               item.assessment_category_id,
               item.assessment_plan_id,
               item.classroom_course_id,
               item.classroom_id,
               item.subject_id,
               item.grade_level_id,
               item.duration_minutes,
               day.exam_date AS exam_date,
               category.name AS assessment_category_name,
               subject.code AS subject_code,
               subject.name_th AS subject_name_th,
               subject.name_en AS subject_name_en,
               classroom.name AS classroom_name,
               assignment.id AS day_room_assignment_id,
               assignment.room_id AS room_id,
               room.name_th AS room_name,
               building.name_th AS building_name
        FROM academic_exam_sessions session
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.exam_round_id = session.exam_round_id
        JOIN academic_exam_days day
          ON day.id = session.exam_day_id
         AND day.exam_round_id = session.exam_round_id
        JOIN academic_assessment_categories category
          ON category.id = item.assessment_category_id
        JOIN subjects subject ON subject.id = item.subject_id
        JOIN class_rooms classroom ON classroom.id = item.classroom_id
        LEFT JOIN academic_exam_day_room_assignments assignment
          ON assignment.exam_day_id = session.exam_day_id
         AND assignment.classroom_id = item.classroom_id
        LEFT JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN buildings building ON building.id = room.building_id
        WHERE session.exam_round_id = $1
        ORDER BY day.sort_order,
                 day.exam_date,
                 session.starts_at,
                 classroom.name,
                 subject.code,
                 category.display_order,
                 category.name,
                 session.id
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await?;

    let assignment_ids: Vec<Uuid> = rows
        .iter()
        .filter_map(|row| row.day_room_assignment_id)
        .collect();
    let invigilators_by_assignment =
        fetch_invigilators_by_assignment_ids(pool, &assignment_ids).await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            let invigilators = invigilators_for_assignment(
                row.day_room_assignment_id,
                &invigilators_by_assignment,
            );
            row.into_view(invigilators)
        })
        .collect())
}

async fn fetch_workspace_counts(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<WorkspaceCounts, AppError> {
    let (
        day_count,
        item_count,
        unscheduled_count,
        missing_room_assignment_count,
        missing_seat_assignment_count,
    ): (i64, i64, i64, i64, i64) = sqlx::query_as(
        r#"
        SELECT (
                   SELECT COUNT(*)::BIGINT
                   FROM academic_exam_days day
                   WHERE day.exam_round_id = $1
               ) AS day_count,
               (
                   SELECT COUNT(*)::BIGINT
                   FROM academic_exam_schedule_items item
                   WHERE item.exam_round_id = $1
               ) AS item_count,
               (
                   SELECT COUNT(*)::BIGINT
                   FROM academic_exam_schedule_items item
                   WHERE item.exam_round_id = $1
                     AND NOT EXISTS (
                         SELECT 1
                         FROM academic_exam_sessions session
                         WHERE session.exam_schedule_item_id = item.id
                     )
               ) AS unscheduled_count,
               (
                   SELECT COUNT(*)::BIGINT
                   FROM (
                       SELECT DISTINCT session.exam_day_id,
                                       item.classroom_id
                       FROM academic_exam_sessions session
                       JOIN academic_exam_schedule_items item
                         ON item.id = session.exam_schedule_item_id
                        AND item.exam_round_id = session.exam_round_id
                       LEFT JOIN academic_exam_day_room_assignments assignment
                         ON assignment.exam_day_id = session.exam_day_id
                        AND assignment.classroom_id = item.classroom_id
                       WHERE session.exam_round_id = $1
                         AND assignment.id IS NULL
                   ) missing_room_assignments
               ) AS missing_room_assignment_count,
               (
                   SELECT COUNT(*)::BIGINT
                   FROM (
                       SELECT DISTINCT session.exam_day_id,
                                       item.classroom_id
                       FROM academic_exam_sessions session
                       JOIN academic_exam_schedule_items item
                         ON item.id = session.exam_schedule_item_id
                        AND item.exam_round_id = session.exam_round_id
                       LEFT JOIN academic_exam_day_room_assignments assignment
                         ON assignment.exam_day_id = session.exam_day_id
                        AND assignment.classroom_id = item.classroom_id
                       WHERE session.exam_round_id = $1
                         AND assignment.id IS NOT NULL
                         AND NOT EXISTS (
                             SELECT 1
                             FROM academic_exam_seat_assignments seat
                             WHERE seat.day_room_assignment_id = assignment.id
                         )
                   ) missing_seat_assignments
               ) AS missing_seat_assignment_count
        "#,
    )
    .bind(round_id)
    .fetch_one(pool)
    .await?;

    Ok(WorkspaceCounts {
        day_count,
        item_count,
        unscheduled_count,
        missing_room_assignment_count,
        missing_seat_assignment_count,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn t(value: &str) -> NaiveTime {
        NaiveTime::parse_from_str(value, "%H:%M").unwrap()
    }

    #[test]
    fn computes_end_time_from_duration() {
        assert_eq!(add_minutes(t("08:30"), 90).unwrap(), t("10:00"));
    }

    #[test]
    fn rejects_zero_duration() {
        assert_eq!(
            add_minutes(t("08:30"), 0),
            Err(SessionValidationError::InvalidDuration)
        );
    }

    #[test]
    fn rejects_negative_duration() {
        assert_eq!(
            add_minutes(t("08:30"), -30),
            Err(SessionValidationError::InvalidDuration)
        );
    }

    #[test]
    fn rejects_end_time_overflow() {
        assert_eq!(
            add_minutes(t("23:30"), 60),
            Err(SessionValidationError::EndTimeOverflow)
        );
    }

    #[test]
    fn detects_half_open_time_overlap() {
        assert!(time_ranges_overlap(
            t("08:30"),
            t("10:00"),
            t("09:59"),
            t("11:00")
        ));
        assert!(!time_ranges_overlap(
            t("08:30"),
            t("10:00"),
            t("10:00"),
            t("11:00")
        ));
    }

    #[test]
    fn rejects_placement_outside_day_window() {
        let outcome = validate_session_window(
            t("08:00"),
            120,
            t("08:30"),
            t("16:00"),
            &[BlockedWindow {
                id: None,
                label: "Lunch".to_string(),
                start_time: t("12:00"),
                end_time: t("13:00"),
            }],
        );
        assert!(matches!(
            outcome,
            Err(SessionValidationError::BeforeDayStart)
        ));
    }

    #[test]
    fn rejects_placement_after_day_end() {
        let outcome = validate_session_window(t("15:30"), 60, t("08:30"), t("16:00"), &[]);
        assert!(matches!(outcome, Err(SessionValidationError::AfterDayEnd)));
    }

    #[test]
    fn rejects_placement_across_blocked_window() {
        let outcome = validate_session_window(
            t("11:30"),
            90,
            t("08:30"),
            t("16:00"),
            &[BlockedWindow {
                id: None,
                label: "Lunch".to_string(),
                start_time: t("12:00"),
                end_time: t("13:00"),
            }],
        );
        assert!(matches!(
            outcome,
            Err(SessionValidationError::BlockedWindow(_))
        ));
    }

    #[test]
    fn readiness_requires_days_items_rooms_and_sessions() {
        let readiness = build_readiness(WorkspaceCounts {
            day_count: 0,
            item_count: 4,
            unscheduled_count: 4,
            missing_room_assignment_count: 2,
            missing_seat_assignment_count: 2,
        });
        assert!(!readiness.can_publish);
        assert!(readiness
            .blockers
            .iter()
            .any(|value| value.contains("exam day")));
        assert!(readiness
            .blockers
            .iter()
            .any(|value| value.contains("unscheduled")));
    }

    #[test]
    fn rejects_round_update_without_supplied_fields() {
        let result = normalize_update_round_request(UpdateExamRoundRequest {
            name: None,
            description: None,
        });

        assert!(matches!(
            result,
            Err(AppError::BadRequest(message)) if message.contains("No fields")
        ));
    }

    #[test]
    fn rejects_blocked_windows_outside_exam_day_range() {
        let result = normalize_blocked_windows(
            t("08:30"),
            t("16:00"),
            vec![BlockedWindowInput {
                label: "Before school".to_string(),
                start_time: t("08:00"),
                end_time: t("08:45"),
            }],
        );

        assert!(matches!(
            result,
            Err(AppError::BadRequest(message)) if message.contains("within the exam day")
        ));
    }

    #[test]
    fn shared_assignment_invigilators_are_reused_for_each_session() {
        let assignment_id = Uuid::from_u128(1);
        let invigilator = ExamInvigilatorView {
            id: Uuid::from_u128(2),
            exam_day_id: Uuid::from_u128(3),
            day_room_assignment_id: assignment_id,
            staff_id: Uuid::from_u128(4),
            staff_name: Some("Exam Staff".to_string()),
            role_label: Some("Lead".to_string()),
        };
        let invigilators_by_assignment =
            HashMap::from([(assignment_id, vec![invigilator.clone()])]);

        let first = invigilators_for_assignment(Some(assignment_id), &invigilators_by_assignment);
        let second = invigilators_for_assignment(Some(assignment_id), &invigilators_by_assignment);

        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
        assert_eq!(first[0].id, invigilator.id);
        assert_eq!(second[0].id, invigilator.id);
        assert!(invigilators_by_assignment.contains_key(&assignment_id));
    }
}
