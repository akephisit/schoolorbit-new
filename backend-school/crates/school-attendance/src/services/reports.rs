use super::{audit, lock_term, settings};
use crate::{models::*, rules::invalid};
use school_database::DbSession;
use school_errors::AppError;
use school_file_platform::repository::SqlFileRepository;
use sqlx::{types::Json, PgConnection, PgPool};
use uuid::Uuid;
// Participants are immutable session metadata. Missing results are read-time unchecked coverage.
pub(crate) const SUMMARY_SQL: &str = r#"WITH eligible AS (
 SELECT p."studentId" student_id,COALESCE(r.result,'unchecked') result,s.kind,s.date,s.title,s.offering_id,
 special.template_id,template.definition->>'title' special_title,s.teacher_ids
 FROM attendance_sessions s CROSS JOIN LATERAL jsonb_to_recordset(s.roster) p("studentId" uuid)
 LEFT JOIN attendance_records r ON r.session_id=s.id AND r.student_id=p."studentId"
 LEFT JOIN attendance_special_rounds special ON special.id=s.special_round_id
 LEFT JOIN attendance_special_templates template ON template.id=special.template_id
 LEFT JOIN attendance_days d ON d.academic_term_id=s.academic_term_id AND d.date=s.date
 LEFT JOIN attendance_settings config ON config.academic_term_id=s.academic_term_id
 WHERE s.academic_term_id=$1 AND NOT s.cancelled AND s.date<=(now() AT TIME ZONE 'Asia/Bangkok')::date
 AND COALESCE(s.count_override,d.counted,COALESCE(config.configuration->'weekdays','[1,2,3,4,5]'::jsonb) @> to_jsonb(EXTRACT(ISODOW FROM s.date)::integer))
 AND ($2::uuid IS NULL OR p."studentId"=$2) AND ($3::uuid IS NULL OR $3=ANY(s.teacher_ids))),
 daily AS (SELECT DISTINCT ON(student_id,date) * FROM eligible WHERE kind='arrival'
 ORDER BY student_id,date,(result IN ('present','late')) DESC,(result<>'unchecked') DESC,result),
 facts AS (
 SELECT student_id,result,kind category,CASE WHEN kind='lesson' THEN COALESCE(offering_id::text,'') ELSE template_id::text END scope_key,
 CASE WHEN kind='special' THEN special_title ELSE title END scope_label,1::bigint weight FROM eligible WHERE kind<>'arrival'
 UNION ALL SELECT student_id,result,'school','','การเข้าโรงเรียน',1 FROM daily
 UNION ALL SELECT "studentId",'unchecked',category,"scopeKey","scopeLabel",expected
 FROM jsonb_to_recordset($4::jsonb) v("studentId" uuid,category text,"scopeKey" text,"scopeLabel" text,"teacherIds" uuid[],expected bigint)
 WHERE ($2::uuid IS NULL OR "studentId"=$2) AND ($3::uuid IS NULL OR $3=ANY("teacherIds")))
 SELECT $1::uuid academic_term_id,student_id,category,scope_key,min(scope_label) scope_label,
 COALESCE(sum(weight) FILTER(WHERE result='present'),0)::bigint present,
 COALESCE(sum(weight) FILTER(WHERE result='late'),0)::bigint late,
 COALESCE(sum(weight) FILTER(WHERE result='absent'),0)::bigint absent,
 COALESCE(sum(weight) FILTER(WHERE result='skipped'),0)::bigint skipped,
 COALESCE(sum(weight) FILTER(WHERE result='sick_leave'),0)::bigint sick_leave,
 COALESCE(sum(weight) FILTER(WHERE result='personal_leave'),0)::bigint personal_leave,
 COALESCE(sum(weight) FILTER(WHERE result='activity'),0)::bigint activity,
 COALESCE(sum(weight) FILTER(WHERE result='unchecked'),0)::bigint unchecked,sum(weight)::bigint expected,
 min(concat_ws(' ',concat(u.title,u.first_name),u.last_name)) display_name
 FROM facts JOIN users u ON u.id=facts.student_id GROUP BY student_id,category,scope_key ORDER BY student_id,category,scope_key"#;
