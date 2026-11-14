use super::invigilation::{fetch_invigilators_by_assignment_ids, invigilators_for_assignment};
use super::rounds_and_days::{fetch_exam_day_details_for_round, fetch_round};
use super::sessions_and_conflicts::ExamSessionRow;
use crate::exam_schedule::models::{
    ExamPaperReceiptItem, ExamScheduleItemView, ExamScheduleReadiness, ExamScheduleReadinessCode,
    ExamScheduleReadinessFinding, ExamScheduleWorkspace, ExamSessionView,
};
use school_database::DbSession;
use school_errors::AppError;
use sqlx::PgPool;
use uuid::Uuid;
#[derive(Debug, Clone, Copy)]
pub(super) struct WorkspaceCounts {
    pub(super) day_count: i64,
    pub(super) item_count: i64,
    pub(super) unscheduled_count: i64,
    pub(super) missing_room_assignment_count: i64,
    pub(super) invalid_session_count: i64,
    pub(super) missing_seat_student_count: i64,
    pub(super) invigilator_conflict_count: i64,
}
pub(super) fn build_readiness(counts: WorkspaceCounts) -> ExamScheduleReadiness {
    let mut findings = Vec::new();
    if counts.day_count == 0 {
        findings.push(ExamScheduleReadinessFinding {
            code: ExamScheduleReadinessCode::MissingExamDay,
            count: 1,
        });
    }
    if counts.item_count == 0 {
        findings.push(ExamScheduleReadinessFinding {
            code: ExamScheduleReadinessCode::MissingExamItems,
            count: 1,
        });
    }
    if counts.unscheduled_count > 0 {
        findings.push(ExamScheduleReadinessFinding {
            code: ExamScheduleReadinessCode::UnscheduledExamItems,
            count: counts.unscheduled_count,
        });
    }
    if counts.missing_room_assignment_count > 0 {
        findings.push(ExamScheduleReadinessFinding {
            code: ExamScheduleReadinessCode::MissingRoomAssignments,
            count: counts.missing_room_assignment_count,
        });
    }
    if counts.invalid_session_count > 0 {
        findings.push(ExamScheduleReadinessFinding {
            code: ExamScheduleReadinessCode::InvalidExamSessions,
            count: counts.invalid_session_count,
        });
    }
    if counts.missing_seat_student_count > 0 {
        findings.push(ExamScheduleReadinessFinding {
            code: ExamScheduleReadinessCode::MissingSeatAssignments,
            count: counts.missing_seat_student_count,
        });
    }
    if counts.invigilator_conflict_count > 0 {
        findings.push(ExamScheduleReadinessFinding {
            code: ExamScheduleReadinessCode::InvigilatorConflicts,
            count: counts.invigilator_conflict_count,
        });
    }
    ExamScheduleReadiness {
        complete: findings.is_empty(),
        findings,
    }
}

pub(super) fn build_readiness_with_current_sources(
    counts: WorkspaceCounts,
    invalid_sources: i64,
) -> ExamScheduleReadiness {
    let mut readiness = build_readiness(counts);
    if invalid_sources > 0 {
        readiness.complete = false;
        readiness.findings.push(ExamScheduleReadinessFinding {
            code: ExamScheduleReadinessCode::InvalidCurrentSources,
            count: invalid_sources,
        });
    }
    readiness
}

