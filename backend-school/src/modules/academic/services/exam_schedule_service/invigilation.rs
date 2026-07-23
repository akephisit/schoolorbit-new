use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};

use chrono::NaiveTime;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::{
    DayRoomAssignmentView, ExamInvigilatorAssignmentSummary, ExamInvigilatorDayWorkload,
    ExamInvigilatorStaffOption, ExamInvigilatorStaffWorkload, ExamInvigilatorView,
    ExamInvigilatorWorkspace, InvigilatorView, UpdateExamInvigilatorsRequest,
};

use super::room_assignments::{
    fetch_day_room_assignment_view, fetch_seat_assignment_context,
    map_day_room_assignment_write_error,
};
use super::rounds_and_days::{
    ensure_exam_round_is_mutable, fetch_round, mark_round_draft_after_mutation,
};
use super::shared::{
    exam_invigilator_staff_lock_keys, has_invigilator_time_conflict, minutes_between_times,
    unique_uuids, InvigilatorSessionWindow,
};

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
pub(super) struct InvigilatorSessionWindowRow {
    pub(super) assignment_id: Uuid,
    pub(super) exam_day_id: Uuid,
    pub(super) staff_id: Uuid,
    pub(super) staff_name: String,
    pub(super) starts_at: NaiveTime,
    pub(super) ends_at: NaiveTime,
}
#[derive(Debug, sqlx::FromRow)]
struct InvigilatorAssignmentMutationContext {
    assignment_id: Uuid,
    exam_day_id: Uuid,
    exam_round_id: Uuid,
    round_status: String,
}
impl InvigilatorViewRow {
    fn into_view(self) -> InvigilatorView {
        InvigilatorView {
            staff_id: self.staff_id,
            display_name: self.display_name,
        }
    }
}
pub(super) async fn lock_exam_invigilator_staff_conflict_scope(
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
pub(super) async fn replace_assignment_invigilators_in_tx(
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
pub(super) fn validate_unique_invigilator_staff_ids(ids: Vec<Uuid>) -> Result<Vec<Uuid>, AppError> {
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
pub(super) async fn validate_invigilator_time_conflicts(
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
pub(super) fn build_invigilator_candidate_session_windows(
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
pub(super) async fn fetch_invigilator_staff_ids_for_assignment(
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
pub(super) async fn validate_invigilator_candidate_session_conflicts(
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
pub(super) fn build_invigilator_staff_workloads(
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
pub(super) async fn fetch_invigilator_views_by_assignment_ids(
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
pub(super) fn invigilator_staff_option_limit(limit: Option<i64>) -> i64 {
    limit
        .unwrap_or(INVIGILATOR_STAFF_OPTION_DEFAULT_LIMIT)
        .clamp(1, INVIGILATOR_STAFF_OPTION_MAX_LIMIT)
}
pub(super) fn invigilator_staff_option_search_pattern(search: Option<String>) -> Option<String> {
    let trimmed = search?.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(format!("%{trimmed}%"))
    }
}
pub(super) async fn fetch_invigilators_by_assignment_ids(
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
pub(super) fn invigilators_for_assignment(
    assignment_id: Option<Uuid>,
    invigilators_by_assignment: &HashMap<Uuid, Vec<ExamInvigilatorView>>,
) -> Vec<ExamInvigilatorView> {
    assignment_id
        .and_then(|assignment_id| invigilators_by_assignment.get(&assignment_id).cloned())
        .unwrap_or_default()
}
