use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::{
    ClearMismatchedExamItemsResult, ExamScheduleItemView, ExamScheduleReadiness,
    ExamScheduleWorkspace, ExamSessionView, ImportExamItemsRequest, ImportExamItemsResult,
};

use super::invigilation::{fetch_invigilators_by_assignment_ids, invigilators_for_assignment};
use super::rounds_and_days::{
    fetch_exam_day_details_for_round, fetch_round, mark_round_draft_after_mutation,
};
use super::sessions_and_conflicts::ExamSessionRow;
use super::shared::unique_uuids;

#[derive(Debug, Clone, Copy)]
pub struct WorkspaceCounts {
    pub day_count: i64,
    pub item_count: i64,
    pub unscheduled_count: i64,
    pub missing_room_assignment_count: i64,
    pub invalid_session_count: i64,
    pub missing_seat_student_count: i64,
    pub invigilator_conflict_count: i64,
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
    if counts.invalid_session_count > 0 {
        blockers.push(format!(
            "Fix {} scheduled exam session(s) that no longer fit the exam day",
            counts.invalid_session_count
        ));
    }
    if counts.missing_seat_student_count > 0 {
        blockers.push(format!(
            "Generate seats for {} active student(s) in scheduled classroom-day group(s)",
            counts.missing_seat_student_count
        ));
    }
    if counts.invigilator_conflict_count > 0 {
        blockers.push(format!(
            "Fix {} overlapping invigilator supervision assignment(s)",
            counts.invigilator_conflict_count
        ));
    }
    ExamScheduleReadiness {
        can_publish: blockers.is_empty(),
        blockers,
    }
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

pub async fn import_exam_items(
    pool: &PgPool,
    round_id: Uuid,
    request: ImportExamItemsRequest,
    actor_user_id: Uuid,
) -> Result<ImportExamItemsResult, AppError> {
    let grade_level_ids = request.grade_level_ids.map(unique_uuids);
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

    let skipped_existing_count: i64 = sqlx::query_scalar(
        r#"
        WITH round_context AS (
          SELECT id AS exam_round_id, academic_semester_id, exam_kind
          FROM academic_exam_rounds
          WHERE id = $1
        ),
        source_items AS (
          SELECT
            rc.exam_round_id,
            c.id AS assessment_category_id,
            cr.id AS classroom_id
          FROM round_context rc
          JOIN academic_assessment_plans ap
            ON ap.academic_semester_id = rc.academic_semester_id
          JOIN academic_assessment_categories c
            ON c.plan_id = ap.id
          JOIN classroom_courses cc
            ON cc.academic_semester_id = rc.academic_semester_id
           AND cc.subject_id = ap.subject_id
          JOIN class_rooms cr
            ON cr.id = cc.classroom_id
          WHERE c.exam_mode = 'in_timetable'
            AND c.code = rc.exam_kind
            AND c.exam_duration_minutes IS NOT NULL
            AND cr.is_active = true
            AND ($2::uuid[] IS NULL OR cr.grade_level_id = ANY($2))
        )
        SELECT COUNT(*)::BIGINT
        FROM source_items source
        WHERE EXISTS (
            SELECT 1
            FROM academic_exam_schedule_items item
            WHERE item.exam_round_id = source.exam_round_id
              AND item.assessment_category_id = source.assessment_category_id
              AND item.classroom_id = source.classroom_id
        )
        "#,
    )
    .bind(round_id)
    .bind(grade_level_ids.clone())
    .fetch_one(&mut *tx)
    .await?;

    let skipped_missing_duration_count: i64 = sqlx::query_scalar(
        r#"
        WITH round_context AS (
          SELECT id AS exam_round_id, academic_semester_id, exam_kind
          FROM academic_exam_rounds
          WHERE id = $1
        )
        SELECT COUNT(*)::BIGINT
        FROM round_context rc
        JOIN academic_assessment_plans ap
          ON ap.academic_semester_id = rc.academic_semester_id
        JOIN academic_assessment_categories c
          ON c.plan_id = ap.id
        JOIN classroom_courses cc
          ON cc.academic_semester_id = rc.academic_semester_id
         AND cc.subject_id = ap.subject_id
        JOIN class_rooms cr
          ON cr.id = cc.classroom_id
        WHERE c.exam_mode = 'in_timetable'
          AND c.code = rc.exam_kind
          AND c.exam_duration_minutes IS NULL
          AND cr.is_active = true
          AND ($2::uuid[] IS NULL OR cr.grade_level_id = ANY($2))
        "#,
    )
    .bind(round_id)
    .bind(grade_level_ids.clone())
    .fetch_one(&mut *tx)
    .await?;

    let insert_result = sqlx::query(
        r#"
        WITH round_context AS (
          SELECT id AS exam_round_id, academic_semester_id, exam_kind
          FROM academic_exam_rounds
          WHERE id = $1
        ),
        source_items AS (
          SELECT
            rc.exam_round_id,
            rc.academic_semester_id,
            c.id AS assessment_category_id,
            ap.id AS assessment_plan_id,
            cc.id AS classroom_course_id,
            cr.id AS classroom_id,
            ap.subject_id,
            cr.grade_level_id,
            c.exam_duration_minutes AS duration_minutes
          FROM round_context rc
          JOIN academic_assessment_plans ap
            ON ap.academic_semester_id = rc.academic_semester_id
          JOIN academic_assessment_categories c
            ON c.plan_id = ap.id
          JOIN classroom_courses cc
            ON cc.academic_semester_id = rc.academic_semester_id
           AND cc.subject_id = ap.subject_id
          JOIN class_rooms cr
            ON cr.id = cc.classroom_id
          WHERE c.exam_mode = 'in_timetable'
            AND c.code = rc.exam_kind
            AND c.exam_duration_minutes IS NOT NULL
            AND cr.is_active = true
            AND ($2::uuid[] IS NULL OR cr.grade_level_id = ANY($2))
        )
        INSERT INTO academic_exam_schedule_items (
          exam_round_id,
          academic_semester_id,
          assessment_category_id,
          assessment_plan_id,
          classroom_course_id,
          classroom_id,
          subject_id,
          grade_level_id,
          duration_minutes
        )
        SELECT
          exam_round_id,
          academic_semester_id,
          assessment_category_id,
          assessment_plan_id,
          classroom_course_id,
          classroom_id,
          subject_id,
          grade_level_id,
          duration_minutes
        FROM source_items
        ON CONFLICT (exam_round_id, assessment_category_id, classroom_id) DO NOTHING
        "#,
    )
    .bind(round_id)
    .bind(grade_level_ids)
    .execute(&mut *tx)
    .await?;

    mark_round_draft_after_mutation(&mut tx, round_id, Some(actor_user_id)).await?;
    tx.commit().await?;

    Ok(ImportExamItemsResult {
        inserted_count: insert_result.rows_affected() as i64,
        skipped_existing_count,
        skipped_missing_duration_count,
    })
}

pub async fn clear_mismatched_exam_items(
    pool: &PgPool,
    round_id: Uuid,
    actor_user_id: Uuid,
) -> Result<ClearMismatchedExamItemsResult, AppError> {
    let mut tx = pool.begin().await?;

    let _round_status: String = sqlx::query_scalar(
        r#"
        SELECT status
        FROM academic_exam_rounds
        WHERE id = $1
        FOR UPDATE
        "#,
    )
    .bind(round_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam round not found".to_string()))?;

    let delete_result = sqlx::query(
        r#"
        WITH round_context AS (
          SELECT id AS exam_round_id, exam_kind
          FROM academic_exam_rounds
          WHERE id = $1
        )
        DELETE FROM academic_exam_schedule_items item
        USING academic_assessment_categories c, round_context rc
        WHERE item.exam_round_id = rc.exam_round_id
          AND item.assessment_category_id = c.id
          AND c.code IS DISTINCT FROM rc.exam_kind
        "#,
    )
    .bind(round_id)
    .execute(&mut *tx)
    .await?;

    mark_round_draft_after_mutation(&mut tx, round_id, Some(actor_user_id)).await?;
    tx.commit().await?;

    Ok(ClearMismatchedExamItemsResult {
        deleted_count: delete_result.rows_affected() as i64,
    })
}

pub(super) async fn fetch_workspace_counts_in_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    round_id: Uuid,
) -> Result<WorkspaceCounts, AppError> {
    let row: (i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(WORKSPACE_COUNTS_SQL)
        .bind(round_id)
        .fetch_one(&mut **tx)
        .await?;

    Ok(workspace_counts_from_row(row))
}

pub(super) fn workspace_counts_from_row(
    (
        day_count,
        item_count,
        unscheduled_count,
        missing_room_assignment_count,
        invalid_session_count,
        missing_seat_student_count,
        invigilator_conflict_count,
    ): (i64, i64, i64, i64, i64, i64, i64),
) -> WorkspaceCounts {
    WorkspaceCounts {
        day_count,
        item_count,
        unscheduled_count,
        missing_room_assignment_count,
        invalid_session_count,
        missing_seat_student_count,
        invigilator_conflict_count,
    }
}

pub(super) const WORKSPACE_COUNTS_SQL: &str = r#"
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
                       SELECT session.id
                       FROM academic_exam_sessions session
                       JOIN academic_exam_schedule_items item
                         ON item.id = session.exam_schedule_item_id
                        AND item.exam_round_id = session.exam_round_id
                       JOIN academic_exam_days day
                         ON day.id = session.exam_day_id
                        AND day.exam_round_id = session.exam_round_id
                       WHERE session.exam_round_id = $1
                         AND (
                             session.starts_at < day.start_time
                             OR session.ends_at > day.end_time
                             OR (EXTRACT(EPOCH FROM session.starts_at)::BIGINT % 300) <> 0
                             OR EXISTS (
                                 SELECT 1
                                 FROM academic_exam_day_blocked_windows blocked
                                 WHERE blocked.exam_day_id = session.exam_day_id
                                   AND session.starts_at < blocked.end_time
                                   AND blocked.start_time < session.ends_at
                             )
                             OR (
                                 EXISTS (
                                     SELECT 1
                                     FROM academic_exam_day_grade_levels scope
                                     WHERE scope.exam_day_id = session.exam_day_id
                                 )
                                 AND NOT EXISTS (
                                     SELECT 1
                                     FROM academic_exam_day_grade_levels scope
                                     WHERE scope.exam_day_id = session.exam_day_id
                                       AND scope.grade_level_id = item.grade_level_id
                                 )
                             )
                         )
                   ) invalid_sessions
               ) AS invalid_session_count,
               (
                   SELECT COUNT(*)::BIGINT
                   FROM (
                       SELECT DISTINCT assignment.id AS day_room_assignment_id,
                                       enrollment.student_id
                       FROM academic_exam_sessions session
                       JOIN academic_exam_schedule_items item
                         ON item.id = session.exam_schedule_item_id
                        AND item.exam_round_id = session.exam_round_id
                       JOIN academic_exam_day_room_assignments assignment
                         ON assignment.exam_day_id = session.exam_day_id
                        AND assignment.classroom_id = item.classroom_id
                       JOIN student_class_enrollments enrollment
                         ON enrollment.class_room_id = item.classroom_id
                        AND enrollment.status = 'active'
                       JOIN users user_account
                         ON user_account.id = enrollment.student_id
                        AND user_account.user_type = 'student'
                        AND user_account.status = 'active'
                       LEFT JOIN academic_exam_seat_assignments seat
                         ON seat.day_room_assignment_id = assignment.id
                        AND seat.student_id = enrollment.student_id
                       WHERE session.exam_round_id = $1
                         AND seat.student_id IS NULL
                   ) missing_seat_students
               ) AS missing_seat_student_count,
               (
                   SELECT COUNT(*)::BIGINT
                   FROM academic_exam_day_invigilators left_invigilator
                   JOIN academic_exam_day_invigilators right_invigilator
                     ON right_invigilator.staff_id = left_invigilator.staff_id
                    AND right_invigilator.exam_day_id = left_invigilator.exam_day_id
                    AND right_invigilator.day_room_assignment_id <> left_invigilator.day_room_assignment_id
                    AND right_invigilator.id > left_invigilator.id
                   JOIN academic_exam_day_room_assignments left_assignment
                     ON left_assignment.id = left_invigilator.day_room_assignment_id
                    AND left_assignment.exam_day_id = left_invigilator.exam_day_id
                   JOIN academic_exam_day_room_assignments right_assignment
                     ON right_assignment.id = right_invigilator.day_room_assignment_id
                    AND right_assignment.exam_day_id = right_invigilator.exam_day_id
                   JOIN academic_exam_days day
                     ON day.id = left_invigilator.exam_day_id
                   JOIN academic_exam_sessions left_session
                     ON left_session.exam_day_id = left_assignment.exam_day_id
                    AND left_session.exam_round_id = day.exam_round_id
                   JOIN academic_exam_schedule_items left_item
                     ON left_item.id = left_session.exam_schedule_item_id
                    AND left_item.classroom_id = left_assignment.classroom_id
                   JOIN academic_exam_sessions right_session
                     ON right_session.exam_day_id = right_assignment.exam_day_id
                    AND right_session.exam_round_id = day.exam_round_id
                   JOIN academic_exam_schedule_items right_item
                     ON right_item.id = right_session.exam_schedule_item_id
                    AND right_item.classroom_id = right_assignment.classroom_id
                   WHERE day.exam_round_id = $1
                     AND left_session.starts_at < right_session.ends_at
                     AND right_session.starts_at < left_session.ends_at
               ) AS invigilator_conflict_count
        "#;

