use crate::models::*;
use chrono::{NaiveDate, NaiveTime};
use school_errors::AppError;
use sqlx::{types::Json, PgPool, Postgres, Transaction};
use uuid::Uuid;
#[derive(serde::Serialize)]
pub struct AttendanceNotice {
    pub student_id: Uuid,
    pub key: String,
    pub title: String,
    pub message: String,
    pub skip_if_key: Option<String>,
    pub daily_slot: Option<String>,
}
pub async fn queue_result(
    tx: &mut Transaction<'_, Postgres>,
    s: &AttendanceSession,
    student: Uuid,
    result: AttendanceResult,
    revision: i64,
    correction: bool,
) -> Result<(), AppError> {
    let notice = result_notice(s, student, result, revision, correction);
    if should_notify(tx, s).await? {
        queue_batch(tx, s.academic_term_id, &[notice]).await?;
    }
    Ok(())
}
pub fn result_notice(
    s: &AttendanceSession,
    student: Uuid,
    result: AttendanceResult,
    revision: i64,
    correction: bool,
) -> AttendanceNotice {
    let daily = matches!(s.kind, AttendanceKind::Arrival | AttendanceKind::Flag);
    let key = if correction {
        format!("correction:{}:{student}:{revision}", s.id)
    } else if s.kind == AttendanceKind::Arrival {
        format!("arrival:{}:{}:{student}", s.academic_term_id, s.date)
    } else if daily {
        format!("daily:{}:{}:{student}", s.academic_term_id, s.date)
    } else {
        format!("session:{}:{student}", s.id)
    };
    let title = if correction {
        "แก้ไขผลเช็คชื่อ"
    } else if s.kind == AttendanceKind::Arrival
        && matches!(result, AttendanceResult::Present | AttendanceResult::Late)
    {
        "เข้าโรงเรียนแล้ว"
    } else {
        "ผลเช็คชื่อ"
    };
    let message = if s.kind == AttendanceKind::Arrival
        && matches!(result, AttendanceResult::Present | AttendanceResult::Late)
    {
        format!(
            "เข้าโรงเรียนวันที่ {} เวลา {} ({})",
            s.date,
            s.start_time.format("%H:%M"),
            result.label()
        )
    } else {
        format!("{} วันที่ {}: {}", s.title, s.date, result.label())
    };
    AttendanceNotice {
        student_id: student,
        key,
        title: title.into(),
        message,
        skip_if_key: (s.kind == AttendanceKind::Flag && !correction)
            .then(|| format!("arrival:{}:{}:{student}", s.academic_term_id, s.date)),
        daily_slot: daily.then(|| format!("{}:{}:{student}", s.academic_term_id, s.date)),
    }
}
pub async fn should_notify(
    tx: &mut Transaction<'_, Postgres>,
    s: &AttendanceSession,
) -> Result<bool, AppError> {
    if s.kind != AttendanceKind::Special {
        return Ok(true);
    }
    Ok(sqlx::query_scalar("SELECT coalesce((t.definition->>'notify')::boolean,true) FROM attendance_special_rounds r JOIN attendance_special_templates t ON t.id=r.template_id WHERE r.id=$1").bind(s.special_round_id).fetch_one(&mut **tx).await?)
}
pub async fn queue_batch(
    tx: &mut Transaction<'_, Postgres>,
    term: Uuid,
    notices: &[AttendanceNotice],
) -> Result<(), AppError> {
    // Arrival and flag transactions have different session locks. Serialize their
    // initial recipient decision by student/day, using the same sorted lock order.
    if notices.iter().any(|notice| notice.daily_slot.is_some()) {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended(daily_slot,1711)) FROM (SELECT DISTINCT daily_slot FROM jsonb_to_recordset($1) AS n(daily_slot text) WHERE daily_slot IS NOT NULL ORDER BY daily_slot) slots")
            .bind(Json(notices)).execute(&mut **tx).await?;
    }
    sqlx::query(r#"WITH facts AS (SELECT * FROM jsonb_to_recordset($2) n(student_id uuid,key text,title text,message text,skip_if_key text)), recipients AS (
 SELECT f.*,u.id recipient_id,u.user_type FROM facts f JOIN users u ON u.id=f.student_id AND u.status='active'
 UNION SELECT f.*,u.id,u.user_type FROM facts f JOIN student_parents p ON p.student_user_id=f.student_id JOIN users u ON u.id=p.parent_user_id AND u.status='active' AND u.user_type='parent')
 INSERT INTO attendance_notifications(academic_term_id,recipient_id,student_id,dedup_key,title,message,link)
 SELECT $1,recipient_id,student_id,key,title,message,CASE WHEN user_type='parent' THEN '/parent/attendance?academicTermId=' ELSE '/student/attendance?academicTermId=' END || $1::text || '&studentId=' || student_id::text FROM recipients WHERE skip_if_key IS NULL OR NOT EXISTS(SELECT 1 FROM attendance_notifications previous WHERE previous.recipient_id=recipients.recipient_id AND previous.dedup_key=recipients.skip_if_key) ON CONFLICT(recipient_id,dedup_key) DO NOTHING"#).bind(term).bind(Json(notices)).execute(&mut **tx).await?;
    Ok(())
}
#[derive(sqlx::FromRow)]
pub struct PendingAttendanceNotification {
    pub id: Uuid,
    pub recipient_id: Uuid,
    pub notification_id: Uuid,
    pub title: String,
    pub message: String,
    pub link: String,
}
pub async fn pending(pool: &PgPool) -> Result<Vec<PendingAttendanceNotification>, AppError> {
    // One worker leases each row. A crash expires the lease without losing stored identity.
    sqlx::query("UPDATE attendance_notifications n SET published_at=now() WHERE published_at IS NULL AND (NOT EXISTS(SELECT 1 FROM users u WHERE u.id=n.recipient_id AND u.status='active') OR (n.student_id IS NOT NULL AND n.recipient_id<>n.student_id AND NOT EXISTS(SELECT 1 FROM student_parents p WHERE p.student_user_id=n.student_id AND p.parent_user_id=n.recipient_id)))").execute(pool).await?;
    Ok(sqlx::query_as(r#"WITH due AS (SELECT id FROM attendance_notifications WHERE published_at IS NULL AND next_attempt_at<=now() ORDER BY next_attempt_at,id LIMIT 100 FOR UPDATE SKIP LOCKED)
 UPDATE attendance_notifications n SET next_attempt_at=now()+interval '2 minutes',attempts=attempts+1 FROM due WHERE n.id=due.id RETURNING n.id,n.recipient_id,n.notification_id,n.title,n.message,n.link"#).fetch_all(pool).await?)
}
pub async fn store(pool: &PgPool, n: &PendingAttendanceNotification) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    sqlx::query("INSERT INTO notifications(id,user_id,title,message,type,link) VALUES($1,$2,$3,$4,'info',$5) ON CONFLICT(id) DO NOTHING").bind(n.notification_id).bind(n.recipient_id).bind(&n.title).bind(&n.message).bind(&n.link).execute(&mut *tx).await?;
    sqlx::query(
        "UPDATE attendance_notifications SET stored_at=coalesce(stored_at,now()) WHERE id=$1",
    )
    .bind(n.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}
pub async fn delivered(pool: &PgPool, id: Uuid) -> Result<(), AppError> {
    sqlx::query("UPDATE attendance_notifications SET published_at=now() WHERE id=$1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
pub async fn has_pending(pool: &PgPool) -> Result<bool, AppError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM attendance_notifications WHERE published_at IS NULL)",
    )
    .fetch_one(pool)
    .await?)
}
pub async fn queue_digest(
    pool: &PgPool,
    term: Uuid,
    date: NaiveDate,
    time: NaiveTime,
) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;
    sqlx::query(r#"WITH teacher_results AS (
 SELECT teacher,t.id,t.kind,t.saved_at,r.result FROM attendance_sessions t CROSS JOIN LATERAL unnest(t.teacher_ids) teacher LEFT JOIN attendance_records r ON r.session_id=t.id
 WHERE t.academic_term_id=$1 AND t.date=$2 AND NOT t.cancelled AND t.kind<>'arrival' AND t.start_time<=$3 AND coalesce(t.count_override,true)),
 summaries AS (SELECT teacher,kind,COUNT(*) FILTER(WHERE result='present') present,COUNT(*) FILTER(WHERE result='late') late,COUNT(*) FILTER(WHERE result='absent') absent,COUNT(*) FILTER(WHERE result='leave') leave,COUNT(*) FILTER(WHERE result='activity') activity,COUNT(*) FILTER(WHERE result='unchecked') unchecked,COUNT(DISTINCT id) FILTER(WHERE saved_at IS NULL) unrecorded FROM teacher_results GROUP BY teacher,kind),
 messages AS (SELECT teacher,string_agg((CASE kind WHEN 'flag' THEN 'หน้าเสาธง' WHEN 'lesson' THEN 'รายคาบ' ELSE 'รอบพิเศษ' END)||': มา '||present||' สาย '||late||' ขาด '||absent||' ลา '||leave||' กิจกรรม '||activity||' ยังไม่เช็ค '||unchecked||' ('||unrecorded||' รอบยังไม่บันทึก)',E'\n' ORDER BY kind) message FROM summaries GROUP BY teacher)
 INSERT INTO attendance_notifications(academic_term_id,recipient_id,dedup_key,title,message,link)
 SELECT $1,teacher,'digest:'||$1::text||':'||$2::text||':'||$3::text,'สรุปเช็คชื่อ '||$2::text,message,
 '/staff/attendance?academicTermId='||$1::text||'&date='||$2::text FROM messages JOIN users u ON u.id=teacher AND u.status='active' ON CONFLICT(recipient_id,dedup_key) DO NOTHING"#).bind(term).bind(date).bind(time).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}
