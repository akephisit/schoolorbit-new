use std::collections::HashSet;

use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool, Postgres, Transaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::{
    ExamScheduleItemView, ExamScheduleReadiness, ExamScheduleReadinessCode,
    ExamScheduleReadinessFinding, ExamScheduleWorkspace, ExamSessionView, ExamSourceChange,
    ExamSourceChangeKind, ExamSourcePreview, ExamSourceSyncItemResult, ExamSourceSyncItemStatus,
    SyncExamSourcesRequest, SyncExamSourcesResult,
};

use super::invigilation::{fetch_invigilators_by_assignment_ids, invigilators_for_assignment};
use super::rounds_and_days::{
    ensure_exam_round_is_mutable, fetch_exam_day_details_for_round, fetch_round,
};
use super::sessions_and_conflicts::{revalidate_session_duration_change_in_tx, ExamSessionRow};

#[derive(Debug, FromRow)]
struct ExamSourceChangeRow {
    source_id: Uuid,
    exam_schedule_item_id: Option<Uuid>,
    assessment_phase_id: Uuid,
    learning_group_id: Uuid,
    homeroom_id: Uuid,
    subject_id: Uuid,
    grade_level_id: Uuid,
    subject_code: String,
    subject_name: String,
    homeroom_name: String,
    change_kind: String,
    snapshot_duration_minutes: Option<i32>,
    current_duration_minutes: Option<i32>,
    scheduled: bool,
}

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
        can_publish: findings.is_empty(),
        findings,
    }
}

pub(super) fn build_readiness_with_source_changes(
    counts: WorkspaceCounts,
    source_change_count: usize,
) -> ExamScheduleReadiness {
    let mut readiness = build_readiness(counts);
    if source_change_count > 0 {
        readiness.findings.push(ExamScheduleReadinessFinding {
            code: ExamScheduleReadinessCode::PendingSourceChanges,
            count: source_change_count as i64,
        });
        readiness.can_publish = false;
    }
    readiness
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
    let source_preview = preview_exam_sources(pool, round_id).await?;
    let readiness = build_readiness_with_source_changes(counts, source_preview.changes.len());

    Ok(ExamScheduleWorkspace {
        round,
        days,
        unscheduled_items,
        scheduled_sessions,
        source_preview,
        readiness,
    })
}

