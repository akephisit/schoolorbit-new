use std::collections::HashMap;

use chrono::NaiveTime;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::{
    BlockedWindow, BlockedWindowInput, CreateExamRoundRequest, ExamDay, ExamDayDetail,
    ExamDayRoomAssignmentView, ExamInvigilatorView, ExamRound, UpdateExamRoundRequest,
    UpsertExamDayRequest,
};

use super::fetch_invigilators_by_assignment_ids;
use super::shared::unique_uuids;

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
pub(super) struct ExamDayContext {
    pub(super) exam_round_id: Uuid,
}

pub(super) struct NormalizedUpdateRoundRequest {
    name: Option<String>,
    description: Option<String>,
    exam_kind: Option<String>,
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
               exam_kind,
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
    let exam_kind = normalize_exam_kind(request.exam_kind.as_deref())?;

    let row = sqlx::query_as::<_, ExamRound>(
        r#"
        INSERT INTO academic_exam_rounds (
            academic_semester_id,
            name,
            description,
            exam_kind,
            created_by,
            updated_by
        )
        VALUES ($1, $2, $3, $4, $5, $5)
        RETURNING id,
                  academic_semester_id,
                  name,
                  description,
                  exam_kind,
                  status,
                  published_at,
                  created_at,
                  updated_at
        "#,
    )
    .bind(request.academic_semester_id)
    .bind(name)
    .bind(request.description)
    .bind(exam_kind)
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
            exam_kind = COALESCE($4, exam_kind),
            updated_by = $5,
            updated_at = now()
        WHERE id = $1
        RETURNING id,
                  academic_semester_id,
                  name,
                  description,
                  exam_kind,
                  status,
                  published_at,
                  created_at,
                  updated_at
        "#,
    )
    .bind(round_id)
    .bind(normalized.name)
    .bind(normalized.description)
    .bind(normalized.exam_kind)
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
            end_time
        )
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (exam_round_id, exam_date)
        DO UPDATE SET
            label = EXCLUDED.label,
            start_time = EXCLUDED.start_time,
            end_time = EXCLUDED.end_time,
            updated_at = now()
        RETURNING id,
                  exam_round_id,
                  exam_date,
                  label,
                  start_time,
                  end_time
        "#,
    )
    .bind(round_id)
    .bind(request.exam_date)
    .bind(request.label)
    .bind(request.start_time)
    .bind(request.end_time)
    .fetch_one(&mut *tx)
    .await?;

    replace_exam_day_configuration(&mut tx, day.id, &grade_level_ids, &blocked_windows).await?;

    mark_round_draft_after_mutation(&mut tx, round_id, None).await?;
    tx.commit().await?;

    fetch_exam_day_detail(pool, day.id).await
}

pub async fn update_exam_day(
    pool: &PgPool,
    exam_day_id: Uuid,
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
    let day = sqlx::query_as::<_, ExamDay>(
        r#"
        UPDATE academic_exam_days
        SET exam_date = $2,
            label = $3,
            start_time = $4,
            end_time = $5,
            updated_at = now()
        WHERE id = $1
        RETURNING id,
                  exam_round_id,
                  exam_date,
                  label,
                  start_time,
                  end_time
        "#,
    )
    .bind(exam_day_id)
    .bind(request.exam_date)
    .bind(request.label)
    .bind(request.start_time)
    .bind(request.end_time)
    .fetch_optional(&mut *tx)
    .await
    .map_err(map_exam_day_write_error)?
    .ok_or_else(|| AppError::NotFound("Exam day not found".to_string()))?;

    replace_exam_day_configuration(&mut tx, day.id, &grade_level_ids, &blocked_windows).await?;

    mark_round_draft_after_mutation(&mut tx, day.exam_round_id, None).await?;
    tx.commit().await?;

    fetch_exam_day_detail(pool, day.id).await
}

