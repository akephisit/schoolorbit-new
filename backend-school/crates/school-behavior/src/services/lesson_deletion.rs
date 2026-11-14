//! Proof that exact removed lesson facts can be reversed by the canonical day provider.
use chrono::NaiveDate;
use school_database::DbSession;
use school_errors::AppError;
use serde::Serialize;
use sqlx::{PgConnection, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LessonDeletionStudent {
    pub date: NaiveDate,
    pub student_id: Uuid,
}
#[derive(Debug, Clone)]
pub struct LessonDeletionScope {
    pub generation: Uuid,
    pub academic_term_id: Uuid,
    pub session_ids: Vec<Uuid>,
    pub affected_students: Vec<LessonDeletionStudent>,
    pub archived_student_ids: Vec<Uuid>,
}
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct LessonDeletionImpact {
    pub entries: i64,
    pub source_digest: String,
}

pub async fn preview_on_connection(
    connection: &mut PgConnection,
    scope: &LessonDeletionScope,
) -> Result<LessonDeletionImpact, AppError> {
    if scope.session_ids.len() > 200_000
        || scope.affected_students.len() > 200_000
        || scope.archived_student_ids.len() > 200_000
    {
        return Err(AppError::Conflict(
            "BEHAVIOR_LESSON_DELETION_TOO_LARGE".into(),
        ));
    }
    let selected = sqlx::types::Json(&scope.affected_students);
    let (unsafe_effects,entries,source_digest):(bool,i64,String)=DbSession::new(connection).query_as("behavior.lesson_deletion.proof",r#"
      WITH selected AS (SELECT date,"studentId" student_id FROM jsonb_to_recordset($4::jsonb) AS s(date date,"studentId" uuid)),
      totals AS (SELECT t.* FROM behavior_daily_totals t WHERE t.academic_term_id=$1 AND t.source='lesson' AND (
        EXISTS(SELECT 1 FROM selected s WHERE s.date=t.date AND s.student_id=t.student_id)
        OR t.student_id=ANY($5) OR EXISTS(SELECT 1 FROM behavior_entries e WHERE e.total_id=t.id AND(e.source_session_id=ANY($3) OR e.source_key=ANY(SELECT id::text FROM unnest($3::uuid[]) id))))),
      entries AS (SELECT e.* FROM behavior_entries e JOIN totals t ON t.id=e.total_id),
      decisions AS (SELECT d.* FROM behavior_source_effect_snapshots d WHERE d.academic_term_id=$1 AND d.kind='lesson' AND (
        d.source_key=ANY(SELECT id::text FROM unnest($3::uuid[]) id) OR EXISTS(SELECT 1 FROM selected s WHERE s.date=d.date AND s.student_id=d.student_id) OR d.student_id=ANY($5))),
      suppressions AS (SELECT s.* FROM behavior_suppressions s WHERE s.source='lesson' AND (s.academic_term_id IS NULL OR s.academic_term_id=$1)
        AND(s.academic_year_id IS NULL OR s.academic_year_id=(SELECT academic_year_id FROM academic_terms WHERE id=$1))),
      cycles AS (SELECT c.* FROM behavior_cycles c WHERE c.id IN(SELECT cycle_id FROM totals)),
      policies AS (SELECT p.* FROM behavior_source_days p WHERE p.academic_term_id=$1 AND p.generation=$2 AND p.date IN(SELECT date FROM selected)),
      evidence AS (
        SELECT 'total' kind,to_jsonb(t) value FROM totals t UNION ALL SELECT 'entry',to_jsonb(e) FROM entries e
        UNION ALL SELECT 'decision',to_jsonb(d) FROM decisions d UNION ALL SELECT 'suppression',to_jsonb(s) FROM suppressions s
        UNION ALL SELECT 'cycle',to_jsonb(c) FROM cycles c UNION ALL SELECT 'policy',to_jsonb(p) FROM policies p
      )
      SELECT EXISTS(SELECT 1 FROM totals t WHERE t.points>0 AND (
          t.details_purged OR t.student_id=ANY($5) OR t.points<>(SELECT COALESCE(sum(e.points),0) FROM entries e WHERE e.total_id=t.id AND e.cancelled_at IS NULL)))
        OR EXISTS(SELECT 1 FROM entries e JOIN totals t ON t.id=e.total_id JOIN cycles c ON c.id=t.cycle_id
          WHERE e.cancelled_at IS NULL AND (e.source_session_id=ANY($3) OR e.source_key=ANY(SELECT id::text FROM unnest($3::uuid[]) id))
          AND(e.source_generation<>$2 OR c.state<>'current' OR EXISTS(SELECT 1 FROM suppressions s WHERE s.side=t.side AND(s.start_date IS NULL OR t.date BETWEEN s.start_date AND s.end_date))))
        OR EXISTS(SELECT 1 FROM decisions d WHERE d.raw_points>0 AND d.source_key=ANY(SELECT id::text FROM unnest($3::uuid[]) id) AND(d.suppressed OR d.generation<>$2)) unsafe_effects,
        (SELECT count(*) FROM entries e WHERE e.cancelled_at IS NULL AND(e.source_session_id=ANY($3) OR e.source_key=ANY(SELECT id::text FROM unnest($3::uuid[]) id))) entries,
        encode(sha256(convert_to(COALESCE((SELECT jsonb_agg(value ORDER BY kind,value::text) FROM evidence),'[]')::text,'UTF8')),'hex') source_digest
    "#).bind(scope.academic_term_id).bind(scope.generation).bind(&scope.session_ids).bind(selected).bind(&scope.archived_student_ids).fetch_one().await?;
    if unsafe_effects {
        return Err(AppError::Conflict(
            "BEHAVIOR_LESSON_DELETION_PROTECTED_OR_UNATTRIBUTABLE".into(),
        ));
    }
    Ok(LessonDeletionImpact {
        entries,
        source_digest,
    })
}

/// Attendance module, canonical term and Attendance term locks precede this lock.
pub async fn require_removal_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    scope: &LessonDeletionScope,
) -> Result<LessonDeletionImpact, AppError> {
    super::lock(&mut **tx).await?;
    preview_on_connection(&mut **tx, scope).await
}