pub async fn preview_exam_sources(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<ExamSourcePreview, AppError> {
    let (status, row_version): (String, i64) =
        sqlx::query_as("SELECT status, row_version FROM academic_exam_rounds WHERE id = $1")
            .bind(round_id)
            .fetch_optional(pool)
            .await?
            .ok_or_else(|| AppError::NotFound("Exam round not found".to_string()))?;
    let rows = fetch_source_change_rows(pool, round_id).await?;
    build_source_preview(round_id, status, row_version, rows)
}

pub async fn sync_exam_sources(
    pool: &PgPool,
    round_id: Uuid,
    actor_user_id: Uuid,
    request: SyncExamSourcesRequest,
) -> Result<SyncExamSourcesResult, AppError> {
    let mut tx = pool.begin().await?;
    let (status, row_version): (String, i64) = sqlx::query_as(
        r#"SELECT status, row_version
           FROM academic_exam_rounds
           WHERE id = $1
           FOR UPDATE"#,
    )
    .bind(round_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or_else(|| AppError::NotFound("Exam round not found".to_string()))?;
    ensure_exam_round_is_mutable(&status)?;
    if row_version != request.round_row_version {
        return Err(AppError::Conflict(
            "Exam round changed after source preview; refresh before syncing".to_string(),
        ));
    }

    let rows = fetch_source_change_rows_in_tx(&mut tx, round_id).await?;
    let preview = build_source_preview(round_id, status, row_version, rows)?;
    if preview.preview_token != request.preview_token {
        return Err(AppError::Conflict(
            "Assessment sources changed after preview; refresh before syncing".to_string(),
        ));
    }

    let selected_ids = request.source_ids.into_iter().collect::<HashSet<_>>();
    if selected_ids.len() > preview.changes.len()
        || selected_ids.iter().any(|source_id| {
            !preview
                .changes
                .iter()
                .any(|change| change.source_id == *source_id)
        })
    {
        return Err(AppError::ValidationError(
            "Selected exam source is not part of the unchanged preview".to_string(),
        ));
    }

    let mut inserted_count = 0_i64;
    let mut updated_duration_count = 0_i64;
    let mut removed_count = 0_i64;
    let mut results = Vec::with_capacity(selected_ids.len());
    for change in preview
        .changes
        .iter()
        .filter(|change| selected_ids.contains(&change.source_id))
    {
        match change.change_kind {
            ExamSourceChangeKind::New => {
                let inserted =
                    insert_exam_source_in_tx(&mut tx, round_id, change.source_id).await?;
                if inserted != 1 {
                    results.push(source_sync_conflict(
                        change,
                        "New exam source changed while synchronizing".to_string(),
                    ));
                    continue;
                }
                inserted_count += inserted;
                results.push(source_sync_applied(change));
            }
            ExamSourceChangeKind::DurationChanged => {
                let Some(item_id) = change.exam_schedule_item_id else {
                    results.push(source_sync_conflict(
                        change,
                        "Changed exam source has no snapshot item".to_string(),
                    ));
                    continue;
                };
                let Some(duration) = change.current_duration_minutes else {
                    results.push(source_sync_conflict(
                        change,
                        "Changed exam source has no current duration".to_string(),
                    ));
                    continue;
                };
                if let Err(error) =
                    revalidate_session_duration_change_in_tx(&mut tx, item_id, duration).await
                {
                    match source_sync_validation_message(error) {
                        Ok(message) => {
                            results.push(source_sync_conflict(change, message));
                            continue;
                        }
                        Err(error) => return Err(error),
                    }
                }
                let updated = sqlx::query(
                    r#"UPDATE academic_exam_schedule_items
                       SET duration_minutes = $2, imported_at = now()
                       WHERE id = $1 AND exam_round_id = $3"#,
                )
                .bind(item_id)
                .bind(duration)
                .bind(round_id)
                .execute(&mut *tx)
                .await?;
                if updated.rows_affected() != 1 {
                    results.push(source_sync_conflict(
                        change,
                        "Changed exam source was removed while synchronizing".to_string(),
                    ));
                    continue;
                }
                updated_duration_count += 1;
                results.push(source_sync_applied(change));
            }
            ExamSourceChangeKind::NoLongerEligible => {
                let Some(item_id) = change.exam_schedule_item_id else {
                    results.push(source_sync_conflict(
                        change,
                        "Ineligible exam source has no snapshot item".to_string(),
                    ));
                    continue;
                };
                let deleted = sqlx::query(
                    "DELETE FROM academic_exam_schedule_items WHERE id = $1 AND exam_round_id = $2",
                )
                .bind(item_id)
                .bind(round_id)
                .execute(&mut *tx)
                .await?;
                if deleted.rows_affected() != 1 {
                    results.push(source_sync_conflict(
                        change,
                        "Ineligible exam source was removed while synchronizing".to_string(),
                    ));
                    continue;
                }
                removed_count += 1;
                results.push(source_sync_applied(change));
            }
        }
    }

    let applied_count = inserted_count + updated_duration_count + removed_count;
    let next_row_version = if applied_count == 0 {
        row_version
    } else {
        sqlx::query_scalar(
            r#"UPDATE academic_exam_rounds
               SET row_version = row_version + 1,
                   updated_by = $2,
                   updated_at = now()
               WHERE id = $1
               RETURNING row_version"#,
        )
        .bind(round_id)
        .bind(actor_user_id)
        .fetch_one(&mut *tx)
        .await?
    };
    tx.commit().await?;

    Ok(SyncExamSourcesResult {
        inserted_count,
        updated_duration_count,
        removed_count,
        round_row_version: next_row_version,
        results,
    })
}

fn source_sync_applied(change: &ExamSourceChange) -> ExamSourceSyncItemResult {
    ExamSourceSyncItemResult {
        source_id: change.source_id,
        change_kind: change.change_kind,
        status: ExamSourceSyncItemStatus::Applied,
        message: None,
    }
}

fn source_sync_conflict(change: &ExamSourceChange, message: String) -> ExamSourceSyncItemResult {
    ExamSourceSyncItemResult {
        source_id: change.source_id,
        change_kind: change.change_kind,
        status: ExamSourceSyncItemStatus::Conflict,
        message: Some(message),
    }
}

fn source_sync_validation_message(error: AppError) -> Result<String, AppError> {
    match error {
        AppError::ValidationError(message)
        | AppError::BadRequest(message)
        | AppError::Conflict(message) => Ok(message),
        error => Err(error),
    }
}

fn build_source_preview(
    round_id: Uuid,
    round_status: String,
    round_row_version: i64,
    rows: Vec<ExamSourceChangeRow>,
) -> Result<ExamSourcePreview, AppError> {
    let changes = rows
        .into_iter()
        .map(source_change_from_row)
        .collect::<Result<Vec<_>, _>>()?;
    let preview_token = source_preview_token(round_id, round_row_version, &changes);
    Ok(ExamSourcePreview {
        round_id,
        round_status,
        round_row_version,
        preview_token,
        new_count: changes
            .iter()
            .filter(|change| change.change_kind == ExamSourceChangeKind::New)
            .count() as i64,
        duration_changed_count: changes
            .iter()
            .filter(|change| change.change_kind == ExamSourceChangeKind::DurationChanged)
            .count() as i64,
        no_longer_eligible_count: changes
            .iter()
            .filter(|change| change.change_kind == ExamSourceChangeKind::NoLongerEligible)
            .count() as i64,
        changes,
    })
}

fn source_change_from_row(row: ExamSourceChangeRow) -> Result<ExamSourceChange, AppError> {
    let change_kind = match row.change_kind.as_str() {
        "new" => ExamSourceChangeKind::New,
        "duration_changed" => ExamSourceChangeKind::DurationChanged,
        "no_longer_eligible" => ExamSourceChangeKind::NoLongerEligible,
        unexpected => {
            tracing::error!(%unexpected, "invalid exam source change kind");
            return Err(AppError::InternalServerError(
                "Invalid exam source change kind".to_string(),
            ));
        }
    };
    Ok(ExamSourceChange {
        source_id: row.source_id,
        exam_schedule_item_id: row.exam_schedule_item_id,
        assessment_phase_id: row.assessment_phase_id,
        learning_group_id: row.learning_group_id,
        homeroom_id: row.homeroom_id,
        subject_id: row.subject_id,
        grade_level_id: row.grade_level_id,
        subject_code: row.subject_code,
        subject_name: row.subject_name,
        homeroom_name: row.homeroom_name,
        change_kind,
        snapshot_duration_minutes: row.snapshot_duration_minutes,
        current_duration_minutes: row.current_duration_minutes,
        scheduled: row.scheduled,
    })
}

fn source_preview_token(
    round_id: Uuid,
    round_row_version: i64,
    changes: &[ExamSourceChange],
) -> String {
    let mut digest = Sha256::new();
    digest.update(round_id.as_bytes());
    digest.update(round_row_version.to_be_bytes());
    for change in changes {
        digest.update(change.source_id.as_bytes());
        digest.update(match change.change_kind {
            ExamSourceChangeKind::New => b"new".as_slice(),
            ExamSourceChangeKind::DurationChanged => b"duration_changed".as_slice(),
            ExamSourceChangeKind::NoLongerEligible => b"no_longer_eligible".as_slice(),
        });
        digest.update(
            change
                .snapshot_duration_minutes
                .unwrap_or_default()
                .to_be_bytes(),
        );
        digest.update(
            change
                .current_duration_minutes
                .unwrap_or_default()
                .to_be_bytes(),
        );
        digest.update([u8::from(change.scheduled)]);
    }
    hex::encode(digest.finalize())
}

async fn fetch_source_change_rows(
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<ExamSourceChangeRow>, AppError> {
    sqlx::query_as(EXAM_SOURCE_PREVIEW_SQL)
        .bind(round_id)
        .fetch_all(pool)
        .await
        .map_err(AppError::from)
}

async fn fetch_source_change_rows_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    round_id: Uuid,
) -> Result<Vec<ExamSourceChangeRow>, AppError> {
    sqlx::query_as(EXAM_SOURCE_PREVIEW_SQL)
        .bind(round_id)
        .fetch_all(&mut **tx)
        .await
        .map_err(AppError::from)
}

pub(super) async fn count_source_changes_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    round_id: Uuid,
) -> Result<usize, AppError> {
    Ok(fetch_source_change_rows_in_tx(tx, round_id).await?.len())
}

async fn insert_exam_source_in_tx(
    tx: &mut Transaction<'_, Postgres>,
    round_id: Uuid,
    source_id: Uuid,
) -> Result<i64, AppError> {
    let result = sqlx::query(
        r#"INSERT INTO academic_exam_schedule_items (
               exam_round_id, academic_term_id, academic_year_id,
               assessment_phase_id, course_assessment_plan_id,
               learning_offering_id, learning_group_id, homeroom_id,
               subject_id, grade_level_id, duration_minutes
           )
           SELECT round.id, source.academic_term_id, source.academic_year_id,
                  source.assessment_phase_id, source.course_assessment_plan_id,
                  source.learning_offering_id, source.learning_group_id,
                  source.homeroom_id, source.subject_id, source.grade_level_id,
                  source.duration_minutes
           FROM academic_exam_rounds round
           JOIN academic_exam_eligible_sources source
             ON source.academic_term_id = round.academic_term_id
            AND source.academic_year_id = round.academic_year_id
            AND source.exam_kind = round.exam_kind
           WHERE round.id = $1 AND source.source_id = $2
           ON CONFLICT (exam_round_id, assessment_phase_id, learning_group_id, homeroom_id)
           DO NOTHING"#,
    )
    .bind(round_id)
    .bind(source_id)
    .execute(&mut **tx)
    .await?;
    Ok(result.rows_affected() as i64)
}

