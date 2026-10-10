use super::{audit, lock_term, settings};
use crate::{models::*, rules::invalid};
use school_errors::AppError;
use school_file_platform::repository::SqlFileRepository;
use sqlx::{types::Json, PgPool};
use uuid::Uuid;
// Unchecked facts remain visible in coverage. A positive arrival proves school presence,
// even after a missed flag ceremony; ceremony and lessons retain independent units.
const SUMMARY_SQL: &str = r#"WITH eligible AS (
 SELECT r.*,s.kind,s.date,s.title,s.offering_id,s.source_key,special.template_id,template.definition->>'title' special_title FROM attendance_records r JOIN attendance_sessions s ON s.id=r.session_id
 LEFT JOIN attendance_special_rounds special ON special.id=s.special_round_id LEFT JOIN attendance_special_templates template ON template.id=special.template_id
 LEFT JOIN attendance_days d ON d.academic_term_id=s.academic_term_id AND d.date=s.date
 LEFT JOIN attendance_settings config ON config.academic_term_id=s.academic_term_id
 WHERE s.academic_term_id=$1 AND NOT s.cancelled AND COALESCE(s.count_override,d.counted,
 COALESCE(config.configuration->'weekdays','[1,2,3,4,5]'::jsonb) @> to_jsonb(EXTRACT(ISODOW FROM s.date)::integer)) AND ($2::uuid IS NULL OR r.student_id=$2) AND ($3::uuid IS NULL OR $3=ANY(s.teacher_ids))),
 daily AS (SELECT DISTINCT ON(student_id,date) * FROM eligible WHERE kind IN ('flag','arrival') ORDER BY student_id,date,(kind='arrival' AND result IN ('present','late')) DESC,(result<>'unchecked') DESC,(kind='flag') DESC),
 facts AS (SELECT student_id,result,kind category,CASE WHEN kind='lesson' THEN offering_id::text WHEN kind='special' THEN template_id::text ELSE '' END scope_key,CASE WHEN kind='special' THEN special_title WHEN kind='lesson' THEN title ELSE kind END scope_label FROM eligible WHERE kind<>'arrival'
 UNION ALL SELECT student_id,result,'school','', 'การมาโรงเรียน' FROM daily)
 SELECT $1::uuid academic_term_id,student_id,category,scope_key,min(scope_label) scope_label,
 COUNT(*) FILTER(WHERE result='present') present,COUNT(*) FILTER(WHERE result='late') late,COUNT(*) FILTER(WHERE result='absent') absent,COUNT(*) FILTER(WHERE result='leave') leave,COUNT(*) FILTER(WHERE result='activity') activity,COUNT(*) FILTER(WHERE result='unchecked') unchecked,COUNT(*) expected,min(concat_ws(' ',u.title,u.first_name,u.last_name)) display_name
 FROM facts JOIN users u ON u.id=facts.student_id GROUP BY student_id,category,scope_key ORDER BY student_id,category,scope_key"#;
