use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::{
    BlockedWindow, ExamInvigilatorView, ExamSessionView, PlaceExamSessionRequest,
};

use super::invigilation::{
    fetch_invigilator_staff_ids_for_assignment, fetch_invigilators_by_assignment_ids,
    invigilators_for_assignment, lock_exam_invigilator_staff_conflict_scope,
    validate_invigilator_candidate_session_conflicts,
};
use super::rounds_and_days::mark_round_draft_after_mutation;
use super::shared::{
    exam_session_conflict_lock_keys, has_same_classroom_conflict, has_same_room_conflict,
    validate_session_window, validation_error_to_app_error, CandidateRoomSession, CandidateSession,
};

#[derive(Debug, sqlx::FromRow)]
pub(super) struct ExamSessionRow {
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
    imported_at: DateTime<Utc>,
    exam_date: Option<NaiveDate>,
    assessment_category_name: Option<String>,
    subject_code: Option<String>,
    subject_name_th: Option<String>,
    subject_name_en: Option<String>,
    subject_group_id: Option<Uuid>,
    subject_group_name: Option<String>,
    subject_group_display_order: Option<i32>,
    subject_type: Option<String>,
    classroom_name: Option<String>,
    grade_level_name: Option<String>,
    grade_level_type: Option<String>,
    grade_level_year: Option<i32>,
    pub(super) day_room_assignment_id: Option<Uuid>,
    room_id: Option<Uuid>,
    room_name: Option<String>,
    building_name: Option<String>,
}
impl ExamSessionRow {
    pub(super) fn into_view(self, invigilators: Vec<ExamInvigilatorView>) -> ExamSessionView {
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
            imported_at: self.imported_at,
            exam_date: self.exam_date,
            assessment_category_name: self.assessment_category_name,
            subject_code: self.subject_code,
            subject_name_th: self.subject_name_th,
            subject_name_en: self.subject_name_en,
            subject_group_id: self.subject_group_id,
            subject_group_name: self.subject_group_name,
            subject_group_display_order: self.subject_group_display_order,
            subject_type: self.subject_type,
            classroom_name: self.classroom_name,
            grade_level_name: self.grade_level_name,
            grade_level_type: self.grade_level_type,
            grade_level_year: self.grade_level_year,
            room_id: self.room_id,
            room_name: self.room_name,
            building_name: self.building_name,
            invigilators,
        }
    }
}