fn archived_summary_sql(teacher: bool) -> &'static str {
    if teacher {
        "SELECT s.academic_term_id,s.student_id,s.category,s.scope_key,s.scope_label,s.present,s.late,s.absent,s.skipped,s.sick_leave,s.personal_leave,s.activity,s.unchecked,s.expected,concat_ws(' ',concat(u.title,u.first_name),u.last_name) display_name FROM attendance_term_teacher_summaries s JOIN users u ON u.id=s.student_id WHERE jsonb_typeof($4::jsonb)='array' AND s.academic_term_id=$1 AND ($2::uuid IS NULL OR s.student_id=$2) AND s.teacher_id=$3 ORDER BY s.student_id,s.category,s.scope_key"
    } else {
        "SELECT s.academic_term_id,s.student_id,s.category,s.scope_key,s.scope_label,s.present,s.late,s.absent,s.skipped,s.sick_leave,s.personal_leave,s.activity,s.unchecked,s.expected,concat_ws(' ',concat(u.title,u.first_name),u.last_name) display_name FROM attendance_term_summaries s JOIN users u ON u.id=s.student_id WHERE jsonb_typeof($4::jsonb)='array' AND s.academic_term_id=$1 AND ($2::uuid IS NULL OR s.student_id=$2) AND $3::uuid IS NULL ORDER BY s.student_id,s.category,s.scope_key"
    }
}
pub async fn summaries(
    pool: &PgPool,
    term: Uuid,
    student: Option<Uuid>,
    teacher: Option<Uuid>,
) -> Result<AttendanceReport, AppError> {
    summaries_with_expected(pool, term, student, teacher, &[]).await
}
pub async fn summaries_with_expected(
    pool: &PgPool,
    term: Uuid,
    student: Option<Uuid>,
    teacher: Option<Uuid>,
    expected: &[AttendanceExpectedSummary],
) -> Result<AttendanceReport, AppError> {
    summaries_with_expected_on_connection(
        &mut *school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.reports.read")
            .await?,
        term,
        student,
        teacher,
        expected,
    )
    .await
}
pub async fn summaries_with_expected_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    student: Option<Uuid>,
    teacher: Option<Uuid>,
    expected: &[AttendanceExpectedSummary],
) -> Result<AttendanceReport, AppError> {
    let settings = settings::get_on_connection(connection, term).await?;
    let summaries = if settings.archived {
        let sql = archived_summary_sql(teacher.is_some());
        DbSession::new(&mut *connection)
            .query_as(
                "attendance.services.reports.summaries_with_expected_on_connection",
                sql,
            )
            .bind(term)
            .bind(student)
            .bind(teacher)
            .bind(Json(expected))
            .fetch_all()
            .await?
    } else {
        DbSession::new(&mut *connection)
            .query_as(
                "attendance.services.reports.summaries_with_expected_on_connection",
                SUMMARY_SQL,
            )
            .bind(term)
            .bind(student)
            .bind(teacher)
            .bind(Json(expected))
            .fetch_all()
            .await?
    };
    Ok(AttendanceReport {
        page: 1,
        page_size: summaries.len() as i64,
        total: summaries.len() as i64,
        summaries,
        archived: settings.archived,
        activity_counts_as_present: settings.configuration.activity_counts_as_present,
    })
}
pub async fn page(
    pool: &PgPool,
    q: &AttendanceReportQuery,
    teacher: Option<Uuid>,
) -> Result<AttendanceReport, AppError> {
    page_with_expected(pool, q, teacher, &[]).await
}
pub async fn page_with_expected(
    pool: &PgPool,
    q: &AttendanceReportQuery,
    teacher: Option<Uuid>,
    expected: &[AttendanceExpectedSummary],
) -> Result<AttendanceReport, AppError> {
    page_with_expected_on_connection(
        &mut *school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.reports.read")
            .await?,
        q,
        teacher,
        expected,
    )
    .await
}
pub async fn page_with_expected_on_connection(
    connection: &mut PgConnection,
    q: &AttendanceReportQuery,
    teacher: Option<Uuid>,
    expected: &[AttendanceExpectedSummary],
) -> Result<AttendanceReport, AppError> {
    let context = settings::AttendanceReadContext::load(connection, q.academic_term_id).await?;
    page_with_context(connection, q, teacher, expected, &context).await
}
pub(crate) async fn page_with_context(
    connection: &mut PgConnection,
    q: &AttendanceReportQuery,
    teacher: Option<Uuid>,
    expected: &[AttendanceExpectedSummary],
    context: &settings::AttendanceReadContext,
) -> Result<AttendanceReport, AppError> {
    q.validate()?;
    let settings = &context.settings;
    let source = if settings.archived {
        archived_summary_sql(teacher.is_some())
    } else {
        SUMMARY_SQL
    };
    let sql = format!(
        r#"WITH summary AS ({source}), filtered AS (
        SELECT * FROM summary WHERE ($5='' OR strpos(lower(display_name),lower($5))>0 OR strpos(lower(scope_label),lower($5))>0)
            AND ($6::text IS NULL OR category=$6)
    ), page AS (SELECT academic_term_id AS "academicTermId", student_id AS "studentId", category AS "category", scope_key AS "scopeKey", scope_label AS "scopeLabel", display_name AS "displayName", present AS "present", late AS "late", absent AS "absent", skipped AS "skipped", sick_leave AS "sickLeave", personal_leave AS "personalLeave", activity AS "activity", unchecked AS "unchecked", expected AS "expected" FROM filtered ORDER BY student_id,category,scope_key LIMIT $7 OFFSET $8)
    SELECT COALESCE((SELECT jsonb_agg(to_jsonb(p) ORDER BY p."studentId",p.category,p."scopeKey") FROM page p),'[]'::jsonb), (SELECT count(*) FROM filtered)"#
    );
    let (Json(summaries), total): (Json<Vec<AttendanceSummary>>, i64) =
        DbSession::new(&mut *connection)
            .query_as(
                "attendance.services.reports.page_with_context",
                sqlx::AssertSqlSafe(sql),
            )
            .bind(q.academic_term_id)
            .bind(q.student_id)
            .bind(teacher)
            .bind(Json(expected))
            .bind(q.search.trim())
            .bind(q.category.as_deref())
            .bind(q.page_size)
            .bind((q.page - 1) * q.page_size)
            .fetch_one()
            .await?;
    Ok(AttendanceReport {
        summaries,
        total,
        page: q.page,
        page_size: q.page_size,
        archived: settings.archived,
        activity_counts_as_present: settings.configuration.activity_counts_as_present,
    })
}
pub async fn impact(pool: &PgPool, term: Uuid) -> Result<AttendancePurgeImpact, AppError> {
    impact_on_connection(
        &mut *school_tenancy::TenantPool::observed_checkout_pool(pool, "attendance.reports.read")
            .await?,
        term,
    )
    .await
}
pub async fn impact_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
) -> Result<AttendancePurgeImpact, AppError> {
    let (records,evidence,evidence_bytes):(i64,i64,i64)=DbSession::new(&mut *connection).query_as("attendance.services.reports.impact_on_connection", "SELECT (SELECT count(*) FROM attendance_records r JOIN attendance_sessions s ON s.id=r.session_id WHERE s.academic_term_id=$1),(SELECT count(*) FROM attendance_scan_events e JOIN attendance_sessions s ON s.id=e.session_id WHERE s.academic_term_id=$1),(SELECT COALESCE(sum(metadata.byte_size),0)::bigint FROM attendance_scan_events e JOIN attendance_sessions s ON s.id=e.session_id JOIN files f ON f.id=e.evidence_file_id LEFT JOIN LATERAL (SELECT v.byte_size FROM file_versions v WHERE v.file_id=f.id ORDER BY v.created_at DESC,v.id DESC LIMIT 1) metadata ON true WHERE s.academic_term_id=$1 AND f.lifecycle_status<>'deleted')").bind(term).fetch_one().await?;
    let archived = settings::get_on_connection(connection, term)
        .await?
        .archived;
    let closed: bool = DbSession::new(&mut *connection)
        .scalar(
            "attendance.services.reports.impact_on_connection",
            "SELECT status='closed' FROM academic_terms WHERE id=$1",
        )
        .bind(term)
        .fetch_optional()
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบภาคเรียน".into()))?;
    Ok(AttendancePurgeImpact {
        academic_term_id: term,
        records,
        evidence,
        evidence_bytes,
        archived,
        can_purge: closed && !archived,
    })
}
pub async fn purge(
    pool: &PgPool,
    actor: Uuid,
    term: Uuid,
    p: PurgeAttendanceTerm,
) -> Result<AttendancePurgeImpact, AppError> {
    purge_with_expected(pool, actor, term, p, &[]).await
}
pub async fn purge_with_expected(
    pool: &PgPool,
    actor: Uuid,
    term: Uuid,
    p: PurgeAttendanceTerm,
    coverage: &[AttendanceExpectedSummary],
) -> Result<AttendancePurgeImpact, AppError> {
    let tx =
        school_tenancy::TenantPool::begin_observed(pool, "attendance.reports.transaction").await?;
    purge_in_transaction(pool, tx, actor, term, p, coverage).await
}
pub async fn purge_in_transaction(
    pool: &PgPool,
    mut tx: school_tenancy::ObservedTransaction,
    actor: Uuid,
    term: Uuid,
    p: PurgeAttendanceTerm,
    coverage: &[AttendanceExpectedSummary],
) -> Result<AttendancePurgeImpact, AppError> {
    if p.reason.trim().is_empty() || p.reason.len() > 1000 {
        return Err(invalid("ระบุเหตุผลล้างรายละเอียด"));
    }
    lock_term(&mut tx, term, false, true).await?;
    let (closed,archived):(bool,bool)=DbSession::new(&mut *tx).query_as("attendance.services.reports.purge_in_transaction", "SELECT t.status='closed',EXISTS(SELECT 1 FROM attendance_term_archives a WHERE a.academic_term_id=t.id) FROM academic_terms t WHERE t.id=$1").bind(term).fetch_one().await?;
    if archived {
        return impact_on_connection(&mut *tx, term).await;
    }
    if !closed {
        return Err(AppError::Conflict("ล้างได้เฉพาะภาคเรียนที่ปิดแล้ว".into()));
    }
    super::score_changes::require_drained(&mut tx, term).await?;
    let notification_ids: Vec<Uuid> = DbSession::new(&mut *tx)
        .scalar(
            "attendance.services.reports.purge_in_transaction",
            "SELECT notification_id FROM attendance_notifications WHERE academic_term_id=$1",
        )
        .bind(term)
        .fetch_all()
        .await?;
    let progress =
        school_notifications::dispatch::progress_on_connection(&mut tx, &notification_ids).await?;
    let pending = progress
        .iter()
        .any(|n| n.stored_at.is_none() && n.published_at.is_none());
    if pending {
        return Err(AppError::Conflict("รอระบบเก็บแจ้งเตือนให้ครบก่อนล้าง".into()));
    }
    let count:i64=DbSession::new(&mut *tx).scalar("attendance.services.reports.purge_in_transaction", "SELECT count(*) FROM attendance_records r JOIN attendance_sessions s ON s.id=r.session_id WHERE s.academic_term_id=$1").bind(term).fetch_one().await?;
    if count != p.expected_records {
        return Err(AppError::Conflict(
            "จำนวนข้อมูลเปลี่ยนแล้ว กรุณาตรวจผลกระทบใหม่".into(),
        ));
    }
    let rows: Vec<AttendanceSummary> = DbSession::new(&mut *tx)
        .query_as(
            "attendance.services.reports.purge_in_transaction",
            SUMMARY_SQL,
        )
        .bind(term)
        .bind(Option::<Uuid>::None)
        .bind(Option::<Uuid>::None)
        .bind(Json(coverage))
        .fetch_all()
        .await?;
    let expected: i64 = rows.iter().map(|s| s.expected).sum();
    freeze(&mut tx, term, None, &rows).await?;
    let mut teachers: Vec<Uuid> = DbSession::new(&mut *tx).scalar("attendance.services.reports.purge_in_transaction",
        "SELECT DISTINCT unnest(teacher_ids) FROM attendance_sessions WHERE academic_term_id=$1",
    )
    .bind(term)
    .fetch_all()
    .await?;
    teachers.extend(coverage.iter().flat_map(|v| v.teacher_ids.iter()).copied());
    teachers.sort();
    teachers.dedup();
    for teacher in teachers {
        let scoped: Vec<AttendanceSummary> = DbSession::new(&mut *tx)
            .query_as(
                "attendance.services.reports.purge_in_transaction",
                SUMMARY_SQL,
            )
            .bind(term)
            .bind(Option::<Uuid>::None)
            .bind(teacher)
            .bind(Json(coverage))
            .fetch_all()
            .await?;
        freeze(&mut tx, term, Some(teacher), &scoped).await?;
    }
    // Reconcile every status bucket, including virtual unchecked coverage, before cleanup.
    let frozen: Vec<AttendanceSummary> = DbSession::new(&mut *tx)
        .query_as(
            "attendance.services.reports.purge_in_transaction",
            archived_summary_sql(false),
        )
        .bind(term)
        .bind(Option::<Uuid>::None)
        .bind(Option::<Uuid>::None)
        .bind(Json(coverage))
        .fetch_all()
        .await?;
    if frozen != rows {
        return Err(AppError::Conflict("ยอดสรุปไม่ตรง หยุดการล้างข้อมูล".into()));
    }
    let configuration: Option<Json<AttendanceConfiguration>> = DbSession::new(&mut *tx)
        .scalar(
            "attendance.services.reports.purge_in_transaction",
            "SELECT configuration FROM attendance_settings WHERE academic_term_id=$1",
        )
        .bind(term)
        .fetch_optional()
        .await?;
    DbSession::new(&mut *tx).query("attendance.services.reports.purge_in_transaction", "INSERT INTO attendance_term_archives(academic_term_id,archived_by,record_count,counted_record_count,settings_snapshot) VALUES($1,$2,$3,$4,$5)").bind(term).bind(actor).bind(count).bind(expected).bind(configuration.unwrap_or(Json(AttendanceConfiguration::default()))).execute().await?;
    let file_ids:Vec<Uuid>=DbSession::new(&mut *tx).scalar("attendance.services.reports.purge_in_transaction", "SELECT e.evidence_file_id FROM attendance_scan_events e JOIN attendance_sessions s ON s.id=e.session_id WHERE s.academic_term_id=$1 ORDER BY e.evidence_file_id").bind(term).fetch_all().await?;
    let repository = SqlFileRepository::new(pool.clone());
    for file in file_ids {
        repository
            .request_delete_in_transaction(&mut tx, file)
            .await
            .map_err(|_| AppError::ServiceUnavailable("เตรียมล้างภาพไม่ได้ จึงยังไม่ล้างผล".into()))?;
    }
    DbSession::new(&mut *tx).query("attendance.services.reports.purge_in_transaction", "DELETE FROM attendance_scan_events WHERE session_id IN(SELECT id FROM attendance_sessions WHERE academic_term_id=$1)").bind(term).execute().await?;
    DbSession::new(&mut *tx)
        .query(
            "attendance.services.reports.purge_in_transaction",
            "DELETE FROM attendance_sessions WHERE academic_term_id=$1",
        )
        .bind(term)
        .execute()
        .await?;
    let completed: Vec<Uuid> = progress
        .iter()
        .filter(|n| n.published_at.is_some())
        .map(|n| n.notification_id)
        .collect();
    DbSession::new(&mut *tx).query("attendance.services.reports.purge_in_transaction", "DELETE FROM attendance_notifications WHERE academic_term_id=$1 AND notification_id=ANY($2)").bind(term).bind(completed).execute().await?;
    audit(&mut tx, actor, term, "delete", &p.reason).await?;
    tx.commit().await?;
    impact(pool, term).await
}

