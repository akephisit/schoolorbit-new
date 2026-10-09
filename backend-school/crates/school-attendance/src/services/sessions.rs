use super::{audit, lock_term, notifications, settings};
use crate::{
    models::*,
    policy,
    rules::{invalid, validate_inputs},
};
use chrono::Utc;
use school_authorization::ActorContext;
use school_errors::AppError;
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use uuid::Uuid;
pub const COLUMNS:&str="id,academic_term_id,date,kind,source_key,title,teacher_ids,homeroom_id,learning_group_id,offering_id,special_round_id,start_time,end_time,count_override,cancelled,cancellation_reason,saved_at,saved_by,row_version";
pub async fn get(pool: &PgPool, id: Uuid) -> Result<AttendanceSession, AppError> {
    let sql = format!("SELECT {COLUMNS} FROM attendance_sessions WHERE id=$1");
    sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรอบเช็คชื่อ".into()))
}
pub async fn list(
    pool: &PgPool,
    actor: &ActorContext,
    q: &AttendanceQuery,
) -> Result<Vec<AttendanceSession>, AppError> {
    let sql=format!("SELECT {COLUMNS} FROM attendance_sessions WHERE academic_term_id=$1 AND date=$2 AND ($3 OR $4=ANY(teacher_ids)) ORDER BY start_time,title,id");
    Ok(sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(q.academic_term_id)
        .bind(q.date)
        .bind(policy::read_school(actor))
        .bind(actor.user_id)
        .fetch_all(pool)
        .await?)
}
pub async fn ensure(
    pool: &PgPool,
    actor: &ActorContext,
    seed: AttendanceSeed,
) -> Result<AttendanceSession, AppError> {
    policy::require_session(actor, &seed.session, true)?;
    if seed.students.len() > 2000 {
        return Err(invalid("รอบเช็คชื่อเกิน 2000 คน กรุณาแบ่งกลุ่ม"));
    }
    let mut tx = pool.begin().await?;
    lock_term(&mut tx, seed.session.academic_term_id, true, false).await?;
    insert_seed(&mut tx, &seed).await?;
    tx.commit().await?;
    let s = &seed.session;
    let sql=format!("SELECT {COLUMNS} FROM attendance_sessions WHERE academic_term_id=$1 AND date=$2 AND kind=$3 AND source_key=$4");
    let found: AttendanceSession = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(s.academic_term_id)
        .bind(s.date)
        .bind(s.kind)
        .bind(&s.source_key)
        .fetch_one(pool)
        .await?;
    policy::require_session(actor, &found, true)?;
    Ok(found)
}
pub async fn insert_seed(
    tx: &mut Transaction<'_, Postgres>,
    seed: &AttendanceSeed,
) -> Result<(), AppError> {
    let s = &seed.session;
    let id:Option<Uuid>=sqlx::query_scalar("INSERT INTO attendance_sessions(id,academic_term_id,date,kind,source_key,title,teacher_ids,homeroom_id,learning_group_id,offering_id,special_round_id,start_time,end_time,count_override) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14) ON CONFLICT(academic_term_id,date,kind,source_key) DO NOTHING RETURNING id").bind(s.id).bind(s.academic_term_id).bind(s.date).bind(s.kind).bind(&s.source_key).bind(&s.title).bind(&s.teacher_ids).bind(s.homeroom_id).bind(s.learning_group_id).bind(s.offering_id).bind(s.special_round_id).bind(s.start_time).bind(s.end_time).bind(s.count_override).fetch_optional(&mut **tx).await?;
    if let Some(id) = id {
        let student_ids: Vec<_> = seed.students.iter().map(|s| s.student_id).collect();
        let years: Vec<_> = seed
            .students
            .iter()
            .map(|s| s.student_academic_year_id)
            .collect();
        let numbers: Vec<_> = seed.students.iter().map(|s| s.class_number).collect();
        sqlx::query("INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,special_round_id,class_number) SELECT $1,student,year,$2,number FROM unnest($3::uuid[],$4::uuid[],$5::integer[]) r(student,year,number)").bind(id).bind(s.special_round_id).bind(student_ids).bind(years).bind(numbers).execute(&mut **tx).await?;
    }
    Ok(())
}
pub async fn detail(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
) -> Result<AttendanceDetail, AppError> {
    let s = get(pool, id).await?;
    policy::require_session(actor, &s, false)?;
    detail_unchecked(pool, s).await
}
pub async fn detail_unchecked(
    pool: &PgPool,
    s: AttendanceSession,
) -> Result<AttendanceDetail, AppError> {
    let settings = settings::get(pool, s.academic_term_id).await?;
    let counted = s.count_override.unwrap_or(
        settings::is_counted(pool, s.academic_term_id, s.date, &settings.configuration).await?,
    ) && !s.cancelled;
    let students=sqlx::query_as(r#"SELECT r.student_id,r.student_academic_year_id,concat_ws(' ',u.title,u.first_name,u.last_name) display_name,r.class_number,r.result,r.origin,r.note,r.observed_at,e.evidence_file_id,
 (SELECT min(a.observed_at) FROM attendance_records a JOIN attendance_sessions s ON s.id=a.session_id WHERE a.student_id=r.student_id AND s.academic_term_id=$2 AND s.date=$3 AND s.kind='arrival') arrival_at,r.row_version
 FROM attendance_records r JOIN users u ON u.id=r.student_id LEFT JOIN attendance_scan_events e ON e.session_id=r.session_id AND e.student_id=r.student_id WHERE r.session_id=$1 ORDER BY r.class_number NULLS LAST,u.first_name COLLATE schoolorbit_thai,u.last_name COLLATE schoolorbit_thai,u.id"#).bind(s.id).bind(s.academic_term_id).bind(s.date).fetch_all(pool).await?;
    let term_writable:bool=sqlx::query_scalar("SELECT t.status NOT IN ('closed','cancelled') AND y.status NOT IN ('closed','archived') FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id WHERE t.id=$1").bind(s.academic_term_id).fetch_one(pool).await?;
    Ok(AttendanceDetail {
        writable: term_writable && !settings.archived && !s.cancelled,
        session: s,
        students,
        counted,
    })
}
async fn lock_session(
    tx: &mut Transaction<'_, Postgres>,
    id: Uuid,
    version: i64,
) -> Result<AttendanceSession, AppError> {
    let sql = format!("SELECT {COLUMNS} FROM attendance_sessions WHERE id=$1 FOR UPDATE");
    let s: AttendanceSession = sqlx::query_as(sqlx::AssertSqlSafe(sql))
        .bind(id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| AppError::NotFound("ไม่พบรอบเช็คชื่อ".into()))?;
    if s.row_version != version {
        return Err(AppError::Conflict(
            "มีการบันทึกหรือสแกนเพิ่มแล้ว กรุณาโหลดรายชื่อใหม่".into(),
        ));
    }
    Ok(s)
}
pub async fn save(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    p: SaveAttendanceResults,
) -> Result<AttendanceDetail, AppError> {
    let s = get(pool, id).await?;
    policy::require_session(actor, &s, true)?;
    if s.date
        > Utc::now()
            .with_timezone(&chrono_tz::Asia::Bangkok)
            .date_naive()
    {
        return Err(invalid("ยังบันทึกผลของวันในอนาคตไม่ได้"));
    }
    let mut tx = pool.begin().await?;
    lock_term(&mut tx, s.academic_term_id, true, false).await?;
    let s = lock_session(&mut tx, id, p.row_version).await?;
    policy::require_session(actor, &s, true)?;
    if s.cancelled {
        return Err(AppError::Conflict("รอบนี้งดเช็คชื่อ".into()));
    }
    if p.reason.len() > 1000 || (s.saved_at.is_some() && p.reason.trim().is_empty()) {
        return Err(invalid("กรุณาระบุเหตุผลเมื่อแก้ไขผลที่บันทึกแล้ว"));
    }
    let roster:Vec<Uuid>=sqlx::query_scalar("SELECT student_id FROM attendance_records WHERE session_id=$1 ORDER BY student_id FOR UPDATE").bind(id).fetch_all(&mut *tx).await?;
    validate_inputs(&p.students, &roster)?;
    let before: Vec<(Uuid, AttendanceResult, String)> =
        sqlx::query_as("SELECT student_id,result,note FROM attendance_records WHERE session_id=$1")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
    #[derive(serde::Serialize)]
    struct Input {
        student_id: Uuid,
        result: String,
        note: String,
    }
    let inputs: Vec<_> = p
        .students
        .iter()
        .map(|r| Input {
            student_id: r.student_id,
            result: r.result.code().into(),
            note: r.note.clone(),
        })
        .collect();
    sqlx::query(r#"WITH input AS (SELECT * FROM jsonb_to_recordset($2) AS i(student_id uuid,result text,note text)),desired AS (
 SELECT r.student_id,CASE WHEN COALESCE(i.result,r.result)='unchecked' THEN 'absent' ELSE COALESCE(i.result,r.result) END result,
 CASE WHEN COALESCE(i.result,r.result)='unchecked' THEN 'inferred' WHEN i.student_id IS NOT NULL AND (i.result<>r.result OR i.note<>r.note) THEN 'teacher' ELSE r.origin END origin,
 COALESCE(i.note,r.note) note FROM attendance_records r LEFT JOIN input i ON i.student_id=r.student_id WHERE r.session_id=$1)
 UPDATE attendance_records r SET result=d.result,origin=d.origin,note=d.note,updated_by=$3,row_version=r.row_version+1 FROM desired d WHERE r.session_id=$1 AND r.student_id=d.student_id AND (r.result,r.origin,r.note) IS DISTINCT FROM (d.result,d.origin,d.note)"#).bind(id).bind(Json(inputs)).bind(actor.user_id).execute(&mut *tx).await?;
    let after: Vec<(Uuid, AttendanceResult, String, i64)> = sqlx::query_as(
        "SELECT student_id,result,note,row_version FROM attendance_records WHERE session_id=$1",
    )
    .bind(id)
    .fetch_all(&mut *tx)
    .await?;
    let old: std::collections::HashMap<_, _> = before.iter().map(|r| (r.0, (&r.1, &r.2))).collect();
    let notices: Vec<_> = after
        .iter()
        .filter(|(student, result, note, _)| {
            s.saved_at.is_none()
                || old
                    .get(student)
                    .is_some_and(|(r, n)| **r != *result || **n != *note)
        })
        .map(|(student, result, _, revision)| {
            notifications::result_notice(&s, *student, *result, *revision, s.saved_at.is_some())
        })
        .collect();
    if notifications::should_notify(&mut tx, &s).await? {
        notifications::queue_batch(&mut tx, s.academic_term_id, &notices).await?;
    }

    sqlx::query("UPDATE attendance_sessions SET saved_at=now(),saved_by=$2,row_version=row_version+1 WHERE id=$1").bind(id).bind(actor.user_id).execute(&mut *tx).await?;
    #[derive(serde::Serialize)]
    struct Change<'a> {
        students: &'a Vec<(Uuid, AttendanceResult, String)>,
        reason: &'a str,
    }
    sqlx::query("INSERT INTO audit_logs(user_id,action,entity_type,entity_id,old_values,new_values,description) VALUES($1,'update','attendance',$2,$3,$4,$5)").bind(actor.user_id).bind(id).bind(Json(Change{students:&before,reason:&p.reason})).bind(Json(&after)).bind(&p.reason).execute(&mut *tx).await?;
    tx.commit().await?;
    detail(pool, actor, id).await
}
pub async fn cancel(
    pool: &PgPool,
    actor: &ActorContext,
    id: Uuid,
    p: AttendanceCancellation,
) -> Result<AttendanceDetail, AppError> {
    let s = get(pool, id).await?;
    policy::require_session(actor, &s, true)?;
    if p.reason.trim().is_empty() || p.reason.len() > 1000 {
        return Err(invalid("ระบุเหตุผลงดหรือคืนรอบเช็คชื่อ"));
    }
    let mut tx = pool.begin().await?;
    lock_term(&mut tx, s.academic_term_id, true, false).await?;
    let s = lock_session(&mut tx, id, p.row_version).await?;
    policy::require_session(actor, &s, true)?;
    sqlx::query("UPDATE attendance_sessions SET cancelled=$2,cancellation_reason=$3,row_version=row_version+1 WHERE id=$1").bind(id).bind(p.cancelled).bind(&p.reason).execute(&mut *tx).await?;
    audit(&mut tx, actor.user_id, id, "update", &p.reason).await?;
    tx.commit().await?;
    detail(pool, actor, id).await
}
