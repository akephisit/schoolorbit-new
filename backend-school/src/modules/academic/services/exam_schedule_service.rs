#![allow(dead_code)]

mod published_views;
mod room_assignments;
mod rounds_and_days;
mod shared;
mod workspace;

pub use self::published_views::{
    list_child_published_exam_schedule, list_my_published_exam_schedule,
    list_staff_published_exam_schedule,
};
pub use self::room_assignments::{
    generate_seats_for_assignment, list_day_room_assignments, upsert_day_room_assignment,
};
pub use self::rounds_and_days::{
    create_round, delete_exam_day, list_rounds, update_exam_day, update_round, upsert_exam_day,
};
pub use self::workspace::{clear_mismatched_exam_items, get_workspace, import_exam_items};

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use chrono::{DateTime, NaiveDate, NaiveTime, Utc};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::{
    BlockedWindow, DayRoomAssignmentView, ExamInvigilatorAssignmentSummary,
    ExamInvigilatorDayWorkload, ExamInvigilatorStaffOption, ExamInvigilatorStaffWorkload,
    ExamInvigilatorView, ExamInvigilatorWorkspace, ExamRound, ExamSessionView, InvigilatorView,
    PersonalExamScheduleRound, PersonalExamSessionView, PlaceExamSessionRequest,
    UpdateExamInvigilatorsRequest,
};

#[cfg(test)]
use crate::modules::academic::models::exam_schedule::{
    BlockedWindowInput, UpdateExamRoundRequest, UpsertDayRoomAssignmentRequest,
};

use self::room_assignments::{
    fetch_day_room_assignment_view, fetch_seat_assignment_context,
    map_day_room_assignment_write_error,
};
use self::rounds_and_days::{
    ensure_exam_round_is_mutable, fetch_round, mark_round_draft_after_mutation,
};
use self::shared::{
    exam_invigilator_staff_lock_keys, exam_session_conflict_lock_keys,
    has_invigilator_time_conflict, has_same_classroom_conflict, has_same_room_conflict,
    minutes_between_times, unique_uuids, validate_session_window, validation_error_to_app_error,
    CandidateRoomSession, CandidateSession, InvigilatorSessionWindow,
};
use self::workspace::{build_readiness, fetch_workspace_counts_in_tx, ExamSessionRow};

#[cfg(test)]
use self::room_assignments::{
    build_default_seat_assignments, validate_seat_generation_capacity, SeatStudent,
};
#[cfg(test)]
use self::rounds_and_days::{
    normalize_blocked_windows, normalize_exam_kind, normalize_update_round_request,
};
#[cfg(test)]
use self::shared::{
    add_minutes, invigilator_workload_minutes, time_ranges_overlap, SessionValidationError,
};
#[cfg(test)]
use self::workspace::{WorkspaceCounts, WORKSPACE_COUNTS_SQL};

const INVIGILATOR_STAFF_OPTION_DEFAULT_LIMIT: i64 = 40;
const INVIGILATOR_STAFF_OPTION_MAX_LIMIT: i64 = 100;

#[derive(Debug, sqlx::FromRow)]
struct InvigilatorViewRow {
    day_room_assignment_id: Uuid,
    staff_id: Uuid,
    display_name: String,
}

#[derive(Debug, sqlx::FromRow)]
struct InvigilatorAssignmentSummaryRow {
    assignment_id: Uuid,
    exam_day_id: Uuid,
    classroom_id: Uuid,
    classroom_name: String,
    room_id: Uuid,
    room_name: String,
    session_minutes: i32,
}

#[derive(Debug, sqlx::FromRow)]
struct InvigilatorSessionWindowRow {
    assignment_id: Uuid,
    exam_day_id: Uuid,
    staff_id: Uuid,
    staff_name: String,
    starts_at: NaiveTime,
    ends_at: NaiveTime,
}