#[derive(Debug, sqlx::FromRow)]
struct ExamScheduleItemPlacementContext {
    id: Uuid,
    exam_round_id: Uuid,
    academic_semester_id: Uuid,
    assessment_category_id: Uuid,
    assessment_plan_id: Uuid,
    classroom_course_id: Uuid,
    classroom_id: Uuid,
    subject_id: Uuid,
    grade_level_id: Uuid,
    duration_minutes: i32,
}
#[derive(Debug, sqlx::FromRow)]
struct ExamDayPlacementContext {
    id: Uuid,
    exam_round_id: Uuid,
    start_time: NaiveTime,
    end_time: NaiveTime,
}
#[derive(Debug, sqlx::FromRow)]
struct DayRoomAssignmentPlacementContext {
    id: Uuid,
    room_id: Uuid,
}
async fn lock_exam_session_conflict_scope(
    tx: &mut Transaction<'_, Postgres>,
    exam_day_id: Uuid,
    classroom_id: Uuid,
    room_id: Uuid,
) -> Result<(), AppError> {
    for lock_key in exam_session_conflict_lock_keys(exam_day_id, classroom_id, room_id) {
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(lock_key)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}
pub async fn place_exam_session(
    pool: &PgPool,
    request: PlaceExamSessionRequest,
    actor_user_id: Uuid,
) -> Result<ExamSessionView, AppError> {
    let mut tx = pool.begin().await?;

    let item =
        fetch_schedule_item_placement_context(&mut tx, request.exam_schedule_item_id).await?;
    let day = fetch_exam_day_placement_context(&mut tx, request.exam_day_id).await?;
    if day.exam_round_id != item.exam_round_id {
        return Err(AppError::BadRequest(
            "Exam day belongs to a different exam round".to_string(),
        ));
    }

    validate_day_allows_grade_level(&mut tx, day.id, item.grade_level_id).await?;
    let blocked_windows = fetch_blocked_windows_for_day_for_placement(&mut tx, day.id).await?;
    let ends_at = validate_session_window(
        request.starts_at,
        item.duration_minutes,
        day.start_time,
        day.end_time,
        &blocked_windows,
    )
    .map_err(validation_error_to_app_error)?;

    let day_room_assignment =
        fetch_day_room_assignment_placement_context(&mut tx, day.id, item.classroom_id).await?;
    lock_exam_session_conflict_scope(
        &mut tx,
        day.id,
        item.classroom_id,
        day_room_assignment.room_id,
    )
    .await?;
    let existing_session_id =
        fetch_existing_session_id_for_item(&mut tx, request.exam_schedule_item_id).await?;

    let candidate = CandidateSession {
        session_id: existing_session_id,
        classroom_id: item.classroom_id,
        exam_day_id: day.id,
        starts_at: request.starts_at,
        ends_at,
    };
    let existing_classroom_sessions = fetch_candidate_sessions_for_day(&mut tx, day.id).await?;
    if has_same_classroom_conflict(&candidate, &existing_classroom_sessions) {
        return Err(AppError::BadRequest(
            "Classroom already has an exam session during this time".to_string(),
        ));
    }

    let room_candidate = CandidateRoomSession {
        session_id: existing_session_id,
        room_id: day_room_assignment.room_id,
        exam_day_id: day.id,
        starts_at: request.starts_at,
        ends_at,
    };
    let existing_room_sessions = fetch_candidate_room_sessions_for_day(&mut tx, day.id).await?;
    if has_same_room_conflict(&room_candidate, &existing_room_sessions) {
        return Err(AppError::BadRequest(
            "Room already has an exam session during this time".to_string(),
        ));
    }

    let invigilator_staff_ids =
        fetch_invigilator_staff_ids_for_assignment(&mut tx, day_room_assignment.id).await?;
    lock_exam_invigilator_staff_conflict_scope(&mut tx, day.id, &invigilator_staff_ids).await?;
    validate_invigilator_candidate_session_conflicts(
        &mut tx,
        item.exam_round_id,
        day_room_assignment.id,
        day.id,
        request.starts_at,
        ends_at,
        &invigilator_staff_ids,
    )
    .await?;

    let session_id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO academic_exam_sessions (
            exam_schedule_item_id,
            exam_round_id,
            exam_day_id,
            starts_at,
            ends_at,
            created_by,
            updated_by
        )
        VALUES ($1, $2, $3, $4, $5, $6, $6)
        ON CONFLICT (exam_schedule_item_id)
        DO UPDATE SET
            exam_round_id = EXCLUDED.exam_round_id,
            exam_day_id = EXCLUDED.exam_day_id,
            starts_at = EXCLUDED.starts_at,
            ends_at = EXCLUDED.ends_at,
            updated_by = EXCLUDED.updated_by,
            updated_at = now()
        RETURNING id
        "#,
    )
    .bind(item.id)
    .bind(item.exam_round_id)
    .bind(day.id)
    .bind(request.starts_at)
    .bind(ends_at)
    .bind(actor_user_id)
    .fetch_one(&mut *tx)
    .await?;

    mark_round_draft_after_mutation(&mut tx, item.exam_round_id, Some(actor_user_id)).await?;
    tx.commit().await?;

    fetch_exam_session_view(pool, session_id).await
}
pub async fn delete_exam_session(
    pool: &PgPool,
    session_id: Uuid,
    actor_user_id: Uuid,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    let round_id: Uuid = sqlx::query_scalar(
        r#"
        SELECT exam_round_id
        FROM academic_exam_sessions
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(session_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam session not found".to_string()))?;

    sqlx::query("DELETE FROM academic_exam_sessions WHERE id = $1")
        .bind(session_id)
        .execute(&mut *tx)
        .await?;

    mark_round_draft_after_mutation(&mut tx, round_id, Some(actor_user_id)).await?;
    tx.commit().await?;

    Ok(())
}
async fn fetch_schedule_item_placement_context(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_schedule_item_id: Uuid,
) -> Result<ExamScheduleItemPlacementContext, AppError> {
    sqlx::query_as::<_, ExamScheduleItemPlacementContext>(
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
               item.duration_minutes
        FROM academic_exam_schedule_items item
        JOIN academic_exam_rounds round ON round.id = item.exam_round_id
        WHERE item.id = $1
        FOR UPDATE OF item
        "#,
    )
    .bind(exam_schedule_item_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam schedule item not found".to_string()))
}
async fn fetch_exam_day_placement_context(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_day_id: Uuid,
) -> Result<ExamDayPlacementContext, AppError> {
    sqlx::query_as::<_, ExamDayPlacementContext>(
        r#"
        SELECT id,
               exam_round_id,
               start_time,
               end_time
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
async fn fetch_blocked_windows_for_day_for_placement(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_day_id: Uuid,
) -> Result<Vec<BlockedWindow>, AppError> {
    sqlx::query_as::<_, BlockedWindow>(
        r#"
        SELECT id,
               label,
               start_time,
               end_time
        FROM academic_exam_day_blocked_windows
        WHERE exam_day_id = $1
        ORDER BY start_time, end_time, label, id
        "#,
    )
    .bind(exam_day_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(AppError::from)
}
async fn fetch_day_room_assignment_placement_context(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_day_id: Uuid,
    classroom_id: Uuid,
) -> Result<DayRoomAssignmentPlacementContext, AppError> {
    sqlx::query_as::<_, DayRoomAssignmentPlacementContext>(
        r#"
        SELECT id,
               room_id
        FROM academic_exam_day_room_assignments
        WHERE exam_day_id = $1
          AND classroom_id = $2
        FOR UPDATE
        "#,
    )
    .bind(exam_day_id)
    .bind(classroom_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| {
        AppError::BadRequest(
            "Assign an exam room for this classroom before placing the session".to_string(),
        )
    })
}
async fn fetch_existing_session_id_for_item(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_schedule_item_id: Uuid,
) -> Result<Option<Uuid>, AppError> {
    sqlx::query_scalar(
        r#"
        SELECT id
        FROM academic_exam_sessions
        WHERE exam_schedule_item_id = $1
        "#,
    )
    .bind(exam_schedule_item_id)
    .fetch_optional(&mut **tx)
    .await
    .map_err(AppError::from)
}
async fn fetch_candidate_sessions_for_day(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_day_id: Uuid,
) -> Result<Vec<CandidateSession>, AppError> {
    let rows = sqlx::query_as::<_, (Uuid, Uuid, Uuid, NaiveTime, NaiveTime)>(
        r#"
        SELECT session.id,
               item.classroom_id,
               session.exam_day_id,
               session.starts_at,
               session.ends_at
        FROM academic_exam_sessions session
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.exam_round_id = session.exam_round_id
        WHERE session.exam_day_id = $1
        "#,
    )
    .bind(exam_day_id)
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(session_id, classroom_id, exam_day_id, starts_at, ends_at)| CandidateSession {
                session_id: Some(session_id),
                classroom_id,
                exam_day_id,
                starts_at,
                ends_at,
            },
        )
        .collect())
}
async fn fetch_candidate_room_sessions_for_day(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_day_id: Uuid,
) -> Result<Vec<CandidateRoomSession>, AppError> {
    let rows = sqlx::query_as::<_, (Uuid, Uuid, Uuid, NaiveTime, NaiveTime)>(
        r#"
        SELECT session.id,
               assignment.room_id,
               session.exam_day_id,
               session.starts_at,
               session.ends_at
        FROM academic_exam_sessions session
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.exam_round_id = session.exam_round_id
        JOIN academic_exam_day_room_assignments assignment
          ON assignment.exam_day_id = session.exam_day_id
         AND assignment.classroom_id = item.classroom_id
        WHERE session.exam_day_id = $1
        "#,
    )
    .bind(exam_day_id)
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows
        .into_iter()
        .map(
            |(session_id, room_id, exam_day_id, starts_at, ends_at)| CandidateRoomSession {
                session_id: Some(session_id),
                room_id,
                exam_day_id,
                starts_at,
                ends_at,
            },
        )
        .collect())
}
async fn fetch_exam_session_view(
    pool: &PgPool,
    session_id: Uuid,
) -> Result<ExamSessionView, AppError> {
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
               item.imported_at,
               day.exam_date AS exam_date,
               category.name AS assessment_category_name,
               subject.code AS subject_code,
               subject.name_th AS subject_name_th,
               subject.name_en AS subject_name_en,
               subject.group_id AS subject_group_id,
               subject_group.name_th AS subject_group_name,
               subject_group.display_order AS subject_group_display_order,
               subject.type AS subject_type,
               classroom.name AS classroom_name,
               CASE grade_level.level_type
                   WHEN 'kindergarten' THEN CONCAT('อ.', grade_level.year)
                   WHEN 'primary' THEN CONCAT('ป.', grade_level.year)
                   WHEN 'secondary' THEN CONCAT('ม.', grade_level.year)
                   ELSE CONCAT('?.', grade_level.year)
               END AS grade_level_name,
               grade_level.level_type AS grade_level_type,
               grade_level.year AS grade_level_year,
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
        LEFT JOIN subject_groups subject_group ON subject_group.id = subject.group_id
        JOIN class_rooms classroom ON classroom.id = item.classroom_id
        JOIN grade_levels grade_level ON grade_level.id = item.grade_level_id
        LEFT JOIN academic_exam_day_room_assignments assignment
          ON assignment.exam_day_id = session.exam_day_id
         AND assignment.classroom_id = item.classroom_id
        LEFT JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN buildings building ON building.id = room.building_id
        WHERE session.id = $1
        "#,
    )
    .bind(session_id)
    .fetch_all(pool)
    .await?;

    let assignment_ids: Vec<Uuid> = rows
        .iter()
        .filter_map(|row| row.day_room_assignment_id)
        .collect();
    let invigilators_by_assignment =
        fetch_invigilators_by_assignment_ids(pool, &assignment_ids).await?;

    rows.into_iter()
        .map(|row| {
            let invigilators = invigilators_for_assignment(
                row.day_room_assignment_id,
                &invigilators_by_assignment,
            );
            row.into_view(invigilators)
        })
        .next()
        .ok_or_else(|| AppError::NotFound("Exam session not found".to_string()))
}
pub(super) async fn validate_day_allows_grade_level(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    exam_day_id: Uuid,
    grade_level_id: Uuid,
) -> Result<(), AppError> {
    let scoped_grade_level_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"
        SELECT grade_level_id
        FROM academic_exam_day_grade_levels
        WHERE exam_day_id = $1
        ORDER BY grade_level_id
        "#,
    )
    .bind(exam_day_id)
    .fetch_all(&mut **tx)
    .await?;

    if !grade_level_allowed_by_day_scope(grade_level_id, &scoped_grade_level_ids) {
        return Err(AppError::BadRequest(
            "Classroom grade level is not allowed on this exam day".to_string(),
        ));
    }
    Ok(())
}
pub(super) fn grade_level_allowed_by_day_scope(
    grade_level_id: Uuid,
    scoped_grade_level_ids: &[Uuid],
) -> bool {
    scoped_grade_level_ids.is_empty() || scoped_grade_level_ids.contains(&grade_level_id)
}