pub(super) async fn fetch_unscheduled_items(
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
               grade_level.year AS grade_level_year
        FROM academic_exam_schedule_items item
        JOIN academic_assessment_categories category
          ON category.id = item.assessment_category_id
        JOIN subjects subject ON subject.id = item.subject_id
        LEFT JOIN subject_groups subject_group ON subject_group.id = subject.group_id
        JOIN class_rooms classroom ON classroom.id = item.classroom_id
        JOIN grade_levels grade_level ON grade_level.id = item.grade_level_id
        WHERE item.exam_round_id = $1
          AND NOT EXISTS (
              SELECT 1
              FROM academic_exam_sessions session
              WHERE session.exam_schedule_item_id = item.id
          )
        ORDER BY subject_group.display_order NULLS LAST,
                 subject_group.name_th NULLS LAST,
                 CASE grade_level.level_type
                     WHEN 'kindergarten' THEN 1
                     WHEN 'primary' THEN 2
                     WHEN 'secondary' THEN 3
                     ELSE 4
                 END,
                 grade_level.year,
                 CASE subject.type
                     WHEN 'BASIC' THEN 1
                     WHEN 'ADDITIONAL' THEN 2
                     WHEN 'ACTIVITY' THEN 3
                     ELSE 4
                 END,
                 subject.code,
                 classroom.room_number NULLS LAST,
                 classroom.name,
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

pub(super) async fn fetch_scheduled_sessions(
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
        WHERE session.exam_round_id = $1
        ORDER BY day.exam_date,
                 day.start_time,
                 day.id,
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

pub(super) async fn fetch_workspace_counts(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<WorkspaceCounts, AppError> {
    let row: (i64, i64, i64, i64, i64, i64, i64) = sqlx::query_as(WORKSPACE_COUNTS_SQL)
        .bind(round_id)
        .fetch_one(pool)
        .await?;

    Ok(workspace_counts_from_row(row))
}
