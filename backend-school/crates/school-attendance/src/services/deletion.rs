//! Exact lesson cleanup for current Delivery cascades. Arrival and special facts
//! remain independent. The application owns Files and Behavior side effects.
use chrono::NaiveDate;
use school_academic_delivery::models::current::DeliveryDeletionScope;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{types::Json, PgConnection, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct AttendanceDeletionImpact {
    pub sessions: i64,
    pub records: i64,
    pub summaries: i64,
    pub evidence_files: i64,
    pub source_digest: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct AttendanceDeletionStudent {
    pub date: NaiveDate,
    pub student_id: Uuid,
}
#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct DeletionRosterStudent {
    student_id: Uuid,
    student_academic_year_id: Uuid,
    class_number: Option<i32>,
}

#[derive(Debug, Clone)]
pub struct AttendanceDeletionSources {
    pub generation: Uuid,
    pub affected_students: Vec<AttendanceDeletionStudent>,
    pub archived_student_ids: Vec<Uuid>,
    pub archived_summary_rows: i64,
    pub session_ids: Vec<Uuid>,
    pub evidence_file_ids: Vec<Uuid>,
    pub affected_dates: Vec<NaiveDate>,
}

/// Must precede the caller's canonical term lock. Finalizers, reset and scans
/// use this same module boundary; acquiring it after the term can deadlock.
pub async fn lock_before_term_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
) -> Result<(), AppError> {
    super::lock_module(tx).await
}