async fn freeze(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    term: Uuid,
    teacher: Option<Uuid>,
    rows: &[AttendanceSummary],
) -> Result<(), AppError> {
    let source = " SELECT $1,student_id,category,scope_key,scope_label,present,late,absent,skipped,\"sickLeave\",\"personalLeave\",activity,unchecked,expected FROM jsonb_to_recordset($2) AS r(\"studentId\" uuid,category text,\"scopeKey\" text,\"scopeLabel\" text,present bigint,late bigint,absent bigint,skipped bigint,\"sickLeave\" bigint,\"personalLeave\" bigint,activity bigint,unchecked bigint,expected bigint)";
    let source = source.replace(
        "student_id,category,scope_key,scope_label",
        "\"studentId\",category,\"scopeKey\",\"scopeLabel\"",
    );
    let sql = if teacher.is_some() {
        format!(
            "INSERT INTO attendance_term_teacher_summaries(academic_term_id,student_id,category,scope_key,scope_label,present,late,absent,skipped,sick_leave,personal_leave,activity,unchecked,expected,teacher_id){}",
            source.replace("expected FROM", "expected,$3 FROM")
        )
    } else {
        format!(
            "INSERT INTO attendance_term_summaries(academic_term_id,student_id,category,scope_key,scope_label,present,late,absent,skipped,sick_leave,personal_leave,activity,unchecked,expected){source}"
        )
    };
    let query = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(term)
        .bind(Json(rows));
    if let Some(teacher) = teacher {
        DbSession::new(&mut **tx)
            .prepared_query("attendance.reports.freeze_teacher", query.bind(teacher))
            .execute()
            .await?;
    } else {
        DbSession::new(&mut **tx)
            .prepared_query("attendance.reports.freeze", query)
            .execute()
            .await?;
    }
    Ok(())
}