fn archived_summary_sql(teacher: bool) -> &'static str {
    if teacher {
        "SELECT s.academic_term_id,s.student_id,s.category,s.scope_key,s.scope_label,s.present,s.late,s.absent,s.leave,s.activity,s.unchecked,s.expected,concat_ws(' ',u.title,u.first_name,u.last_name) display_name FROM attendance_term_teacher_summaries s JOIN users u ON u.id=s.student_id WHERE s.academic_term_id=$1 AND ($2::uuid IS NULL OR s.student_id=$2) AND s.teacher_id=$3 ORDER BY s.student_id,s.category,s.scope_key"
    } else {
        "SELECT s.academic_term_id,s.student_id,s.category,s.scope_key,s.scope_label,s.present,s.late,s.absent,s.leave,s.activity,s.unchecked,s.expected,concat_ws(' ',u.title,u.first_name,u.last_name) display_name FROM attendance_term_summaries s JOIN users u ON u.id=s.student_id WHERE s.academic_term_id=$1 AND ($2::uuid IS NULL OR s.student_id=$2) AND $3::uuid IS NULL ORDER BY s.student_id,s.category,s.scope_key"
    }
}
pub async fn summaries(
    pool: &PgPool,
    term: Uuid,
    student: Option<Uuid>,
    teacher: Option<Uuid>,
) -> Result<AttendanceReport, AppError> {
    let settings = settings::get(pool, term).await?;
    let summaries = if settings.archived {
        let sql = archived_summary_sql(teacher.is_some());
        sqlx::query_as(sql)
            .bind(term)
            .bind(student)
            .bind(teacher)
            .fetch_all(pool)
            .await?
    } else {
        sqlx::query_as(SUMMARY_SQL)
            .bind(term)
            .bind(student)
            .bind(teacher)
            .fetch_all(pool)
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
    q.validate()?;
    let settings = settings::get(pool, q.academic_term_id).await?;
    let source = if settings.archived {
        archived_summary_sql(teacher.is_some())
    } else {
        SUMMARY_SQL
    };
    let sql = format!(
        r#"WITH summary AS ({source}), filtered AS (
        SELECT * FROM summary WHERE ($4='' OR strpos(lower(display_name),lower($4))>0 OR strpos(lower(scope_label),lower($4))>0)
            AND ($5::text IS NULL OR category=$5)
    ), page AS (SELECT academic_term_id AS "academicTermId", student_id AS "studentId", category AS "category", scope_key AS "scopeKey", scope_label AS "scopeLabel", display_name AS "displayName", present AS "present", late AS "late", absent AS "absent", leave AS "leave", activity AS "activity", unchecked AS "unchecked", expected AS "expected" FROM filtered ORDER BY student_id,category,scope_key LIMIT $6 OFFSET $7)
    SELECT COALESCE((SELECT jsonb_agg(to_jsonb(p) ORDER BY p."studentId",p.category,p."scopeKey") FROM page p),'[]'::jsonb), (SELECT count(*) FROM filtered)"#
    );
    let (Json(summaries), total): (Json<Vec<AttendanceSummary>>, i64) =
        sqlx::query_as(sqlx::AssertSqlSafe(sql))
            .bind(q.academic_term_id)
            .bind(q.student_id)
            .bind(teacher)
            .bind(q.search.trim())
            .bind(q.category.as_deref())
            .bind(q.page_size)
            .bind((q.page - 1) * q.page_size)
            .fetch_one(pool)
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
    let (records,evidence,evidence_bytes):(i64,i64,i64)=sqlx::query_as("SELECT (SELECT count(*) FROM attendance_records r JOIN attendance_sessions s ON s.id=r.session_id WHERE s.academic_term_id=$1),(SELECT count(*) FROM attendance_scan_events e JOIN attendance_sessions s ON s.id=e.session_id WHERE s.academic_term_id=$1),(SELECT COALESCE(sum(metadata.byte_size),0)::bigint FROM attendance_scan_events e JOIN attendance_sessions s ON s.id=e.session_id JOIN files f ON f.id=e.evidence_file_id LEFT JOIN LATERAL (SELECT v.byte_size FROM file_versions v WHERE v.file_id=f.id ORDER BY v.created_at DESC,v.id DESC LIMIT 1) metadata ON true WHERE s.academic_term_id=$1 AND f.lifecycle_status<>'deleted')").bind(term).fetch_one(pool).await?;
    let archived = settings::get(pool, term).await?.archived;
    let closed: bool = sqlx::query_scalar("SELECT status='closed' FROM academic_terms WHERE id=$1")
        .bind(term)
        .fetch_optional(pool)
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
    if p.reason.trim().is_empty() || p.reason.len() > 1000 {
        return Err(invalid("ระบุเหตุผลล้างรายละเอียด"));
    }
    let mut tx = pool.begin().await?;
    lock_term(&mut tx, term, false, true).await?;
    let (closed,archived):(bool,bool)=sqlx::query_as("SELECT t.status='closed',EXISTS(SELECT 1 FROM attendance_term_archives a WHERE a.academic_term_id=t.id) FROM academic_terms t WHERE t.id=$1").bind(term).fetch_one(&mut *tx).await?;
    if archived {
        return impact(pool, term).await;
    }
    if !closed {
        return Err(AppError::Conflict("ล้างได้เฉพาะภาคเรียนที่ปิดแล้ว".into()));
    }
    let pending:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM attendance_notifications WHERE academic_term_id=$1 AND stored_at IS NULL AND published_at IS NULL)").bind(term).fetch_one(&mut *tx).await?;
    if pending {
        return Err(AppError::Conflict("รอระบบเก็บแจ้งเตือนให้ครบก่อนล้าง".into()));
    }
    let count:i64=sqlx::query_scalar("SELECT count(*) FROM attendance_records r JOIN attendance_sessions s ON s.id=r.session_id WHERE s.academic_term_id=$1").bind(term).fetch_one(&mut *tx).await?;
    if count != p.expected_records {
        return Err(AppError::Conflict(
            "จำนวนข้อมูลเปลี่ยนแล้ว กรุณาตรวจผลกระทบใหม่".into(),
        ));
    }
    let rows: Vec<AttendanceSummary> = sqlx::query_as(SUMMARY_SQL)
        .bind(term)
        .bind(Option::<Uuid>::None)
        .bind(Option::<Uuid>::None)
        .fetch_all(&mut *tx)
        .await?;
    let expected: i64 = rows
        .iter()
        .filter(|s| s.category != "school")
        .map(|s| s.expected)
        .sum();
    freeze(&mut tx, term, None, &rows).await?;
    let teachers: Vec<Uuid> = sqlx::query_scalar(
        "SELECT DISTINCT unnest(teacher_ids) FROM attendance_sessions WHERE academic_term_id=$1",
    )
    .bind(term)
    .fetch_all(&mut *tx)
    .await?;
    for teacher in teachers {
        let scoped: Vec<AttendanceSummary> = sqlx::query_as(SUMMARY_SQL)
            .bind(term)
            .bind(Option::<Uuid>::None)
            .bind(teacher)
            .fetch_all(&mut *tx)
            .await?;
        freeze(&mut tx, term, Some(teacher), &scoped).await?;
    }
    // Compare against the detailed source independently before deleting any facts.
    let source:i64=sqlx::query_scalar("SELECT count(*) FROM attendance_records r JOIN attendance_sessions s ON s.id=r.session_id LEFT JOIN attendance_days d ON d.academic_term_id=s.academic_term_id AND d.date=s.date LEFT JOIN attendance_settings c ON c.academic_term_id=s.academic_term_id WHERE s.academic_term_id=$1 AND s.kind<>'arrival' AND NOT s.cancelled AND COALESCE(s.count_override,d.counted,COALESCE(c.configuration->'weekdays','[1,2,3,4,5]'::jsonb) @> to_jsonb(EXTRACT(ISODOW FROM s.date)::integer))").bind(term).fetch_one(&mut *tx).await?;
    if source != expected {
        return Err(AppError::Conflict(
            "ยอดรายละเอียดไม่ตรงกับสรุป หยุดการล้างข้อมูล".into(),
        ));
    }

    let frozen:i64=sqlx::query_scalar("SELECT COALESCE(sum(expected),0)::bigint FROM attendance_term_summaries WHERE academic_term_id=$1 AND category<>'school'").bind(term).fetch_one(&mut *tx).await?;
    if frozen != expected {
        return Err(AppError::Conflict("ยอดสรุปไม่ตรง หยุดการล้างข้อมูล".into()));
    }
    let configuration: Option<Json<AttendanceConfiguration>> = sqlx::query_scalar(
        "SELECT configuration FROM attendance_settings WHERE academic_term_id=$1",
    )
    .bind(term)
    .fetch_optional(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO attendance_term_archives(academic_term_id,archived_by,record_count,counted_record_count,settings_snapshot) VALUES($1,$2,$3,$4,$5)").bind(term).bind(actor).bind(count).bind(expected).bind(configuration.unwrap_or(Json(AttendanceConfiguration::default()))).execute(&mut *tx).await?;
    let file_ids:Vec<Uuid>=sqlx::query_scalar("SELECT e.evidence_file_id FROM attendance_scan_events e JOIN attendance_sessions s ON s.id=e.session_id WHERE s.academic_term_id=$1 ORDER BY e.evidence_file_id").bind(term).fetch_all(&mut *tx).await?;
    let repository = SqlFileRepository::new(pool.clone());
    for file in file_ids {
        repository
            .request_delete_in_transaction(&mut tx, file)
            .await
            .map_err(|_| AppError::ServiceUnavailable("เตรียมล้างภาพไม่ได้ จึงยังไม่ล้างผล".into()))?;
    }
    sqlx::query("DELETE FROM attendance_scan_events WHERE session_id IN(SELECT id FROM attendance_sessions WHERE academic_term_id=$1)").bind(term).execute(&mut *tx).await?;
    sqlx::query("DELETE FROM attendance_sessions WHERE academic_term_id=$1")
        .bind(term)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM attendance_notifications WHERE academic_term_id=$1 AND published_at IS NOT NULL").bind(term).execute(&mut *tx).await?;
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
    let source=" SELECT $1,student_id,category,scope_key,scope_label,present,late,absent,leave,activity,unchecked,expected FROM jsonb_to_recordset($2) AS r(\"studentId\" uuid,category text,\"scopeKey\" text,\"scopeLabel\" text,present bigint,late bigint,absent bigint,leave bigint,activity bigint,unchecked bigint,expected bigint)";
    let source = source.replace(
        "student_id,category,scope_key,scope_label",
        "\"studentId\",category,\"scopeKey\",\"scopeLabel\"",
    );
    let sql = if teacher.is_some() {
        format!("INSERT INTO attendance_term_teacher_summaries(academic_term_id,student_id,category,scope_key,scope_label,present,late,absent,leave,activity,unchecked,expected,teacher_id){}",source.replace("expected FROM","expected,$3 FROM"))
    } else {
        format!("INSERT INTO attendance_term_summaries(academic_term_id,student_id,category,scope_key,scope_label,present,late,absent,leave,activity,unchecked,expected){source}")
    };
    let query = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(term)
        .bind(Json(rows));
    if let Some(teacher) = teacher {
        query.bind(teacher).execute(&mut **tx).await?;
    } else {
        query.execute(&mut **tx).await?;
    }
    Ok(())
}