async fn require_bounded_scope_on_connection(
    connection: &mut PgConnection,
    scope: &DeliveryDeletionScope,
) -> Result<(), AppError> {
    let oversized:bool=DbSession::new(connection).scalar("attendance.deletion.source_bound", r#"
      WITH selected AS(SELECT id,roster FROM attendance_sessions WHERE academic_term_id=$1 AND kind='lesson' AND offering_id=$2 AND ($4 OR learning_group_id=ANY($3)))
      SELECT count(*)>200000 OR COALESCE(sum(CASE WHEN jsonb_typeof(roster)='array' THEN jsonb_array_length(roster) ELSE 200001 END),0)>200000
        OR (SELECT count(*) FROM attendance_records record JOIN selected session ON session.id=record.session_id)>200000
        OR (SELECT count(*) FROM attendance_scan_events scan JOIN selected session ON session.id=scan.session_id)>200000
        OR (SELECT count(*) FROM attendance_notifications notice WHERE academic_term_id=$1 AND split_part(dedup_key,':',1) IN('session','correction') AND split_part(dedup_key,':',2) IN(SELECT id::text FROM selected))>200000
        OR (SELECT count(*) FROM attendance_term_summaries WHERE academic_term_id=$1 AND category='lesson' AND scope_key=$2::text)>200000
        OR (SELECT count(*) FROM attendance_term_teacher_summaries WHERE academic_term_id=$1 AND category='lesson' AND scope_key=$2::text)>200000
      FROM selected
    "#).bind(scope.academic_term_id).bind(scope.learning_offering_id).bind(&scope.learning_group_ids).bind(scope.delete_offering).fetch_one().await?;
    if oversized {
        return Err(AppError::Conflict(
            "ATTENDANCE_DELETION_SOURCE_TOO_LARGE_OR_UNMAPPABLE".into(),
        ));
    }
    Ok(())
}

pub async fn deletion_sources_on_connection(
    connection: &mut PgConnection,
    scope: &DeliveryDeletionScope,
) -> Result<AttendanceDeletionSources, AppError> {
    require_bounded_scope_on_connection(connection, scope).await?;
    let rows: Vec<(Uuid,NaiveDate,Json<Vec<DeletionRosterStudent>>)> = DbSession::new(connection).query_as("attendance.deletion.sources", "SELECT id,date,roster FROM attendance_sessions WHERE academic_term_id=$1 AND kind='lesson' AND offering_id=$2 AND ($4 OR learning_group_id=ANY($3)) ORDER BY date,id LIMIT 200001").bind(scope.academic_term_id).bind(scope.learning_offering_id).bind(&scope.learning_group_ids).bind(scope.delete_offering).fetch_all().await.map_err(|error| match error { sqlx::Error::ColumnDecode {..} => AppError::Conflict("ATTENDANCE_DELETION_ROSTER_UNMAPPABLE".into()), error => AppError::DbError(error) })?;
    let mut affected_students = std::collections::BTreeSet::new();
    for (_, date, roster) in &rows {
        let mut seen = std::collections::HashSet::new();
        for student in &roster.0 {
            if student.student_id.is_nil()
                || student.student_academic_year_id.is_nil()
                || !seen.insert(student.student_id)
                || student.class_number.is_some_and(|number| number < 0)
            {
                return Err(AppError::Conflict(
                    "ATTENDANCE_DELETION_ROSTER_UNMAPPABLE".into(),
                ));
            }
            affected_students.insert(AttendanceDeletionStudent {
                date: *date,
                student_id: student.student_id,
            });
        }
    }
    let session_ids = rows.iter().map(|row| row.0).collect::<Vec<_>>();
    let records:Vec<(NaiveDate,Uuid)>=DbSession::new(connection).query_as("attendance.deletion.record_students", "SELECT DISTINCT session.date,record.student_id FROM attendance_records record JOIN attendance_sessions session ON session.id=record.session_id WHERE session.id=ANY($1) ORDER BY session.date,record.student_id LIMIT 200001").bind(&session_ids).fetch_all().await?;
    if records.len() > 200000 {
        return Err(AppError::Conflict(
            "ATTENDANCE_DELETION_SOURCE_TOO_LARGE_OR_UNMAPPABLE".into(),
        ));
    }
    affected_students.extend(
        records
            .into_iter()
            .map(|(date, student_id)| AttendanceDeletionStudent { date, student_id }),
    );
    let archived:Vec<Uuid>=DbSession::new(connection).scalar("attendance.deletion.archived_students", "SELECT student_id FROM attendance_term_summaries WHERE academic_term_id=$1 AND category='lesson' AND scope_key=$2 UNION SELECT student_id FROM attendance_term_teacher_summaries WHERE academic_term_id=$1 AND category='lesson' AND scope_key=$2 ORDER BY student_id LIMIT 200001").bind(scope.academic_term_id).bind(scope.learning_offering_id.to_string()).fetch_all().await?;
    if archived.len() > 200000 {
        return Err(AppError::Conflict(
            "ATTENDANCE_DELETION_SOURCE_TOO_LARGE_OR_UNMAPPABLE".into(),
        ));
    }
    let archived_summary_rows:i64=DbSession::new(connection).scalar("attendance.deletion.archived_summary_count", "SELECT (SELECT count(*) FROM attendance_term_summaries WHERE academic_term_id=$1 AND category='lesson' AND scope_key=$2)+(SELECT count(*) FROM attendance_term_teacher_summaries WHERE academic_term_id=$1 AND category='lesson' AND scope_key=$2)").bind(scope.academic_term_id).bind(scope.learning_offering_id.to_string()).fetch_one().await?;
    let generation = super::sessions::module_generation_on_connection(connection).await?;
    let evidence_file_ids = DbSession::new(connection).scalar("attendance.deletion.evidence", "SELECT DISTINCT scan.evidence_file_id FROM attendance_scan_events scan WHERE scan.session_id=ANY($1) ORDER BY scan.evidence_file_id").bind(&session_ids).fetch_all().await?;
    let mut affected_dates = rows.into_iter().map(|row| row.1).collect::<Vec<_>>();
    affected_dates.sort_unstable();
    affected_dates.dedup();
    Ok(AttendanceDeletionSources {
        generation,
        affected_students: affected_students.into_iter().collect(),
        archived_student_ids: archived,
        archived_summary_rows,
        session_ids,
        evidence_file_ids,
        affected_dates,
    })
}

pub async fn deletion_impact_on_connection(
    connection: &mut PgConnection,
    scope: &DeliveryDeletionScope,
) -> Result<AttendanceDeletionImpact, AppError> {
    require_bounded_scope_on_connection(connection, scope).await?;
    let impact: AttendanceDeletionImpact = DbSession::new(connection).query_as("attendance.deletion.impact", r#"
      WITH selected AS (SELECT * FROM attendance_sessions WHERE academic_term_id=$1 AND kind='lesson' AND offering_id=$2 AND ($4 OR learning_group_id=ANY($3))),
      summaries AS (
        SELECT 'student' kind,to_jsonb(s) value FROM attendance_term_summaries s WHERE academic_term_id=$1 AND category='lesson' AND scope_key=$2::text
        UNION ALL SELECT 'teacher',to_jsonb(s) FROM attendance_term_teacher_summaries s WHERE academic_term_id=$1 AND category='lesson' AND scope_key=$2::text
      ), evidence AS (
        SELECT 'session' kind,s.id::text identity,to_jsonb(s) value FROM selected s
        UNION ALL SELECT 'record',r.session_id::text||':'||r.student_id::text,to_jsonb(r) FROM attendance_records r JOIN selected s ON s.id=r.session_id
        UNION ALL SELECT 'scan',e.id::text,to_jsonb(e) FROM attendance_scan_events e JOIN selected s ON s.id=e.session_id
        UNION ALL SELECT kind,value->>'student_id',value FROM summaries
        UNION ALL SELECT 'notice',n.id::text,to_jsonb(n) FROM attendance_notifications n WHERE n.academic_term_id=$1 AND split_part(n.dedup_key,':',1) IN('session','correction') AND split_part(n.dedup_key,':',2) IN(SELECT id::text FROM selected)
      )
      SELECT (SELECT count(*) FROM selected) sessions,
        (SELECT count(*) FROM attendance_records r JOIN selected s ON s.id=r.session_id) records,
        (SELECT count(*) FROM summaries) summaries,
        (SELECT count(DISTINCT e.evidence_file_id) FROM attendance_scan_events e JOIN selected s ON s.id=e.session_id) evidence_files,
        encode(sha256(convert_to(COALESCE((SELECT jsonb_agg(value ORDER BY kind,identity,value::text) FROM evidence),'[]')::text,'UTF8')),'hex') source_digest
    "#).bind(scope.academic_term_id).bind(scope.learning_offering_id).bind(&scope.learning_group_ids).bind(scope.delete_offering).fetch_one().await?;
    require_mappable_summaries(scope.delete_offering, impact.summaries)?;
    Ok(impact)
}
#[derive(sqlx::FromRow)]
struct LessonNoticeIdentity {
    id: Uuid,
    notification_id: Uuid,
    recipient_id: Uuid,
    student_id: Option<Uuid>,
    dedup_key: String,
}
fn lesson_notice_matches(
    notice: &LessonNoticeIdentity,
    sessions: &std::collections::HashSet<Uuid>,
) -> bool {
    let parts = notice.dedup_key.split(':').collect::<Vec<_>>();
    let (kind, session, student) = match parts.as_slice() {
        ["session", session, student] => ("session", *session, *student),
        ["correction", session, student, revision]
            if revision
                .parse::<i64>()
                .is_ok_and(|value| value > 0 && value.to_string() == *revision) =>
        {
            ("correction", *session, *student)
        }
        _ => return false,
    };
    let canonical = |text: &str| {
        Uuid::parse_str(text)
            .ok()
            .filter(|id| id.to_string() == text)
    };
    let _kind = kind;
    canonical(session).is_some_and(|id| sessions.contains(&id))
        && canonical(student).is_some_and(|id| notice.student_id == Some(id))
}
fn require_mappable_summaries(whole_offering: bool, summaries: i64) -> Result<(), AppError> {
    if !whole_offering && summaries > 0 {
        return Err(AppError::Conflict(
            "ATTENDANCE_GROUP_ARCHIVE_SCOPE_UNMAPPABLE".into(),
        ));
    }
    Ok(())
}

/// Caller holds module-before-term admission and the exclusive term lifecycle
/// lock, has reconciled captured impact, and has prepared every evidence deletion
/// through Files. Returns exact day/session identities for synchronous Behavior
/// reconciliation on this transaction; the existing dirty-day queue also advances.
pub async fn delete_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    scope: &DeliveryDeletionScope,
) -> Result<AttendanceDeletionSources, AppError> {
    let sources = deletion_sources_on_connection(&mut **tx, scope).await?;
    require_mappable_summaries(scope.delete_offering, sources.archived_summary_rows)?;
    let notices: Vec<LessonNoticeIdentity> = DbSession::new(&mut **tx).query_as("attendance.deletion.notice_sources", "SELECT id,notification_id,recipient_id,student_id,dedup_key FROM attendance_notifications WHERE academic_term_id=$1 AND split_part(dedup_key,':',1) IN('session','correction') AND split_part(dedup_key,':',2)=ANY($2) ORDER BY id FOR UPDATE").bind(scope.academic_term_id).bind(sources.session_ids.iter().map(Uuid::to_string).collect::<Vec<_>>()).fetch_all().await?;
    if notices.len() > 200_000 {
        return Err(AppError::ValidationError(
            "การแจ้งเตือนของคาบที่ต้องลบเกินขนาดที่รองรับ".into(),
        ));
    }
    let selected = sources
        .session_ids
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let identities = notices
        .iter()
        .map(|notice| {
            if !lesson_notice_matches(notice, &selected) {
                return Err(AppError::Conflict(
                    "ATTENDANCE_NOTIFICATION_SOURCE_UNMAPPABLE".into(),
                ));
            }
            Ok(
                school_notifications::dispatch::PendingCancellationIdentity {
                    receipt_id: notice.id,
                    notification_id: notice.notification_id,
                    recipient_id: notice.recipient_id,
                    source: school_notifications::dispatch::NotificationSource::Attendance {
                        source_id: notice.id,
                    },
                },
            )
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    for batch in identities.chunks(school_notifications::dispatch::PENDING_CANCELLATION_BATCH_SIZE)
    {
        school_notifications::dispatch::cancel_pending_in_transaction(tx, batch).await?;
    }
    DbSession::new(&mut **tx)
        .query(
            "attendance.deletion.scans",
            "DELETE FROM attendance_scan_events WHERE session_id=ANY($1)",
        )
        .bind(&sources.session_ids)
        .execute()
        .await?;
    DbSession::new(&mut **tx)
        .query(
            "attendance.deletion.records",
            "DELETE FROM attendance_records WHERE session_id=ANY($1)",
        )
        .bind(&sources.session_ids)
        .execute()
        .await?;
    DbSession::new(&mut **tx).query("attendance.deletion.sessions", "DELETE FROM attendance_sessions WHERE id=ANY($1) AND academic_term_id=$2 AND kind='lesson'").bind(&sources.session_ids).bind(scope.academic_term_id).execute().await?;
    if scope.delete_offering {
        DbSession::new(&mut **tx).query("attendance.deletion.student_summaries", "DELETE FROM attendance_term_summaries WHERE academic_term_id=$1 AND category='lesson' AND scope_key=$2").bind(scope.academic_term_id).bind(scope.learning_offering_id.to_string()).execute().await?;
        DbSession::new(&mut **tx).query("attendance.deletion.teacher_summaries", "DELETE FROM attendance_term_teacher_summaries WHERE academic_term_id=$1 AND category='lesson' AND scope_key=$2").bind(scope.academic_term_id).bind(scope.learning_offering_id.to_string()).execute().await?;
    }
    super::score_changes::enqueue_dates(tx, scope.academic_term_id, &sources.affected_dates)
        .await?;
    Ok(sources)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn lesson_notice_identity_requires_exact_session_student_and_positive_revision() {
        let session = Uuid::from_u128(1);
        let student = Uuid::from_u128(2);
        let sessions = std::collections::HashSet::from([session]);
        let mut notice = LessonNoticeIdentity {
            id: Uuid::from_u128(3),
            notification_id: Uuid::from_u128(4),
            recipient_id: student,
            student_id: Some(student),
            dedup_key: format!("session:{session}:{student}"),
        };
        assert!(lesson_notice_matches(&notice, &sessions));
        notice.dedup_key = format!("correction:{session}:{student}:2");
        assert!(lesson_notice_matches(&notice, &sessions));
        for key in [
            format!("correction:{session}:{student}:0"),
            format!("correction:{session}:{student}:02"),
            format!("session:{session}:{student}:extra"),
            format!("arrival:{session}:{student}"),
        ] {
            notice.dedup_key = key;
            assert!(!lesson_notice_matches(&notice, &sessions));
        }
        notice.dedup_key = format!("session:{session}:{student}");
        notice.student_id = Some(Uuid::from_u128(9));
        assert!(!lesson_notice_matches(&notice, &sessions));
    }
    #[test]
    fn grouped_archives_require_exact_offering_scope() {
        assert!(require_mappable_summaries(true, 4).is_ok());
        assert!(require_mappable_summaries(false, 0).is_ok());
        assert!(require_mappable_summaries(false, 1).is_err());
    }
}