#[derive(Debug, sqlx::FromRow)]
struct InvigilatorAssignmentMutationContext {
    assignment_id: Uuid,
    exam_day_id: Uuid,
    exam_round_id: Uuid,
    round_status: String,
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

#[derive(Debug, sqlx::FromRow)]
struct PersonalExamSessionRow {
    round_id: Uuid,
    round_name: String,
    academic_semester_id: Uuid,
    published_at: Option<DateTime<Utc>>,
    exam_date: NaiveDate,
    starts_at: NaiveTime,
    ends_at: NaiveTime,
    subject_name: String,
    assessment_category_name: String,
    classroom_name: String,
    room_name: String,
    building_name: Option<String>,
    seat_number: Option<String>,
}

impl PersonalExamSessionRow {
    fn into_session_view(self) -> PersonalExamSessionView {
        PersonalExamSessionView {
            exam_date: self.exam_date,
            starts_at: self.starts_at,
            ends_at: self.ends_at,
            subject_name: self.subject_name,
            assessment_category_name: self.assessment_category_name,
            classroom_name: self.classroom_name,
            room_name: self.room_name,
            building_name: self.building_name,
            seat_number: self.seat_number,
        }
    }
}

impl InvigilatorViewRow {
    fn into_view(self) -> InvigilatorView {
        InvigilatorView {
            staff_id: self.staff_id,
            display_name: self.display_name,
        }
    }
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

async fn lock_exam_invigilator_staff_conflict_scope(
    tx: &mut Transaction<'_, Postgres>,
    exam_day_id: Uuid,
    staff_ids: &[Uuid],
) -> Result<(), AppError> {
    if staff_ids.is_empty() {
        return Ok(());
    }

    for lock_key in exam_invigilator_staff_lock_keys(exam_day_id, staff_ids) {
        sqlx::query("SELECT pg_advisory_xact_lock($1)")
            .bind(lock_key)
            .execute(&mut **tx)
            .await?;
    }
    Ok(())
}

pub async fn get_invigilator_workspace(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<ExamInvigilatorWorkspace, AppError> {
    fetch_round(pool, round_id).await?;
    let assignments = fetch_invigilator_assignment_summaries(pool, round_id).await?;
    let assignment_ids: Vec<Uuid> = assignments.iter().map(|item| item.assignment_id).collect();
    let mut invigilators_by_assignment =
        fetch_invigilator_views_by_assignment_ids(pool, &assignment_ids).await?;
    let staff_workloads = fetch_invigilator_staff_workloads(pool, round_id).await?;

    Ok(ExamInvigilatorWorkspace {
        round_id,
        assignments: assignments
            .into_iter()
            .map(|row| ExamInvigilatorAssignmentSummary {
                assignment_id: row.assignment_id,
                exam_day_id: row.exam_day_id,
                classroom_id: row.classroom_id,
                classroom_name: row.classroom_name,
                room_id: row.room_id,
                room_name: row.room_name,
                session_minutes: row.session_minutes,
                invigilators: invigilators_by_assignment
                    .remove(&row.assignment_id)
                    .unwrap_or_default(),
            })
            .collect(),
        staff_workloads,
    })
}

pub async fn list_invigilator_staff_options(
    pool: &PgPool,
    round_id: Uuid,
    search: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<ExamInvigilatorStaffOption>, AppError> {
    fetch_round(pool, round_id).await?;
    let search_pattern = invigilator_staff_option_search_pattern(search);
    let limit = invigilator_staff_option_limit(limit);

    sqlx::query_as::<_, ExamInvigilatorStaffOption>(
        r#"
        SELECT user_account.id AS staff_id,
               COALESCE(
                   NULLIF(
                       concat_ws(
                           ' ',
                           NULLIF(
                               concat_ws('', NULLIF(TRIM(user_account.title), ''), NULLIF(TRIM(user_account.first_name), '')),
                               ''
                           ),
                           NULLIF(TRIM(user_account.last_name), '')
                       ),
                       ''
                   ),
                   user_account.id::TEXT
               ) AS display_name
        FROM users user_account
        WHERE user_account.user_type = 'staff'
          AND user_account.status = 'active'
          AND (
              $1::TEXT IS NULL
              OR user_account.first_name ILIKE $1
              OR user_account.last_name ILIKE $1
              OR concat_ws(
                    ' ',
                    NULLIF(
                        concat_ws('', NULLIF(TRIM(user_account.title), ''), NULLIF(TRIM(user_account.first_name), '')),
                        ''
                    ),
                    NULLIF(TRIM(user_account.last_name), '')
                 ) ILIKE $1
          )
        ORDER BY user_account.first_name, user_account.last_name, user_account.id
        LIMIT $2
        "#,
    )
    .bind(search_pattern)
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

pub async fn update_assignment_invigilators(
    pool: &PgPool,
    assignment_id: Uuid,
    request: UpdateExamInvigilatorsRequest,
    actor_user_id: Uuid,
) -> Result<DayRoomAssignmentView, AppError> {
    let invigilator_staff_ids =
        validate_unique_invigilator_staff_ids(request.invigilator_staff_ids)?;
    let mut tx = pool.begin().await?;
    let context = fetch_seat_assignment_context(&mut tx, assignment_id).await?;
    let exam_day_id: Uuid = sqlx::query_scalar(
        "SELECT exam_day_id FROM academic_exam_day_room_assignments WHERE id = $1",
    )
    .bind(assignment_id)
    .fetch_one(&mut *tx)
    .await?;

    lock_exam_invigilator_staff_conflict_scope(&mut tx, exam_day_id, &invigilator_staff_ids)
        .await?;
    validate_invigilator_time_conflicts(
        &mut tx,
        context.exam_round_id,
        assignment_id,
        &invigilator_staff_ids,
    )
    .await?;
    replace_assignment_invigilators_in_tx(
        &mut tx,
        context.exam_round_id,
        exam_day_id,
        assignment_id,
        &invigilator_staff_ids,
    )
    .await?;
    mark_round_draft_after_mutation(&mut tx, context.exam_round_id, Some(actor_user_id)).await?;
    tx.commit().await?;

    fetch_day_room_assignment_view(pool, assignment_id).await
}

pub async fn assign_invigilator_to_assignment(
    pool: &PgPool,
    assignment_id: Uuid,
    staff_id: Uuid,
    actor_user_id: Uuid,
) -> Result<ExamInvigilatorWorkspace, AppError> {
    let mut tx = pool.begin().await?;
    let context =
        fetch_invigilator_assignment_mutation_context_for_update(&mut tx, assignment_id).await?;
    ensure_exam_round_is_mutable(&context.round_status)?;

    let staff_ids = vec![staff_id];
    lock_exam_invigilator_staff_conflict_scope(&mut tx, context.exam_day_id, &staff_ids).await?;
    validate_active_staff_users(&mut tx, &staff_ids).await?;

    let removed_count = delete_staff_invigilator_from_other_day_assignments_in_tx(
        &mut tx,
        context.exam_day_id,
        context.assignment_id,
        staff_id,
    )
    .await?;
    let inserted_count = insert_staff_invigilator_if_missing_in_tx(
        &mut tx,
        context.exam_day_id,
        context.assignment_id,
        staff_id,
    )
    .await?;

    let round_id = context.exam_round_id;
    if removed_count > 0 || inserted_count > 0 {
        mark_round_draft_after_mutation(&mut tx, round_id, Some(actor_user_id)).await?;
    }
    tx.commit().await?;

    get_invigilator_workspace(pool, round_id).await
}

pub async fn remove_invigilator_from_assignment(
    pool: &PgPool,
    assignment_id: Uuid,
    staff_id: Uuid,
    actor_user_id: Uuid,
) -> Result<ExamInvigilatorWorkspace, AppError> {
    let mut tx = pool.begin().await?;
    let context =
        fetch_invigilator_assignment_mutation_context_for_update(&mut tx, assignment_id).await?;
    ensure_exam_round_is_mutable(&context.round_status)?;

    let deleted_count =
        delete_staff_invigilator_from_assignment_in_tx(&mut tx, context.assignment_id, staff_id)
            .await?;

    let round_id = context.exam_round_id;
    if deleted_count > 0 {
        mark_round_draft_after_mutation(&mut tx, round_id, Some(actor_user_id)).await?;
    }
    tx.commit().await?;

    get_invigilator_workspace(pool, round_id).await
}

async fn replace_assignment_invigilators_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    _round_id: Uuid,
    exam_day_id: Uuid,
    assignment_id: Uuid,
    invigilator_staff_ids: &[Uuid],
) -> Result<(), AppError> {
    validate_active_staff_users(tx, invigilator_staff_ids).await?;

    sqlx::query(
        r#"
        DELETE FROM academic_exam_day_invigilators
        WHERE day_room_assignment_id = $1
        "#,
    )
    .bind(assignment_id)
    .execute(&mut **tx)
    .await?;

    if invigilator_staff_ids.is_empty() {
        return Ok(());
    }

    sqlx::query(
        r#"
        INSERT INTO academic_exam_day_invigilators (
            exam_day_id,
            day_room_assignment_id,
            staff_id
        )
        SELECT $1, $2, staff_id
        FROM unnest($3::uuid[]) AS staff_id
        "#,
    )
    .bind(exam_day_id)
    .bind(assignment_id)
    .bind(invigilator_staff_ids)
    .execute(&mut **tx)
    .await
    .map_err(map_day_room_assignment_write_error)?;

    Ok(())
}

async fn delete_staff_invigilator_from_other_day_assignments_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    exam_day_id: Uuid,
    target_assignment_id: Uuid,
    staff_id: Uuid,
) -> Result<u64, AppError> {
    let result = sqlx::query(
        r#"
        DELETE FROM academic_exam_day_invigilators invigilator
        USING academic_exam_day_room_assignments assignment
        WHERE assignment.id = invigilator.day_room_assignment_id
          AND assignment.exam_day_id = invigilator.exam_day_id
          AND assignment.exam_day_id = $1
          AND invigilator.staff_id = $2
          AND invigilator.day_room_assignment_id <> $3
        "#,
    )
    .bind(exam_day_id)
    .bind(staff_id)
    .bind(target_assignment_id)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected())
}

async fn insert_staff_invigilator_if_missing_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    exam_day_id: Uuid,
    assignment_id: Uuid,
    staff_id: Uuid,
) -> Result<u64, AppError> {
    let result = sqlx::query(
        r#"
        INSERT INTO academic_exam_day_invigilators (
            exam_day_id,
            day_room_assignment_id,
            staff_id
        )
        VALUES ($1, $2, $3)
        ON CONFLICT (day_room_assignment_id, staff_id) DO NOTHING
        "#,
    )
    .bind(exam_day_id)
    .bind(assignment_id)
    .bind(staff_id)
    .execute(&mut **tx)
    .await
    .map_err(map_day_room_assignment_write_error)?;

    Ok(result.rows_affected())
}

async fn delete_staff_invigilator_from_assignment_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    assignment_id: Uuid,
    staff_id: Uuid,
) -> Result<u64, AppError> {
    let result = sqlx::query(
        r#"
        DELETE FROM academic_exam_day_invigilators
        WHERE day_room_assignment_id = $1
          AND staff_id = $2
        "#,
    )
    .bind(assignment_id)
    .bind(staff_id)
    .execute(&mut **tx)
    .await?;

    Ok(result.rows_affected())
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

pub async fn publish_round(
    pool: &PgPool,
    round_id: Uuid,
    actor_user_id: Uuid,
) -> Result<ExamRound, AppError> {
    let mut tx = pool.begin().await?;

    let _locked_round_id: Uuid = sqlx::query_scalar(
        r#"
        SELECT id
        FROM academic_exam_rounds
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(round_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam round not found".to_string()))?;

    let readiness = build_readiness(fetch_workspace_counts_in_tx(&mut tx, round_id).await?);
    if !readiness.can_publish {
        return Err(AppError::BadRequest(format!(
            "Exam round is not ready to publish: {}",
            readiness.blockers.join("; ")
        )));
    }

    let round = sqlx::query_as::<_, ExamRound>(
        r#"
        UPDATE academic_exam_rounds
        SET status = 'published',
            published_at = now(),
            published_by = $2,
            updated_by = $2,
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
    .bind(actor_user_id)
    .fetch_one(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(round)
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

async fn ensure_active_student_user(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    let user_row: Option<(String, String)> =
        sqlx::query_as("SELECT user_type, status FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;

    match user_row
        .as_ref()
        .map(|(user_type, status)| (user_type.as_str(), status.as_str()))
    {
        Some(("student", "active")) => Ok(()),
        Some(_) => Err(AppError::Forbidden(
            "Only active students can view personal exam schedules".to_string(),
        )),
        None => Err(AppError::AuthError("Please sign in".to_string())),
    }
}

async fn ensure_active_staff_user_for_exam_schedule(
    pool: &PgPool,
    user_id: Uuid,
) -> Result<(), AppError> {
    let user_row: Option<(String, String)> =
        sqlx::query_as("SELECT user_type, status FROM users WHERE id = $1")
            .bind(user_id)
            .fetch_optional(pool)
            .await?;

    match user_row
        .as_ref()
        .map(|(user_type, status)| (user_type.as_str(), status.as_str()))
    {
        Some(("staff", "active")) => Ok(()),
        Some(_) => Err(AppError::Forbidden(
            "Only active staff can view published exam schedules".to_string(),
        )),
        None => Err(AppError::AuthError("Please sign in".to_string())),
    }
}

async fn ensure_parent_user_for_exam_schedule(
    pool: &PgPool,
    parent_user_id: Uuid,
) -> Result<(), AppError> {
    let user_type: Option<String> = sqlx::query_scalar("SELECT user_type FROM users WHERE id = $1")
        .bind(parent_user_id)
        .fetch_optional(pool)
        .await?;

    match user_type.as_deref() {
        Some("parent") => Ok(()),
        Some(_) => Err(AppError::Forbidden("เฉพาะผู้ปกครองเท่านั้น".to_string())),
        None => Err(AppError::AuthError("กรุณาเข้าสู่ระบบ".to_string())),
    }
}

async fn ensure_parent_student_link_for_exam_schedule(
    pool: &PgPool,
    parent_user_id: Uuid,
    student_id: Uuid,
) -> Result<(), AppError> {
    let is_linked: bool = sqlx::query_scalar(
        r#"
        SELECT EXISTS(
            SELECT 1
            FROM student_parents
            JOIN users user_account ON user_account.id = student_parents.student_user_id
            WHERE student_parents.parent_user_id = $1
              AND student_parents.student_user_id = $2
              AND user_account.user_type = 'student'
              AND user_account.status = 'active'
        )
        "#,
    )
    .bind(parent_user_id)
    .bind(student_id)
    .fetch_one(pool)
    .await?;

    if !is_linked {
        return Err(AppError::Forbidden(
            "คุณไม่มีสิทธิ์เข้าถึงข้อมูลนักเรียนคนนี้".to_string(),
        ));
    }

    Ok(())
}

async fn list_published_exam_schedule_for_student(
    pool: &PgPool,
    student_id: Uuid,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<PersonalExamScheduleRound>, AppError> {
    let rows = sqlx::query_as::<_, PersonalExamSessionRow>(
        r#"
        SELECT round.id AS round_id,
               round.name AS round_name,
               round.academic_semester_id,
               round.published_at,
               day.exam_date,
               session.starts_at,
               session.ends_at,
               COALESCE(NULLIF(subject.name_th, ''), NULLIF(subject.name_en, ''), subject.code)
                   AS subject_name,
               category.name AS assessment_category_name,
               classroom.name AS classroom_name,
               room.name_th AS room_name,
               building.name_th AS building_name,
               seat.seat_number
        FROM student_class_enrollments enrollment
        JOIN users student_user
          ON student_user.id = enrollment.student_id
         AND student_user.user_type = 'student'
         AND student_user.status = 'active'
        JOIN academic_exam_schedule_items item
          ON item.classroom_id = enrollment.class_room_id
        JOIN academic_exam_rounds round
          ON round.id = item.exam_round_id
         AND round.academic_semester_id = item.academic_semester_id
        JOIN academic_exam_sessions session
          ON session.exam_schedule_item_id = item.id
         AND session.exam_round_id = item.exam_round_id
        JOIN academic_exam_days day
          ON day.id = session.exam_day_id
         AND day.exam_round_id = session.exam_round_id
        JOIN academic_assessment_categories category
          ON category.id = item.assessment_category_id
        JOIN subjects subject ON subject.id = item.subject_id
        JOIN class_rooms classroom ON classroom.id = item.classroom_id
        JOIN academic_exam_day_room_assignments assignment
          ON assignment.exam_day_id = session.exam_day_id
         AND assignment.classroom_id = item.classroom_id
        JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN buildings building ON building.id = room.building_id
        LEFT JOIN academic_exam_seat_assignments seat
          ON seat.day_room_assignment_id = assignment.id
         AND seat.student_id = enrollment.student_id
        WHERE enrollment.student_id = $1
          AND enrollment.status = 'active'
          AND round.status = 'published'
          AND ($2::uuid IS NULL OR round.academic_semester_id = $2)
        ORDER BY round.published_at DESC NULLS LAST,
                 round.name,
                 day.exam_date,
                 session.starts_at,
                 classroom.name,
                 subject.code,
                 category.display_order,
                 category.name,
                 session.id
        "#,
    )
    .bind(student_id)
    .bind(academic_semester_id)
    .fetch_all(pool)
    .await?;

    Ok(group_personal_exam_schedule_rows(rows))
}

async fn list_published_exam_schedule_for_staff(
    pool: &PgPool,
    academic_semester_id: Option<Uuid>,
) -> Result<Vec<PersonalExamScheduleRound>, AppError> {
    let rows = sqlx::query_as::<_, PersonalExamSessionRow>(
        r#"
        SELECT round.id AS round_id,
               round.name AS round_name,
               round.academic_semester_id,
               round.published_at,
               day.exam_date,
               session.starts_at,
               session.ends_at,
               COALESCE(NULLIF(subject.name_th, ''), NULLIF(subject.name_en, ''), subject.code)
                   AS subject_name,
               category.name AS assessment_category_name,
               classroom.name AS classroom_name,
               room.name_th AS room_name,
               building.name_th AS building_name,
               NULL::text AS seat_number
        FROM academic_exam_sessions session
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.exam_round_id = session.exam_round_id
        JOIN academic_exam_rounds round
          ON round.id = item.exam_round_id
         AND round.academic_semester_id = item.academic_semester_id
        JOIN academic_exam_days day
          ON day.id = session.exam_day_id
         AND day.exam_round_id = session.exam_round_id
        JOIN academic_assessment_categories category
          ON category.id = item.assessment_category_id
        JOIN subjects subject ON subject.id = item.subject_id
        JOIN class_rooms classroom ON classroom.id = item.classroom_id
        JOIN academic_exam_day_room_assignments assignment
          ON assignment.exam_day_id = session.exam_day_id
         AND assignment.classroom_id = item.classroom_id
        JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN buildings building ON building.id = room.building_id
        WHERE round.status = 'published'
          AND ($1::uuid IS NULL OR round.academic_semester_id = $1)
        ORDER BY round.published_at DESC NULLS LAST,
                 round.name,
                 day.exam_date,
                 session.starts_at,
                 classroom.name,
                 subject.code,
                 category.display_order,
                 category.name,
                 session.id
        "#,
    )
    .bind(academic_semester_id)
    .fetch_all(pool)
    .await?;

    Ok(group_personal_exam_schedule_rows(rows))
}

fn group_personal_exam_schedule_rows(
    rows: Vec<PersonalExamSessionRow>,
) -> Vec<PersonalExamScheduleRound> {
    let mut rounds = Vec::new();
    let mut round_indexes = HashMap::new();

    for row in rows {
        let round_id = row.round_id;
        let round_index = match round_indexes.get(&round_id) {
            Some(index) => *index,
            None => {
                let index = rounds.len();
                rounds.push(PersonalExamScheduleRound {
                    round_id,
                    round_name: row.round_name.clone(),
                    academic_semester_id: row.academic_semester_id,
                    published_at: row.published_at,
                    sessions: Vec::new(),
                });
                round_indexes.insert(round_id, index);
                index
            }
        };

        rounds[round_index].sessions.push(row.into_session_view());
    }

    rounds
}

fn validate_unique_invigilator_staff_ids(ids: Vec<Uuid>) -> Result<Vec<Uuid>, AppError> {
    let mut seen = HashSet::new();
    for id in &ids {
        if !seen.insert(*id) {
            return Err(AppError::BadRequest(
                "Duplicate invigilator staff ids are not allowed".to_string(),
            ));
        }
    }
    Ok(ids)
}

async fn validate_day_allows_grade_level(
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

fn grade_level_allowed_by_day_scope(grade_level_id: Uuid, scoped_grade_level_ids: &[Uuid]) -> bool {
    scoped_grade_level_ids.is_empty() || scoped_grade_level_ids.contains(&grade_level_id)
}

async fn validate_active_staff_users(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    staff_ids: &[Uuid],
) -> Result<(), AppError> {
    if staff_ids.is_empty() {
        return Ok(());
    }

    let invalid_count: i64 = sqlx::query_scalar(
        r#"
        SELECT COUNT(*)::BIGINT
        FROM unnest($1::uuid[]) AS requested(staff_id)
        LEFT JOIN users user_account
          ON user_account.id = requested.staff_id
         AND user_account.user_type = 'staff'
         AND user_account.status = 'active'
        WHERE user_account.id IS NULL
        "#,
    )
    .bind(staff_ids)
    .fetch_one(&mut **tx)
    .await?;

    if invalid_count > 0 {
        return Err(AppError::BadRequest(
            "Every invigilator must be an active staff user".to_string(),
        ));
    }
    Ok(())
}

async fn fetch_assignment_session_windows(
    tx: &mut Transaction<'_, Postgres>,
    assignment_id: Uuid,
    staff_ids: &[Uuid],
) -> Result<Vec<InvigilatorSessionWindow>, AppError> {
    if staff_ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query_as::<_, InvigilatorSessionWindowRow>(
        r#"
        SELECT assignment.id AS assignment_id,
               assignment.exam_day_id,
               staff.staff_id,
               '' AS staff_name,
               session.starts_at,
               session.ends_at
        FROM academic_exam_day_room_assignments assignment
        JOIN unnest($2::uuid[]) AS staff(staff_id) ON TRUE
        JOIN academic_exam_days day ON day.id = assignment.exam_day_id
        JOIN academic_exam_sessions session
          ON session.exam_day_id = assignment.exam_day_id
         AND session.exam_round_id = day.exam_round_id
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.classroom_id = assignment.classroom_id
        WHERE assignment.id = $1
        ORDER BY session.starts_at, staff.staff_id
        "#,
    )
    .bind(assignment_id)
    .bind(staff_ids)
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| InvigilatorSessionWindow {
            assignment_id: row.assignment_id,
            exam_day_id: row.exam_day_id,
            staff_id: row.staff_id,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
        })
        .collect())
}

async fn fetch_existing_invigilator_session_windows(
    tx: &mut Transaction<'_, Postgres>,
    round_id: Uuid,
    staff_ids: &[Uuid],
) -> Result<Vec<InvigilatorSessionWindow>, AppError> {
    if staff_ids.is_empty() {
        return Ok(Vec::new());
    }

    let rows = sqlx::query_as::<_, InvigilatorSessionWindowRow>(
        r#"
        SELECT assignment.id AS assignment_id,
               assignment.exam_day_id,
               invigilator.staff_id,
               '' AS staff_name,
               session.starts_at,
               session.ends_at
        FROM academic_exam_day_invigilators invigilator
        JOIN academic_exam_day_room_assignments assignment
          ON assignment.id = invigilator.day_room_assignment_id
        JOIN academic_exam_days day ON day.id = assignment.exam_day_id
        JOIN academic_exam_sessions session
          ON session.exam_day_id = assignment.exam_day_id
         AND session.exam_round_id = day.exam_round_id
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.classroom_id = assignment.classroom_id
        WHERE day.exam_round_id = $1
          AND invigilator.staff_id = ANY($2)
        ORDER BY assignment.exam_day_id, session.starts_at, invigilator.staff_id
        "#,
    )
    .bind(round_id)
    .bind(staff_ids)
    .fetch_all(&mut **tx)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| InvigilatorSessionWindow {
            assignment_id: row.assignment_id,
            exam_day_id: row.exam_day_id,
            staff_id: row.staff_id,
            starts_at: row.starts_at,
            ends_at: row.ends_at,
        })
        .collect())
}

async fn validate_invigilator_time_conflicts(
    tx: &mut Transaction<'_, Postgres>,
    round_id: Uuid,
    assignment_id: Uuid,
    staff_ids: &[Uuid],
) -> Result<(), AppError> {
    if staff_ids.is_empty() {
        return Ok(());
    }

    let candidate_windows = fetch_assignment_session_windows(tx, assignment_id, staff_ids).await?;
    if candidate_windows.is_empty() {
        return Ok(());
    }

    let existing_windows =
        fetch_existing_invigilator_session_windows(tx, round_id, staff_ids).await?;
    if has_invigilator_time_conflict(assignment_id, &candidate_windows, &existing_windows) {
        return Err(AppError::BadRequest(
            "Invigilator has an overlapping exam supervision assignment".to_string(),
        ));
    }

    Ok(())
}

fn build_invigilator_candidate_session_windows(
    assignment_id: Uuid,
    exam_day_id: Uuid,
    starts_at: NaiveTime,
    ends_at: NaiveTime,
    staff_ids: &[Uuid],
) -> Vec<InvigilatorSessionWindow> {
    staff_ids
        .iter()
        .map(|staff_id| InvigilatorSessionWindow {
            assignment_id,
            exam_day_id,
            staff_id: *staff_id,
            starts_at,
            ends_at,
        })
        .collect()
}

async fn fetch_invigilator_staff_ids_for_assignment(
    tx: &mut Transaction<'_, Postgres>,
    assignment_id: Uuid,
) -> Result<Vec<Uuid>, AppError> {
    sqlx::query_scalar(
        r#"
        SELECT staff_id
        FROM academic_exam_day_invigilators
        WHERE day_room_assignment_id = $1
        ORDER BY staff_id
        "#,
    )
    .bind(assignment_id)
    .fetch_all(&mut **tx)
    .await
    .map_err(AppError::from)
}

async fn validate_invigilator_candidate_session_conflicts(
    tx: &mut Transaction<'_, Postgres>,
    round_id: Uuid,
    assignment_id: Uuid,
    exam_day_id: Uuid,
    starts_at: NaiveTime,
    ends_at: NaiveTime,
    staff_ids: &[Uuid],
) -> Result<(), AppError> {
    if staff_ids.is_empty() {
        return Ok(());
    }

    let candidate_windows = build_invigilator_candidate_session_windows(
        assignment_id,
        exam_day_id,
        starts_at,
        ends_at,
        staff_ids,
    );
    let existing_windows =
        fetch_existing_invigilator_session_windows(tx, round_id, staff_ids).await?;
    if has_invigilator_time_conflict(assignment_id, &candidate_windows, &existing_windows) {
        return Err(AppError::BadRequest(
            "Invigilator has an overlapping exam supervision assignment".to_string(),
        ));
    }

    Ok(())
}

async fn fetch_invigilator_assignment_summaries(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<InvigilatorAssignmentSummaryRow>, AppError> {
    sqlx::query_as::<_, InvigilatorAssignmentSummaryRow>(
        r#"
        SELECT assignment.id AS assignment_id,
               day.id AS exam_day_id,
               assignment.classroom_id,
               classroom.name AS classroom_name,
               assignment.room_id,
               room.name_th AS room_name,
               COALESCE(SUM(EXTRACT(EPOCH FROM (session.ends_at - session.starts_at)) / 60), 0)::INT
                   AS session_minutes
        FROM academic_exam_day_room_assignments assignment
        JOIN academic_exam_days day ON day.id = assignment.exam_day_id
        JOIN class_rooms classroom ON classroom.id = assignment.classroom_id
        JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN academic_exam_schedule_items item
          ON item.exam_round_id = day.exam_round_id
         AND item.classroom_id = assignment.classroom_id
        LEFT JOIN academic_exam_sessions session
          ON session.exam_schedule_item_id = item.id
         AND session.exam_day_id = assignment.exam_day_id
         AND session.exam_round_id = day.exam_round_id
        WHERE day.exam_round_id = $1
        GROUP BY assignment.id, day.id, assignment.classroom_id, classroom.name, assignment.room_id, room.name_th
        ORDER BY day.exam_date, day.start_time, day.id, classroom.name, room.name_th, assignment.id
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .map_err(AppError::from)
}

async fn fetch_invigilator_staff_workloads(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<ExamInvigilatorStaffWorkload>, AppError> {
    let rows = sqlx::query_as::<_, InvigilatorSessionWindowRow>(
        r#"
        SELECT assignment.id AS assignment_id,
               assignment.exam_day_id,
               invigilator.staff_id,
               concat_ws(
                   ' ',
                   NULLIF(
                       concat_ws('', NULLIF(TRIM(user_account.title), ''), NULLIF(TRIM(user_account.first_name), '')),
                       ''
                   ),
                   NULLIF(TRIM(user_account.last_name), '')
               )
                   AS staff_name,
               session.starts_at,
               session.ends_at
        FROM academic_exam_day_invigilators invigilator
        JOIN academic_exam_day_room_assignments assignment
          ON assignment.id = invigilator.day_room_assignment_id
        JOIN academic_exam_days day ON day.id = assignment.exam_day_id
        JOIN users user_account ON user_account.id = invigilator.staff_id
        JOIN academic_exam_sessions session
          ON session.exam_day_id = assignment.exam_day_id
         AND session.exam_round_id = day.exam_round_id
        JOIN academic_exam_schedule_items item
          ON item.id = session.exam_schedule_item_id
         AND item.classroom_id = assignment.classroom_id
        WHERE day.exam_round_id = $1
        ORDER BY staff_name, day.exam_date, day.start_time, day.id, session.starts_at, assignment.id
        "#,
    )
    .bind(round_id)
    .fetch_all(pool)
    .await?;

    Ok(build_invigilator_staff_workloads(rows))
}

#[derive(Debug, Default)]
struct StaffWorkloadAccumulator {
    staff_name: String,
    day_minutes: BTreeMap<Uuid, i32>,
    day_assignments: BTreeMap<Uuid, BTreeSet<Uuid>>,
    assignments: BTreeSet<Uuid>,
}

fn build_invigilator_staff_workloads(
    rows: Vec<InvigilatorSessionWindowRow>,
) -> Vec<ExamInvigilatorStaffWorkload> {
    let mut by_staff: BTreeMap<Uuid, StaffWorkloadAccumulator> = BTreeMap::new();

    for row in rows {
        let minutes = minutes_between_times(row.starts_at, row.ends_at);
        let accumulator =
            by_staff
                .entry(row.staff_id)
                .or_insert_with(|| StaffWorkloadAccumulator {
                    staff_name: row.staff_name.clone(),
                    ..Default::default()
                });

        *accumulator.day_minutes.entry(row.exam_day_id).or_insert(0) += minutes;
        accumulator
            .day_assignments
            .entry(row.exam_day_id)
            .or_default()
            .insert(row.assignment_id);
        accumulator.assignments.insert(row.assignment_id);
    }

    by_staff
        .into_iter()
        .map(|(staff_id, accumulator)| {
            let days = accumulator
                .day_minutes
                .iter()
                .map(|(exam_day_id, minutes)| ExamInvigilatorDayWorkload {
                    exam_day_id: *exam_day_id,
                    minutes: *minutes,
                    assignment_count: accumulator
                        .day_assignments
                        .get(exam_day_id)
                        .map(|assignment_ids| assignment_ids.len() as i32)
                        .unwrap_or(0),
                })
                .collect::<Vec<_>>();

            ExamInvigilatorStaffWorkload {
                staff_id,
                staff_name: accumulator.staff_name,
                total_minutes: days.iter().map(|day| day.minutes).sum(),
                assigned_day_count: days.len() as i32,
                assignment_count: accumulator.assignments.len() as i32,
                days,
            }
        })
        .collect()
}

async fn fetch_invigilator_views_by_assignment_ids(
    pool: &PgPool,
    assignment_ids: &[Uuid],
) -> Result<HashMap<Uuid, Vec<InvigilatorView>>, AppError> {
    let assignment_ids = unique_uuids(assignment_ids.to_vec());
    if assignment_ids.is_empty() {
        return Ok(HashMap::new());
    }

    let rows = sqlx::query_as::<_, InvigilatorViewRow>(
        r#"
        SELECT invigilator.day_room_assignment_id,
               invigilator.staff_id,
               concat_ws(
                   ' ',
                   NULLIF(
                       concat_ws('', NULLIF(TRIM(user_account.title), ''), NULLIF(TRIM(user_account.first_name), '')),
                       ''
                   ),
                   NULLIF(TRIM(user_account.last_name), '')
               )
                   AS display_name
        FROM academic_exam_day_invigilators invigilator
        JOIN users user_account ON user_account.id = invigilator.staff_id
        WHERE invigilator.day_room_assignment_id = ANY($1)
        ORDER BY invigilator.day_room_assignment_id,
                 user_account.first_name,
                 user_account.last_name,
                 invigilator.staff_id
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
            .push(row.into_view());
    }
    Ok(grouped)
}

async fn fetch_invigilator_assignment_mutation_context_for_update(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    assignment_id: Uuid,
) -> Result<InvigilatorAssignmentMutationContext, AppError> {
    sqlx::query_as::<_, InvigilatorAssignmentMutationContext>(
        r#"
        SELECT assignment.id AS assignment_id,
               assignment.exam_day_id,
               exam_day.exam_round_id,
               exam_round.status AS round_status
        FROM academic_exam_day_room_assignments assignment
        JOIN academic_exam_days exam_day ON exam_day.id = assignment.exam_day_id
        JOIN academic_exam_rounds exam_round ON exam_round.id = exam_day.exam_round_id
        WHERE assignment.id = $1
        FOR UPDATE OF assignment, exam_round
        "#,
    )
    .bind(assignment_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam room assignment not found".to_string()))
}

fn invigilator_staff_option_limit(limit: Option<i64>) -> i64 {
    limit
        .unwrap_or(INVIGILATOR_STAFF_OPTION_DEFAULT_LIMIT)
        .clamp(1, INVIGILATOR_STAFF_OPTION_MAX_LIMIT)
}

fn invigilator_staff_option_search_pattern(search: Option<String>) -> Option<String> {
    let trimmed = search?.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(format!("%{trimmed}%"))
    }
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
                   concat_ws(
                       ' ',
                       NULLIF(
                           concat_ws('', NULLIF(TRIM(user_account.title), ''), NULLIF(TRIM(user_account.first_name), '')),
                           ''
                       ),
                       NULLIF(TRIM(user_account.last_name), '')
                   ),
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn t(value: &str) -> NaiveTime {
        NaiveTime::parse_from_str(value, "%H:%M").unwrap()
    }

    #[test]
    fn room_assignment_payload_without_invigilators_preserves_existing_staff() {
        let request = serde_json::json!({
            "classroomId": Uuid::from_u128(1),
            "roomId": Uuid::from_u128(2),
            "capacityOverride": null
        });

        let parsed: UpsertDayRoomAssignmentRequest = serde_json::from_value(request).unwrap();

        assert_eq!(parsed.invigilator_staff_ids, None);
    }

    #[test]
    fn room_assignment_payload_with_invigilators_remains_backwards_compatible() {
        let staff_id = Uuid::from_u128(3);
        let request = serde_json::json!({
            "classroomId": Uuid::from_u128(1),
            "roomId": Uuid::from_u128(2),
            "capacityOverride": null,
            "invigilatorStaffIds": [staff_id]
        });

        let parsed: UpsertDayRoomAssignmentRequest = serde_json::from_value(request).unwrap();

        assert_eq!(parsed.invigilator_staff_ids, Some(vec![staff_id]));
    }

    #[test]
    fn invigilator_staff_option_limit_uses_bounded_default() {
        assert_eq!(invigilator_staff_option_limit(None), 40);
        assert_eq!(invigilator_staff_option_limit(Some(0)), 1);
        assert_eq!(invigilator_staff_option_limit(Some(250)), 100);
        assert_eq!(invigilator_staff_option_limit(Some(24)), 24);
    }

    #[test]
    fn invigilator_staff_option_search_pattern_trims_empty_values() {
        assert_eq!(invigilator_staff_option_search_pattern(None), None);
        assert_eq!(
            invigilator_staff_option_search_pattern(Some("   ".to_string())),
            None
        );
        assert_eq!(
            invigilator_staff_option_search_pattern(Some("  Kru A  ".to_string())),
            Some("%Kru A%".to_string())
        );
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
    fn detects_classroom_time_conflict() {
        let candidate = CandidateSession {
            session_id: None,
            classroom_id: Uuid::nil(),
            exam_day_id: Uuid::nil(),
            starts_at: t("09:00"),
            ends_at: t("10:00"),
        };
        let existing = vec![CandidateSession {
            session_id: Some(Uuid::max()),
            classroom_id: Uuid::nil(),
            exam_day_id: Uuid::nil(),
            starts_at: t("09:30"),
            ends_at: t("10:30"),
        }];
        assert!(has_same_classroom_conflict(&candidate, &existing));
    }

    #[test]
    fn invigilator_workload_sums_session_minutes_without_gaps() {
        let assignment_id = Uuid::from_u128(1);
        let staff_id = Uuid::from_u128(2);
        let windows = vec![
            InvigilatorSessionWindow {
                assignment_id,
                exam_day_id: Uuid::from_u128(10),
                staff_id,
                starts_at: t("08:30"),
                ends_at: t("09:30"),
            },
            InvigilatorSessionWindow {
                assignment_id,
                exam_day_id: Uuid::from_u128(10),
                staff_id,
                starts_at: t("10:00"),
                ends_at: t("11:30"),
            },
        ];

        let minutes = invigilator_workload_minutes(&windows);

        assert_eq!(minutes, 150);
    }

    #[test]
    fn invigilator_staff_workloads_group_by_staff_and_day() {
        let staff_id = Uuid::from_u128(7);
        let exam_day_id = Uuid::from_u128(10);
        let rows = vec![
            InvigilatorSessionWindowRow {
                assignment_id: Uuid::from_u128(1),
                exam_day_id,
                staff_id,
                staff_name: "Teacher One".to_string(),
                starts_at: t("08:30"),
                ends_at: t("09:30"),
            },
            InvigilatorSessionWindowRow {
                assignment_id: Uuid::from_u128(2),
                exam_day_id,
                staff_id,
                staff_name: "Teacher One".to_string(),
                starts_at: t("10:00"),
                ends_at: t("11:30"),
            },
        ];

        let workloads = build_invigilator_staff_workloads(rows);

        assert_eq!(workloads.len(), 1);
        assert_eq!(workloads[0].staff_id, staff_id);
        assert_eq!(workloads[0].staff_name, "Teacher One");
        assert_eq!(workloads[0].total_minutes, 150);
        assert_eq!(workloads[0].assigned_day_count, 1);
        assert_eq!(workloads[0].assignment_count, 2);
        assert_eq!(workloads[0].days.len(), 1);
        assert_eq!(workloads[0].days[0].exam_day_id, exam_day_id);
        assert_eq!(workloads[0].days[0].minutes, 150);
        assert_eq!(workloads[0].days[0].assignment_count, 2);
    }

    #[test]
    fn invigilator_conflict_rejects_overlapping_live_session_ranges() {
        let staff_id = Uuid::from_u128(7);
        let candidate = vec![InvigilatorSessionWindow {
            assignment_id: Uuid::from_u128(1),
            exam_day_id: Uuid::from_u128(10),
            staff_id,
            starts_at: t("08:30"),
            ends_at: t("09:30"),
        }];
        let existing = vec![InvigilatorSessionWindow {
            assignment_id: Uuid::from_u128(2),
            exam_day_id: Uuid::from_u128(10),
            staff_id,
            starts_at: t("09:00"),
            ends_at: t("10:00"),
        }];

        assert!(has_invigilator_time_conflict(
            Uuid::from_u128(1),
            &candidate,
            &existing
        ));
    }

    #[test]
    fn invigilator_conflict_allows_non_overlapping_same_day_assignments() {
        let staff_id = Uuid::from_u128(7);
        let candidate = vec![InvigilatorSessionWindow {
            assignment_id: Uuid::from_u128(1),
            exam_day_id: Uuid::from_u128(10),
            staff_id,
            starts_at: t("08:30"),
            ends_at: t("09:30"),
        }];
        let existing = vec![InvigilatorSessionWindow {
            assignment_id: Uuid::from_u128(2),
            exam_day_id: Uuid::from_u128(10),
            staff_id,
            starts_at: t("09:30"),
            ends_at: t("10:30"),
        }];

        assert!(!has_invigilator_time_conflict(
            Uuid::from_u128(1),
            &candidate,
            &existing
        ));
    }

    #[test]
    fn exam_invigilator_staff_lock_keys_are_sorted_deduped_and_stable() {
        let exam_day_id = Uuid::parse_str("01020304-0506-0708-090a-0b0c0d0e0f10").unwrap();
        let staff_a = Uuid::parse_str("11121314-1516-1718-191a-1b1c1d1e1f20").unwrap();
        let staff_b = Uuid::parse_str("21222324-2526-2728-292a-2b2c2d2e2f30").unwrap();

        let keys = exam_invigilator_staff_lock_keys(exam_day_id, &[staff_b, staff_a, staff_a]);

        assert_eq!(
            keys,
            exam_invigilator_staff_lock_keys(exam_day_id, &[staff_a, staff_b])
        );
        assert_eq!(keys.len(), 2);
        assert!(keys[0] < keys[1]);
    }

    #[test]
    fn assign_invigilator_to_assignment_uses_day_level_move_semantics() {
        let source = include_str!("exam_schedule_service.rs");
        let start = source
            .find("pub async fn assign_invigilator_to_assignment")
            .expect("assign service should exist");
        let body = &source[start
            ..source[start..]
                .find("pub async fn remove_invigilator_from_assignment")
                .map(|index| start + index)
                .unwrap_or(source.len())];

        let lock_position = body
            .find("lock_exam_invigilator_staff_conflict_scope")
            .expect("assign service should lock staff/day scope");
        let validate_position = body
            .find("validate_active_staff_users")
            .expect("assign service should validate active staff");
        let delete_position = body
            .find("delete_staff_invigilator_from_other_day_assignments_in_tx")
            .expect("assign service should remove staff from other rooms on the same day");
        let insert_position = body
            .find("insert_staff_invigilator_if_missing_in_tx")
            .expect("assign service should insert target staff");

        assert!(lock_position < validate_position);
        assert!(validate_position < delete_position);
        assert!(delete_position < insert_position);
        assert!(body.contains("ensure_exam_round_is_mutable"));
        assert!(body.contains("get_invigilator_workspace(pool, round_id)"));
    }

    #[test]
    fn remove_invigilator_from_assignment_only_deletes_target_assignment() {
        let source = include_str!("exam_schedule_service.rs");
        let start = source
            .find("pub async fn remove_invigilator_from_assignment")
            .expect("remove service should exist");
        let body = &source[start
            ..source[start..]
                .find("async fn replace_assignment_invigilators_in_tx")
                .map(|index| start + index)
                .unwrap_or(source.len())];

        assert!(body.contains("delete_staff_invigilator_from_assignment_in_tx"));
        assert!(!body.contains("delete_staff_invigilator_from_other_day_assignments_in_tx"));
        assert!(body.contains("ensure_exam_round_is_mutable"));
        assert!(body.contains("get_invigilator_workspace(pool, round_id)"));
    }

    #[test]
    fn exam_round_mutation_guard_rejects_published_rounds() {
        assert!(ensure_exam_round_is_mutable("draft").is_ok());
        assert!(ensure_exam_round_is_mutable("published").is_err());
    }

    #[test]
    fn academic_routes_expose_staff_level_invigilator_actions() {
        let source = include_str!("../../academic.rs");

        assert!(source
            .contains("/exam-schedules/room-assignments/{assignment_id}/invigilators/{staff_id}"));
        assert!(source.contains("assign_assignment_invigilator"));
        assert!(source.contains("remove_assignment_invigilator"));
    }

    #[test]
    fn exam_schedule_handler_uses_staff_level_invigilator_services() {
        let source = include_str!("../handlers/exam_schedule.rs");

        assert!(source.contains("pub async fn assign_assignment_invigilator"));
        assert!(source.contains("pub async fn remove_assignment_invigilator"));
        assert!(source.contains("exam_schedule_service::assign_invigilator_to_assignment"));
        assert!(source.contains("exam_schedule_service::remove_invigilator_from_assignment"));
        assert!(source.contains("Path((assignment_id, staff_id)): Path<(Uuid, Uuid)>"));
    }

    #[test]
    fn exam_day_update_preserves_day_identity_and_child_assignments() {
        let source = include_str!("exam_schedule_service/rounds_and_days.rs");
        let update_start = source.find("pub async fn update_exam_day").unwrap();
        let update_tail = &source[update_start..];
        let update_end = update_tail.find("pub async fn delete_exam_day").unwrap();
        let update_body = &update_tail[..update_end];

        assert!(update_body.contains("UPDATE academic_exam_days"));
        assert!(update_body.contains("WHERE id = $1"));
        assert!(update_body.contains("replace_exam_day_configuration"));
        assert!(update_body.contains("mark_round_draft_after_mutation"));
        assert!(!update_body.contains("DELETE FROM academic_exam_days"));
        assert!(!update_body.contains("academic_exam_sessions"));
        assert!(!update_body.contains("academic_exam_day_room_assignments"));
        assert!(!update_body.contains("academic_exam_day_invigilators"));
        assert!(!update_body.contains("academic_exam_seat_assignments"));
    }

    #[test]
    fn exam_day_update_maps_occupied_dates_to_actionable_error() {
        let source = include_str!("exam_schedule_service/rounds_and_days.rs");

        assert!(source.contains("map_err(map_exam_day_write_error)"));
        assert!(source.contains("กรุณาย้ายวันนั้นไปวันที่ว่างก่อน"));
    }

    #[test]
    fn update_assignment_invigilators_locks_staff_scope_before_conflict_validation() {
        let source = include_str!("exam_schedule_service.rs");
        let update_start = source
            .find("pub async fn update_assignment_invigilators")
            .unwrap();
        let update_body = &source[update_start..];
        let lock_position = update_body
            .find("lock_exam_invigilator_staff_conflict_scope")
            .unwrap();
        let validation_position = update_body
            .find("validate_invigilator_time_conflicts")
            .unwrap();

        assert!(lock_position < validation_position);
    }

    #[test]
    fn upsert_day_room_assignment_locks_optional_invigilator_scope_before_conflict_validation() {
        let source = include_str!("exam_schedule_service/room_assignments.rs");
        let upsert_start = source
            .find("pub async fn upsert_day_room_assignment")
            .unwrap();
        let upsert_tail = &source[upsert_start..];
        let update_start = upsert_tail
            .find("pub async fn generate_seats_for_assignment")
            .unwrap();
        let upsert_body = &upsert_tail[..update_start];
        let lock_position = upsert_body
            .find("lock_exam_invigilator_staff_conflict_scope")
            .unwrap();
        let validation_position = upsert_body
            .find("validate_invigilator_time_conflicts")
            .unwrap();

        assert!(lock_position < validation_position);
    }

    #[test]
    fn place_exam_session_locks_and_validates_invigilators_before_insert() {
        let source = include_str!("exam_schedule_service.rs");
        let placement_start = source.find("pub async fn place_exam_session").unwrap();
        let placement_body = &source[placement_start..];
        let lock_position = placement_body
            .find("lock_exam_invigilator_staff_conflict_scope")
            .unwrap();
        let validation_position = placement_body
            .find("validate_invigilator_candidate_session_conflicts")
            .unwrap();
        let insert_position = placement_body
            .find("INSERT INTO academic_exam_sessions")
            .unwrap();

        assert!(lock_position < validation_position);
        assert!(validation_position < insert_position);
    }

    #[test]
    fn builds_invigilator_candidate_session_windows_for_each_staff_member() {
        let assignment_id = Uuid::from_u128(1);
        let exam_day_id = Uuid::from_u128(2);
        let staff_a = Uuid::from_u128(3);
        let staff_b = Uuid::from_u128(4);

        let windows = build_invigilator_candidate_session_windows(
            assignment_id,
            exam_day_id,
            t("08:30"),
            t("09:30"),
            &[staff_a, staff_b],
        );

        assert_eq!(
            windows,
            vec![
                InvigilatorSessionWindow {
                    assignment_id,
                    exam_day_id,
                    staff_id: staff_a,
                    starts_at: t("08:30"),
                    ends_at: t("09:30"),
                },
                InvigilatorSessionWindow {
                    assignment_id,
                    exam_day_id,
                    staff_id: staff_b,
                    starts_at: t("08:30"),
                    ends_at: t("09:30"),
                },
            ]
        );
    }

    #[test]
    fn get_invigilator_workspace_checks_round_before_assignment_queries() {
        let source = include_str!("exam_schedule_service.rs");
        let workspace_start = source
            .find("pub async fn get_invigilator_workspace")
            .unwrap();
        let workspace_body = &source[workspace_start..];
        let round_position = workspace_body
            .find("fetch_round(pool, round_id).await?")
            .unwrap();
        let assignments_position = workspace_body
            .find("fetch_invigilator_assignment_summaries")
            .unwrap();

        assert!(round_position < assignments_position);
    }

    #[test]
    fn import_exam_items_filters_source_categories_by_round_kind() {
        let source = include_str!("exam_schedule_service/workspace.rs");
        let import_start = source.find("pub async fn import_exam_items").unwrap();
        let import_tail = &source[import_start..];
        let next_function_start = import_tail
            .find("pub async fn clear_mismatched_exam_items")
            .unwrap();
        let import_body = &import_tail[..next_function_start];

        assert!(import_body.contains("exam_kind"));
        assert_eq!(
            import_body.matches("c.code = rc.exam_kind").count(),
            3,
            "existing, missing-duration, and insert source queries must filter by round kind"
        );
    }

    #[test]
    fn clear_mismatched_exam_items_deletes_only_items_outside_round_kind() {
        let source = include_str!("exam_schedule_service/workspace.rs");
        let clear_start = source
            .find("pub async fn clear_mismatched_exam_items")
            .unwrap();
        let clear_tail = &source[clear_start..];
        let next_function_start = clear_tail
            .find("pub(super) async fn fetch_workspace_counts_in_tx")
            .unwrap();
        let clear_body = &clear_tail[..next_function_start];

        assert!(clear_body.contains("SELECT status"));
        assert!(clear_body.contains("FOR UPDATE"));
        assert!(clear_body.contains("DELETE FROM academic_exam_schedule_items"));
        assert!(clear_body.contains("USING academic_assessment_categories c"));
        assert!(clear_body.contains("round_context rc"));
        assert!(clear_body.contains("item.assessment_category_id = c.id"));
        assert!(clear_body.contains("c.code IS DISTINCT FROM rc.exam_kind"));
        assert!(clear_body.contains("mark_round_draft_after_mutation"));
    }

    #[test]
    fn publish_round_locks_round_before_readiness_check() {
        let source = include_str!("exam_schedule_service.rs");
        let publish_start = source.find("pub async fn publish_round").unwrap();
        let publish_body = &source[publish_start..];
        let tx_position = publish_body
            .find("let mut tx = pool.begin().await?")
            .unwrap();
        let lock_position = publish_body.find("FOR UPDATE").unwrap();
        let readiness_position = publish_body.find("fetch_workspace_counts_in_tx").unwrap();
        let update_position = publish_body.find("UPDATE academic_exam_rounds").unwrap();

        assert!(tx_position < lock_position);
        assert!(lock_position < readiness_position);
        assert!(readiness_position < update_position);
    }

    #[test]
    fn placement_locks_conflict_scope_before_conflict_queries() {
        let source = include_str!("exam_schedule_service.rs");
        let placement_start = source.find("pub async fn place_exam_session").unwrap();
        let placement_body = &source[placement_start..];
        let lock_position = placement_body
            .find("lock_exam_session_conflict_scope")
            .unwrap();
        let classroom_conflict_query_position = placement_body
            .find("fetch_candidate_sessions_for_day")
            .unwrap();
        let room_conflict_query_position = placement_body
            .find("fetch_candidate_room_sessions_for_day")
            .unwrap();

        assert!(lock_position < classroom_conflict_query_position);
        assert!(lock_position < room_conflict_query_position);
    }

    #[test]
    fn exam_session_conflict_lock_keys_are_sorted_and_scoped() {
        let exam_day_id = Uuid::parse_str("01020304-0506-0708-090a-0b0c0d0e0f10").unwrap();
        let classroom_id = Uuid::parse_str("11121314-1516-1718-191a-1b1c1d1e1f20").unwrap();
        let room_id = Uuid::parse_str("21222324-2526-2728-292a-2b2c2d2e2f30").unwrap();

        let keys = exam_session_conflict_lock_keys(exam_day_id, classroom_id, room_id);

        assert_eq!(
            keys,
            exam_session_conflict_lock_keys(exam_day_id, classroom_id, room_id)
        );
        assert!(keys[0] < keys[1]);
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
    fn accepts_placement_start_time_on_5_minute_slot() {
        let outcome = validate_session_window(t("08:35"), 60, t("08:30"), t("16:00"), &[]);

        assert!(outcome.is_ok());
    }

    #[test]
    fn rejects_placement_start_time_outside_5_minute_slot() {
        let outcome = validate_session_window(t("08:37"), 60, t("08:30"), t("16:00"), &[]);

        assert!(outcome.is_err());
    }

    #[test]
    fn empty_day_grade_scope_allows_any_grade_level() {
        assert!(grade_level_allowed_by_day_scope(Uuid::nil(), &[]));
    }

    #[test]
    fn explicit_day_grade_scope_rejects_removed_grade_level() {
        assert!(!grade_level_allowed_by_day_scope(
            Uuid::from_u128(1),
            &[Uuid::from_u128(2)]
        ));
    }

    #[test]
    fn readiness_sql_rechecks_sessions_after_day_window_changes() {
        assert!(WORKSPACE_COUNTS_SQL.contains("invalid_session_count"));
        assert!(WORKSPACE_COUNTS_SQL.contains("session.starts_at < day.start_time"));
        assert!(WORKSPACE_COUNTS_SQL.contains("session.ends_at > day.end_time"));
    }

    #[test]
    fn readiness_sql_uses_same_five_minute_slot_as_placement_validation() {
        assert!(WORKSPACE_COUNTS_SQL.contains("% 300"));
        assert!(!WORKSPACE_COUNTS_SQL.contains("% 900"));
    }

    #[test]
    fn readiness_sql_rechecks_sessions_after_blocked_window_changes() {
        assert!(WORKSPACE_COUNTS_SQL.contains("academic_exam_day_blocked_windows blocked"));
        assert!(WORKSPACE_COUNTS_SQL.contains("session.starts_at < blocked.end_time"));
        assert!(WORKSPACE_COUNTS_SQL.contains("blocked.start_time < session.ends_at"));
    }

    #[test]
    fn readiness_sql_rechecks_sessions_after_grade_scope_changes() {
        assert!(WORKSPACE_COUNTS_SQL.contains("academic_exam_day_grade_levels scope"));
        assert!(WORKSPACE_COUNTS_SQL.contains("scope.grade_level_id = item.grade_level_id"));
    }

    #[test]
    fn readiness_sql_requires_seats_for_every_active_student() {
        assert!(WORKSPACE_COUNTS_SQL.contains("missing_seat_student_count"));
        assert!(WORKSPACE_COUNTS_SQL.contains("student_class_enrollments enrollment"));
        assert!(WORKSPACE_COUNTS_SQL.contains("seat.student_id IS NULL"));
    }

    #[test]
    fn readiness_sql_counts_invigilator_live_range_conflicts() {
        assert!(WORKSPACE_COUNTS_SQL.contains("invigilator_conflict_count"));
        assert!(WORKSPACE_COUNTS_SQL.contains("academic_exam_day_invigilators"));
        assert!(WORKSPACE_COUNTS_SQL.contains("left_session.starts_at < right_session.ends_at"));
        assert!(WORKSPACE_COUNTS_SQL.contains("right_session.starts_at < left_session.ends_at"));
    }

    #[test]
    fn day_staff_unique_error_mapping_is_removed_after_live_range_migration() {
        let source = include_str!("exam_schedule_service/room_assignments.rs");
        let mapping_start = source
            .find("fn map_day_room_assignment_write_error")
            .unwrap();
        let mapping_body = &source[mapping_start..];

        assert!(!mapping_body.contains("exam_day_id_staff_id"));
    }

    #[test]
    fn readiness_requires_days_items_rooms_and_sessions() {
        let readiness = build_readiness(WorkspaceCounts {
            day_count: 0,
            item_count: 4,
            unscheduled_count: 4,
            missing_room_assignment_count: 2,
            invalid_session_count: 0,
            missing_seat_student_count: 2,
            invigilator_conflict_count: 0,
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
    fn readiness_reports_missing_active_student_seats() {
        let readiness = build_readiness(WorkspaceCounts {
            day_count: 1,
            item_count: 4,
            unscheduled_count: 0,
            missing_room_assignment_count: 0,
            invalid_session_count: 0,
            missing_seat_student_count: 3,
            invigilator_conflict_count: 0,
        });

        assert!(!readiness.can_publish);
        assert!(readiness
            .blockers
            .iter()
            .any(|value| value.contains("active student")));
    }

    #[test]
    fn readiness_reports_invalid_scheduled_sessions() {
        let readiness = build_readiness(WorkspaceCounts {
            day_count: 1,
            item_count: 4,
            unscheduled_count: 0,
            missing_room_assignment_count: 0,
            invalid_session_count: 2,
            missing_seat_student_count: 0,
            invigilator_conflict_count: 0,
        });

        assert!(!readiness.can_publish);
        assert!(readiness
            .blockers
            .iter()
            .any(|value| value.contains("no longer fit")));
    }

    #[test]
    fn readiness_reports_invigilator_live_range_conflicts() {
        let readiness = build_readiness(WorkspaceCounts {
            day_count: 1,
            item_count: 4,
            unscheduled_count: 0,
            missing_room_assignment_count: 0,
            invalid_session_count: 0,
            missing_seat_student_count: 0,
            invigilator_conflict_count: 2,
        });

        assert!(!readiness.can_publish);
        assert!(readiness
            .blockers
            .iter()
            .any(|value| value.contains("overlapping invigilator")));
    }

    #[test]
    fn rejects_round_update_without_supplied_fields() {
        let result = normalize_update_round_request(UpdateExamRoundRequest {
            name: None,
            description: None,
            exam_kind: None,
        });

        assert!(matches!(
            result,
            Err(AppError::BadRequest(message)) if message.contains("No fields")
        ));
    }

    #[test]
    fn normalizes_supported_exam_round_kinds() {
        assert_eq!(normalize_exam_kind(None).unwrap(), "midterm");
        assert_eq!(normalize_exam_kind(Some(" final ")).unwrap(), "final");

        assert!(matches!(
            normalize_exam_kind(Some("quiz")),
            Err(AppError::BadRequest(message)) if message.contains("midterm or final")
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

    #[test]
    fn generates_padded_seat_numbers_in_input_order() {
        let students = vec![
            SeatStudent {
                student_id: Uuid::nil(),
            },
            SeatStudent {
                student_id: Uuid::max(),
            },
        ];
        let seats = build_default_seat_assignments(&students);
        assert_eq!(seats[0].seat_number, "01");
        assert_eq!(seats[1].seat_number, "02");
    }

    #[test]
    fn rejects_seat_generation_when_student_count_exceeds_capacity() {
        let result = validate_seat_generation_capacity(41, 40);

        assert!(matches!(
            result,
            Err(AppError::BadRequest(message)) if message.contains("exceeds")
        ));
    }

    #[test]
    fn rejects_seat_generation_when_effective_capacity_is_not_positive() {
        let result = validate_seat_generation_capacity(0, 0);

        assert!(matches!(
            result,
            Err(AppError::BadRequest(message)) if message.contains("greater than zero")
        ));
    }
}