pub async fn get_workspace(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<ExamScheduleWorkspace, AppError> {
    let mut connection = school_tenancy::TenantPool::observed_checkout_pool(
        pool,
        "academic_assessment.exam_schedule.services.workspace.get_workspace",
    )
    .await?;
    get_workspace_on_connection(&mut connection, round_id).await
}

pub async fn get_workspace_on_connection(
    connection: &mut sqlx::PgConnection,
    round_id: Uuid,
) -> Result<ExamScheduleWorkspace, AppError> {
    let round = fetch_round(connection, round_id).await?;
    let days = fetch_exam_day_details_for_round(connection, round_id).await?;
    let unscheduled_items = fetch_unscheduled_items(connection, round_id).await?;
    let scheduled_sessions = fetch_scheduled_sessions(connection, round_id).await?;
    let paper_receipt_items = fetch_paper_receipt_items(connection, round_id).await?;
    let mut counts = fetch_workspace_counts(connection, round_id).await?;
    counts.item_count = (unscheduled_items.len() + scheduled_sessions.len()) as i64;
    counts.unscheduled_count = unscheduled_items.len() as i64;
    let invalid_sources: i64 = DbSession::new(connection).scalar("academic_assessment.exam_schedule.workspace.current_source_findings", "SELECT count(*) FROM academic_exam_sessions session JOIN academic_exam_schedule_items item ON item.id=session.exam_schedule_item_id LEFT JOIN academic_exam_eligible_sources source ON source.assessment_phase_id=item.assessment_phase_id AND source.learning_group_id=item.learning_group_id AND source.homeroom_id=item.homeroom_id AND source.exam_kind=$2 WHERE item.exam_round_id=$1 AND (source.source_id IS NULL OR source.duration_minutes<>EXTRACT(EPOCH FROM(session.ends_at-session.starts_at))/60)").bind(round_id).bind(&round.exam_kind).fetch_one().await?;
    let readiness = build_readiness_with_current_sources(counts, invalid_sources);

    Ok(ExamScheduleWorkspace {
        round,
        days,
        unscheduled_items,
        scheduled_sessions,
        paper_receipt_items,
        readiness,
    })
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
        WITH seat_roster AS (
            SELECT assignment.id AS assignment_id, student_year.student_id,
                   placement.class_number, seat.seat_number,
                   COUNT(*) OVER (PARTITION BY assignment.id, placement.class_number) AS number_count
            FROM academic_exam_day_room_assignments assignment
            JOIN academic_exam_days day ON day.id = assignment.exam_day_id
            JOIN homeroom_placements placement
              ON placement.homeroom_id = assignment.homeroom_id AND placement.status = 'current'
            JOIN student_academic_years student_year
              ON student_year.id = placement.student_academic_year_id
             AND student_year.academic_year_id = assignment.academic_year_id
             AND student_year.status = 'active'
            JOIN users student ON student.id = student_year.student_id
             AND student.user_type = 'student' AND student.status = 'active'
            LEFT JOIN academic_exam_seat_assignments seat
              ON $2 AND seat.day_room_assignment_id = assignment.id
             AND seat.student_id = student_year.student_id
            WHERE day.exam_round_id = $1
        )
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
                                       item.homeroom_id
                       FROM academic_exam_sessions session
                       JOIN academic_exam_schedule_items item
                         ON item.id = session.exam_schedule_item_id
                        AND item.exam_round_id = session.exam_round_id
                       LEFT JOIN academic_exam_day_room_assignments assignment
                         ON assignment.exam_day_id = session.exam_day_id
                        AND assignment.homeroom_id = item.homeroom_id
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
                        AND assignment.homeroom_id = item.homeroom_id
                       JOIN learning_group_students enrollment
                         ON enrollment.learning_group_id = item.learning_group_id
                        AND enrollment.academic_term_id = item.academic_term_id
                        AND enrollment.membership_status = 'active'
                       JOIN users user_account
                         ON user_account.id = enrollment.student_id
                        AND user_account.user_type = 'student'
                        AND user_account.status = 'active'
                       LEFT JOIN seat_roster roster
                         ON roster.assignment_id = assignment.id
                        AND roster.student_id = enrollment.student_id
                       WHERE session.exam_round_id = $1
                         AND roster.student_id IS NULL
                       UNION
                       SELECT roster.assignment_id, roster.student_id
                       FROM seat_roster roster
                       WHERE roster.class_number IS NULL OR roster.class_number <= 0
                          OR roster.number_count > 1
                          OR ($2 AND roster.seat_number IS DISTINCT FROM roster.class_number::text)
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
                    AND left_item.homeroom_id = left_assignment.homeroom_id
                   JOIN academic_exam_sessions right_session
                     ON right_session.exam_day_id = right_assignment.exam_day_id
                    AND right_session.exam_round_id = day.exam_round_id
                   JOIN academic_exam_schedule_items right_item
                     ON right_item.id = right_session.exam_schedule_item_id
                    AND right_item.homeroom_id = right_assignment.homeroom_id
                   WHERE day.exam_round_id = $1
                     AND left_session.starts_at < right_session.ends_at
                     AND right_session.starts_at < left_session.ends_at
               ) AS invigilator_conflict_count
        "#;

pub(super) async fn fetch_unscheduled_items(
    connection: &mut sqlx::PgConnection,
    round_id: Uuid,
) -> Result<Vec<ExamScheduleItemView>, AppError> {
    DbSession::new(connection).query_as::<ExamScheduleItemView>("academic_assessment.exam_schedule.services.workspace.fetch_unscheduled_items",
        r#"
        WITH item AS (
          SELECT COALESCE(saved.id,uuid_generate_v5(round.id,source.source_id::text)) id,
            round.id exam_round_id,source.academic_term_id,source.academic_year_id,
            source.assessment_phase_id,source.course_assessment_plan_id,source.learning_offering_id,
            source.learning_group_id,source.homeroom_id,source.subject_id,source.grade_level_id,
            source.duration_minutes,COALESCE(saved.imported_at,round.created_at) imported_at
          FROM academic_exam_rounds round JOIN academic_exam_eligible_sources source
            ON source.academic_term_id=round.academic_term_id AND source.academic_year_id=round.academic_year_id AND source.exam_kind=round.exam_kind
          LEFT JOIN academic_exam_schedule_items saved ON saved.exam_round_id=round.id
            AND saved.assessment_phase_id=source.assessment_phase_id AND saved.learning_group_id=source.learning_group_id AND saved.homeroom_id=source.homeroom_id
          WHERE round.id=$1
        )
        SELECT item.id,
               item.exam_round_id,
               item.academic_term_id,
               item.academic_year_id,
               item.assessment_phase_id,
               item.course_assessment_plan_id,
               item.learning_offering_id,
               item.learning_group_id,
               item.homeroom_id,
               item.subject_id,
               item.grade_level_id,
               item.duration_minutes,
               item.imported_at,
               CASE phase.phase_code
                   WHEN 'midterm' THEN 'กลางภาค'
                   WHEN 'final' THEN 'ปลายภาค'
                   WHEN 'before_midterm' THEN 'ก่อนกลางภาค'
                   WHEN 'after_midterm' THEN 'หลังกลางภาค'
               END AS assessment_phase_name,
               subject.code AS subject_code,
               subject_version.name_th AS subject_name_th,
               subject_version.name_en AS subject_name_en,
               offering.name_snapshot AS subject_version_display_label,
               subject.subject_group_id,
               subject_group.name_th AS subject_group_name,
               subject_group.display_order AS subject_group_display_order,
               subject_version.type AS subject_type,
               classroom.name AS homeroom_name,
               CASE grade_level.level_type
                   WHEN 'kindergarten' THEN CONCAT('อ.', grade_level.year)
                   WHEN 'primary' THEN CONCAT('ป.', grade_level.year)
                   WHEN 'secondary' THEN CONCAT('ม.', grade_level.year)
                   ELSE CONCAT('?.', grade_level.year)
               END AS grade_level_name,
               grade_level.level_type AS grade_level_type,
               grade_level.year AS grade_level_year
        FROM item
        JOIN course_assessment_phases phase
          ON phase.id = item.assessment_phase_id
        JOIN learning_offerings offering ON offering.id = item.learning_offering_id
        JOIN course_offering_details course_detail
          ON course_detail.learning_offering_id = item.learning_offering_id
        JOIN subject_versions subject_version ON subject_version.id = course_detail.subject_version_id
        JOIN subjects subject ON subject.id = item.subject_id
        LEFT JOIN subject_groups subject_group ON subject_group.id = subject.subject_group_id
        JOIN homerooms classroom ON classroom.id = item.homeroom_id
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
                 CASE subject_version.type
                     WHEN 'BASIC' THEN 1
                     WHEN 'ADDITIONAL' THEN 2
                     WHEN 'ACTIVITY' THEN 3
                     ELSE 4
                 END,
                 subject.code,
                 classroom.room_number NULLS LAST,
                 classroom.name,
                 CASE phase.phase_code
                     WHEN 'before_midterm' THEN 1
                     WHEN 'midterm' THEN 2
                     WHEN 'after_midterm' THEN 3
                     WHEN 'final' THEN 4
                 END,
                 item.id
        "#,
    )
    .bind(round_id)
    .fetch_all()
    .await
    .map_err(AppError::from)
}

pub(super) async fn fetch_scheduled_sessions(
    connection: &mut sqlx::PgConnection,
    round_id: Uuid,
) -> Result<Vec<ExamSessionView>, AppError> {
    let rows = DbSession::new(connection).query_as::<ExamSessionRow>("academic_assessment.exam_schedule.services.workspace.fetch_scheduled_sessions",
        r#"
        SELECT session.id,
               session.exam_schedule_item_id,
               session.exam_round_id,
               session.exam_day_id,
               session.starts_at,
               session.ends_at,
               item.academic_term_id,
               item.academic_year_id,
               item.assessment_phase_id,
               item.course_assessment_plan_id,
               item.learning_offering_id,
               item.learning_group_id,
               item.homeroom_id,
               item.subject_id,
               item.grade_level_id,
               item.duration_minutes,
               item.imported_at,
               day.exam_date AS exam_date,
               CASE phase.phase_code
                   WHEN 'midterm' THEN 'กลางภาค'
                   WHEN 'final' THEN 'ปลายภาค'
                   WHEN 'before_midterm' THEN 'ก่อนกลางภาค'
                   WHEN 'after_midterm' THEN 'หลังกลางภาค'
               END AS assessment_phase_name,
               subject.code AS subject_code,
               subject_version.name_th AS subject_name_th,
               subject_version.name_en AS subject_name_en,
               offering.name_snapshot AS subject_version_display_label,
               subject.subject_group_id,
               subject_group.name_th AS subject_group_name,
               subject_group.display_order AS subject_group_display_order,
               subject_version.type AS subject_type,
               classroom.name AS homeroom_name,
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
        JOIN course_assessment_phases phase
          ON phase.id = item.assessment_phase_id
        JOIN learning_offerings offering ON offering.id = item.learning_offering_id
        JOIN course_offering_details course_detail
          ON course_detail.learning_offering_id = item.learning_offering_id
        JOIN subject_versions subject_version ON subject_version.id = course_detail.subject_version_id
        JOIN subjects subject ON subject.id = item.subject_id
        LEFT JOIN subject_groups subject_group ON subject_group.id = subject.subject_group_id
        JOIN homerooms classroom ON classroom.id = item.homeroom_id
        JOIN grade_levels grade_level ON grade_level.id = item.grade_level_id
        LEFT JOIN academic_exam_day_room_assignments assignment
          ON assignment.exam_day_id = session.exam_day_id
         AND assignment.homeroom_id = item.homeroom_id
        LEFT JOIN rooms room ON room.id = assignment.room_id
        LEFT JOIN buildings building ON building.id = room.building_id
        WHERE session.exam_round_id = $1
        ORDER BY day.exam_date,
                 day.start_time,
                 day.id,
                 session.starts_at,
                 classroom.name,
                 subject.code,
                 CASE phase.phase_code
                     WHEN 'before_midterm' THEN 1
                     WHEN 'midterm' THEN 2
                     WHEN 'after_midterm' THEN 3
                     WHEN 'final' THEN 4
                 END,
                 session.id
        "#,
    )
    .bind(round_id)
    .fetch_all()
    .await?;

    let assignment_ids: Vec<Uuid> = rows
        .iter()
        .filter_map(|row| row.day_room_assignment_id)
        .collect();
    let invigilators_by_assignment =
        fetch_invigilators_by_assignment_ids(connection, &assignment_ids).await?;

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
    connection: &mut sqlx::PgConnection,
    round_id: Uuid,
) -> Result<WorkspaceCounts, AppError> {
    let row: (i64, i64, i64, i64, i64, i64, i64) = DbSession::new(connection)
        .query_as(
            "academic_assessment.exam_schedule.services.workspace.fetch_workspace_counts",
            WORKSPACE_COUNTS_SQL,
        )
        .bind(round_id)
        .bind(true) // Current completeness includes the actual canonical seat assignments.
        .fetch_one()
        .await?;

    Ok(workspace_counts_from_row(row))
}

// Receipt collection covers configured exams even before scheduling readiness/import.
// It must never put outside-timetable exams into the scheduling tray or readiness counts.
async fn fetch_paper_receipt_items(
    connection: &mut sqlx::PgConnection,
    round_id: Uuid,
) -> Result<Vec<ExamPaperReceiptItem>, AppError> {
    Ok(DbSession::new(connection)
        .query_as::<ExamPaperReceiptItem>(
            "academic_assessment.exam_schedule.services.workspace.fetch_paper_receipt_items",
            r#"
        SELECT DISTINCT phase.id AS assessment_phase_id,
               learning_group.id AS learning_group_id,
               homeroom.id AS homeroom_id,
               subject.code AS subject_code,
               version.name_th AS subject_name_th,
               version.name_en AS subject_name_en,
               subject.subject_group_id,
               subject_group.name_th AS subject_group_name,
               subject_group.display_order AS subject_group_display_order,
               grade.level_type AS grade_level_type,
               grade.year AS grade_level_year,
               homeroom.name AS homeroom_name,
               phase.exam_arrangement
        FROM academic_exam_rounds round
        JOIN course_assessment_plans plan
          ON plan.academic_term_id = round.academic_term_id
         AND plan.academic_year_id = round.academic_year_id
        JOIN course_assessment_phases phase
          ON phase.plan_id = plan.id AND phase.phase_code = round.exam_kind
        JOIN course_offering_details detail
          ON detail.learning_offering_id = plan.learning_offering_id
         AND detail.subject_version_id = plan.subject_version_id
         AND detail.academic_term_id = plan.academic_term_id
         AND detail.academic_year_id = plan.academic_year_id
        JOIN subject_versions version ON version.id = plan.subject_version_id
        JOIN subjects subject ON subject.id = version.subject_id
        LEFT JOIN subject_groups subject_group ON subject_group.id = subject.subject_group_id
        JOIN learning_groups learning_group
          ON learning_group.learning_offering_id = plan.learning_offering_id
         AND learning_group.academic_term_id = plan.academic_term_id
         AND learning_group.academic_year_id = plan.academic_year_id
        JOIN learning_group_homerooms coverage ON coverage.learning_group_id = learning_group.id
        JOIN homerooms homeroom
          ON homeroom.id = coverage.homeroom_id
         AND homeroom.academic_year_id = plan.academic_year_id
         AND homeroom.is_active
        JOIN grade_levels grade ON grade.id = homeroom.grade_level_id
        WHERE round.id = $1
          AND phase.exam_arrangement IN ('in_timetable', 'outside_timetable')
        ORDER BY subject_group_display_order NULLS LAST, subject_group_name NULLS LAST,
                 grade_level_type, grade_level_year, subject_code, homeroom_name,
                 assessment_phase_id, learning_group_id, homeroom_id
        "#,
        )
        .bind(round_id)
        .fetch_all()
        .await?)
}