const EXAM_SOURCE_PREVIEW_SQL: &str = r#"
WITH round_context AS (
    SELECT id, academic_term_id, academic_year_id, exam_kind
    FROM academic_exam_rounds
    WHERE id = $1
), eligible AS (
    SELECT source.*
    FROM academic_exam_eligible_sources source
    JOIN round_context round
      ON round.academic_term_id = source.academic_term_id
     AND round.academic_year_id = source.academic_year_id
     AND round.exam_kind = source.exam_kind
), existing AS (
    SELECT uuid_generate_v5(
               '5c33b984-10df-58db-bf80-62dbc4a03d1b'::uuid,
               'exam-source:' || item.assessment_phase_id::text || ':'
                   || item.learning_group_id::text || ':' || item.homeroom_id::text
           ) AS source_id,
           item.*,
           EXISTS (
               SELECT 1 FROM academic_exam_sessions session
               WHERE session.exam_schedule_item_id = item.id
           ) AS scheduled
    FROM academic_exam_schedule_items item
    JOIN round_context round ON round.id = item.exam_round_id
), changes AS (
    SELECT eligible.source_id,
           NULL::uuid AS exam_schedule_item_id,
           eligible.assessment_phase_id,
           eligible.learning_group_id,
           eligible.homeroom_id,
           eligible.subject_id,
           eligible.grade_level_id,
           'new'::text AS change_kind,
           NULL::integer AS snapshot_duration_minutes,
           eligible.duration_minutes AS current_duration_minutes,
           false AS scheduled
    FROM eligible
    LEFT JOIN existing
      ON existing.assessment_phase_id = eligible.assessment_phase_id
     AND existing.learning_group_id = eligible.learning_group_id
     AND existing.homeroom_id = eligible.homeroom_id
    WHERE existing.id IS NULL
    UNION ALL
    SELECT existing.source_id,
           existing.id,
           existing.assessment_phase_id,
           existing.learning_group_id,
           existing.homeroom_id,
           existing.subject_id,
           existing.grade_level_id,
           'duration_changed'::text,
           existing.duration_minutes,
           eligible.duration_minutes,
           existing.scheduled
    FROM existing
    JOIN eligible
      ON eligible.assessment_phase_id = existing.assessment_phase_id
     AND eligible.learning_group_id = existing.learning_group_id
     AND eligible.homeroom_id = existing.homeroom_id
    WHERE existing.duration_minutes IS DISTINCT FROM eligible.duration_minutes
    UNION ALL
    SELECT existing.source_id,
           existing.id,
           existing.assessment_phase_id,
           existing.learning_group_id,
           existing.homeroom_id,
           existing.subject_id,
           existing.grade_level_id,
           'no_longer_eligible'::text,
           existing.duration_minutes,
           NULL::integer,
           existing.scheduled
    FROM existing
    LEFT JOIN eligible
      ON eligible.assessment_phase_id = existing.assessment_phase_id
     AND eligible.learning_group_id = existing.learning_group_id
     AND eligible.homeroom_id = existing.homeroom_id
    WHERE eligible.source_id IS NULL
)
SELECT changes.source_id,
       changes.exam_schedule_item_id,
       changes.assessment_phase_id,
       changes.learning_group_id,
       changes.homeroom_id,
       changes.subject_id,
       changes.grade_level_id,
       subject.code AS subject_code,
       coalesce(subject_version.name_th, subject_version.name_en, subject.code) AS subject_name,
       homeroom.name AS homeroom_name,
       changes.change_kind,
       changes.snapshot_duration_minutes,
       changes.current_duration_minutes,
       changes.scheduled