pub(super) async fn replace_exam_day_configuration(
    tx: &mut Transaction<'_, Postgres>,
    exam_day_id: Uuid,
    grade_level_ids: &[Uuid],
    blocked_windows: &[BlockedWindowInput],
) -> Result<(), AppError> {
    sqlx::query("DELETE FROM academic_exam_day_grade_levels WHERE exam_day_id = $1")
        .bind(exam_day_id)
        .execute(&mut **tx)
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
        .bind(exam_day_id)
        .bind(grade_level_ids)
        .execute(&mut **tx)
        .await?;
    }

    sqlx::query("DELETE FROM academic_exam_day_blocked_windows WHERE exam_day_id = $1")
        .bind(exam_day_id)
        .execute(&mut **tx)
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
        .bind(exam_day_id)
        .bind(&labels)
        .bind(&start_times)
        .bind(&end_times)
        .execute(&mut **tx)
        .await?;
    }

    Ok(())
}

pub async fn delete_exam_day(pool: &PgPool, exam_day_id: Uuid) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    let round_id: Option<Uuid> = sqlx::query_scalar(
        r#"
        DELETE FROM academic_exam_days
        WHERE id = $1
        RETURNING exam_round_id
        "#,
    )
    .bind(exam_day_id)
    .fetch_optional(&mut *tx)
    .await?;

    let round_id = round_id.ok_or_else(|| AppError::NotFound("Exam day not found".to_string()))?;

    mark_round_draft_after_mutation(&mut tx, round_id, None).await?;
    tx.commit().await?;

    Ok(())
}

pub(super) async fn mark_round_draft_after_mutation(
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

pub(super) fn ensure_exam_round_is_mutable(status: &str) -> Result<(), AppError> {
    if status == "published" {
        return Err(AppError::BadRequest(
            "Published exam rounds cannot be changed".to_string(),
        ));
    }

    Ok(())
}

pub(super) async fn fetch_exam_day_context_for_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_day_id: Uuid,
) -> Result<ExamDayContext, AppError> {
    sqlx::query_as::<_, ExamDayContext>(
        r#"
        SELECT exam_round_id
        FROM academic_exam_days
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(exam_day_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam day not found".to_string()))
}

pub(super) fn map_exam_day_write_error(error: sqlx::Error) -> AppError {
    if let sqlx::Error::Database(database_error) = &error {
        if database_error.code().as_deref() == Some("23505") {
            return AppError::BadRequest("วันที่นี้มีวันสอบอยู่แล้ว กรุณาย้ายวันนั้นไปวันที่ว่างก่อน".to_string());
        }
    }
    AppError::from(error)
}

pub(super) fn validate_exam_day_window(
    start_time: NaiveTime,
    end_time: NaiveTime,
) -> Result<(), AppError> {
    if start_time >= end_time {
        return Err(AppError::BadRequest(
            "Exam day start time must be before end time".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn normalize_exam_kind(value: Option<&str>) -> Result<String, AppError> {
    let normalized = value.unwrap_or("midterm").trim();
    match normalized {
        "midterm" | "final" => Ok(normalized.to_string()),
        _ => Err(AppError::BadRequest(
            "Exam round kind must be midterm or final".to_string(),
        )),
    }
}

pub(super) fn normalize_update_round_request(
    request: UpdateExamRoundRequest,
) -> Result<NormalizedUpdateRoundRequest, AppError> {
    if request.name.is_none() && request.description.is_none() && request.exam_kind.is_none() {
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
        exam_kind: match request.exam_kind {
            Some(value) => Some(normalize_exam_kind(Some(&value))?),
            None => None,
        },
    })
}

pub(super) fn normalize_blocked_windows(
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

pub(super) async fn fetch_round(pool: &PgPool, round_id: Uuid) -> Result<ExamRound, AppError> {
    sqlx::query_as::<_, ExamRound>(
        r#"
        SELECT id,
               academic_semester_id,
               name,
               description,
               exam_kind,
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

pub(super) async fn fetch_exam_day_detail(
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
               end_time
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

pub(super) async fn fetch_exam_day_details_for_round(
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
               end_time
        FROM academic_exam_days
        WHERE exam_round_id = $1
        ORDER BY exam_date ASC, start_time ASC, id ASC
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await?;

    hydrate_exam_day_details(pool, days).await
}

pub(super) async fn hydrate_exam_day_details(
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
                grade_level_ids: grade_ids_by_day.remove(&day_id).unwrap_or_default(),
                blocked_windows: blocked_windows_by_day.remove(&day_id).unwrap_or_default(),
                room_assignments: assignments_by_day.remove(&day_id).unwrap_or_default(),
            }
        })
        .collect())
}