FROM changes
JOIN subjects subject ON subject.id = changes.subject_id
JOIN course_assessment_plans plan
  ON plan.id = (
      SELECT item.course_assessment_plan_id
      FROM academic_exam_schedule_items item
      WHERE item.id = changes.exam_schedule_item_id
      UNION ALL
      SELECT source.course_assessment_plan_id
      FROM eligible source
      WHERE source.source_id = changes.source_id
      LIMIT 1
  )
JOIN subject_versions subject_version ON subject_version.id = plan.subject_version_id
JOIN homerooms homeroom ON homeroom.id = changes.homeroom_id
ORDER BY changes.change_kind, subject.code, homeroom.name, changes.source_id
"#;

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
    pool: &PgPool,
    round_id: Uuid,
) -> Result<Vec<ExamScheduleItemView>, AppError> {
    sqlx::query_as::<_, ExamScheduleItemView>(
        r#"
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
               subject_version.group_id AS subject_group_id,
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
        FROM academic_exam_schedule_items item
        JOIN course_assessment_phases phase
          ON phase.id = item.assessment_phase_id
        JOIN learning_offerings offering ON offering.id = item.learning_offering_id
        JOIN course_offering_details course_detail
          ON course_detail.learning_offering_id = item.learning_offering_id
        JOIN subject_versions subject_version ON subject_version.id = course_detail.subject_version_id
        JOIN subjects subject ON subject.id = item.subject_id
        LEFT JOIN subject_groups subject_group ON subject_group.id = subject_version.group_id
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
               subject_version.group_id AS subject_group_id,
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
        LEFT JOIN subject_groups subject_group ON subject_group.id = subject_version.group_id
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
