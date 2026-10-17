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
    let daily = s.kind == AttendanceKind::Arrival;
    let key = if correction {
        format!("correction:{}:{student}:{revision}", s.id)
    } else if s.kind == AttendanceKind::Arrival {
        format!("arrival:{}:{}:{student}", s.academic_term_id, s.date)
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
    super::lock_module(tx).await?;
    // Serialize initial recipient decisions across arrival rounds in a stable student/day order.
    if notices.iter().any(|notice| notice.daily_slot.is_some()) {
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended(daily_slot,1711)) FROM (SELECT DISTINCT daily_slot FROM jsonb_to_recordset($1) AS n(daily_slot text) WHERE daily_slot IS NOT NULL ORDER BY daily_slot) slots")
            .bind(Json(notices)).execute(&mut **tx).await?;
    }
    let rows: Vec<NewLedgerNotice> = sqlx::query_as(r#"WITH facts AS (SELECT n.* FROM jsonb_to_recordset($2) n(student_id uuid,key text,title text,message text) JOIN users student ON student.id=n.student_id AND student.status='active' AND student.user_type='student'), recipients AS (
 SELECT f.*,f.student_id recipient_id,'student'::text user_type FROM facts f
 UNION SELECT f.*,u.id,u.user_type FROM facts f JOIN student_parents p ON p.student_user_id=f.student_id JOIN users u ON u.id=p.parent_user_id AND u.status='active' AND u.user_type='parent'), inserted AS (
 INSERT INTO attendance_notifications(academic_term_id,recipient_id,student_id,dedup_key,notification_id)
 SELECT $1,recipient_id,student_id,key,uuidv7() FROM recipients ON CONFLICT(recipient_id,dedup_key) DO NOTHING
 RETURNING id,notification_id,recipient_id,student_id,dedup_key)
 SELECT i.id,i.notification_id,i.recipient_id,i.dedup_key,r.title,r.message,
 CASE WHEN r.user_type='parent' THEN '/parent/attendance?academicTermId=' ELSE '/student/attendance?academicTermId=' END || $1::text || '&studentId=' || i.student_id::text link
 FROM inserted i JOIN recipients r ON r.recipient_id=i.recipient_id AND r.student_id=i.student_id AND r.key=i.dedup_key"#).bind(term).bind(Json(notices)).fetch_all(&mut **tx).await?;
    enqueue_new_ledger(tx, rows).await
}
#[derive(sqlx::FromRow)]
struct NewLedgerNotice {
    id: Uuid,
    notification_id: Uuid,
    recipient_id: Uuid,
    dedup_key: String,
    title: String,
    message: String,
    link: String,
}
async fn enqueue_new_ledger(
    tx: &mut Transaction<'_, Postgres>,
    rows: Vec<NewLedgerNotice>,
) -> Result<(), AppError> {
    use school_notifications::{
        dispatch::{self, NotificationSource, QueuedNotification},
        publisher::NotificationType,
    };
    let items: Vec<_> = rows
        .into_iter()
        .map(|n| QueuedNotification {
            id: n.id,
            notification_id: n.notification_id,
            recipient_id: n.recipient_id,
            source: NotificationSource::Attendance { source_id: n.id },
            dedup_key: Some(n.dedup_key),
            title: n.title,
            message: n.message,
            type_: NotificationType::Info,
            link: Some(n.link),
        })
        .collect();
    let outcomes = dispatch::enqueue_in_transaction(tx, &items).await?;
    if outcomes.len() != items.len()
        || outcomes.iter().zip(&items).any(|(o, n)| {
            o.input_id != n.id
                || o.queue_id != n.id
                || o.notification_id != n.notification_id
                || o.source != n.source
        })
    {
        return Err(AppError::Conflict(
            "notification_ledger_identity_conflict".into(),
        ));
    }
    Ok(())
}
/// Structural source guard only. Recipients were fixed by the save-time snapshot.
pub async fn authorize_delivery_on_connection(
    connection: &mut sqlx::PgConnection,
    source_id: Uuid,
    recipient_id: Uuid,
) -> Result<bool, AppError> {
    sqlx::query("SELECT pg_advisory_xact_lock_shared(17010000)")
        .execute(&mut *connection)
        .await?;
    Ok(sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM attendance_notifications WHERE id=$1 AND recipient_id=$2)",
    )
    .bind(source_id)
    .bind(recipient_id)
    .fetch_one(connection)
    .await?)
}
pub async fn queue_digest(
    pool: &PgPool,
    term: Uuid,
    date: NaiveDate,
    time: NaiveTime,
) -> Result<(), AppError> {
    let epoch = super::sessions::module_generation(pool).await?;
    let tx = pool.begin().await?;
    queue_digest_in_transaction(tx, term, date, time, epoch, &[]).await
}
pub async fn queue_digest_in_transaction(
    mut tx: Transaction<'_, Postgres>,
    term: Uuid,
    date: NaiveDate,
    time: NaiveTime,
    epoch: Uuid,
    expected: &[AttendanceExpectedSummary],
) -> Result<(), AppError> {
    super::lock_term(&mut tx, term, true, true).await?;
    if super::sessions::module_generation_in_tx(&mut tx).await? != epoch {
        return Err(AppError::Conflict("ข้อมูลการเช็คชื่อถูกล้างแล้ว".into()));
    }
    let rows: Vec<NewLedgerNotice> = sqlx::query_as(r#"WITH actual AS (
 SELECT teacher,CASE s.kind WHEN 'arrival' THEN 'school' ELSE s.kind END category,COALESCE(r.result,'unchecked') result,1::bigint weight
 FROM attendance_sessions s CROSS JOIN LATERAL unnest(s.teacher_ids) teacher
 CROSS JOIN LATERAL jsonb_to_recordset(s.roster) p("studentId" uuid)
 LEFT JOIN attendance_records r ON r.session_id=s.id AND r.student_id=p."studentId"
 LEFT JOIN attendance_days d ON d.academic_term_id=s.academic_term_id AND d.date=s.date
 LEFT JOIN attendance_settings c ON c.academic_term_id=s.academic_term_id
 WHERE s.academic_term_id=$1 AND s.date=$2 AND NOT s.cancelled AND s.start_time<=$3
 AND COALESCE(s.count_override,d.counted,COALESCE(c.configuration->'weekdays','[1,2,3,4,5]'::jsonb) @> to_jsonb(EXTRACT(ISODOW FROM s.date)::integer))
 UNION ALL SELECT teacher,v.category,'unchecked',v.expected
 FROM jsonb_to_recordset($4) v(category text,"teacherIds" uuid[],expected bigint) CROSS JOIN LATERAL unnest(v."teacherIds") teacher),
 summaries AS (SELECT teacher,category,
 COALESCE(sum(weight) FILTER(WHERE result='present'),0) present,COALESCE(sum(weight) FILTER(WHERE result='late'),0) late,
 COALESCE(sum(weight) FILTER(WHERE result='absent'),0) absent,COALESCE(sum(weight) FILTER(WHERE result='skipped'),0) skipped,
 COALESCE(sum(weight) FILTER(WHERE result='sick_leave'),0) sick_leave,COALESCE(sum(weight) FILTER(WHERE result='personal_leave'),0) personal_leave,
 COALESCE(sum(weight) FILTER(WHERE result='activity'),0) activity,COALESCE(sum(weight) FILTER(WHERE result='unchecked'),0) unchecked
 FROM actual GROUP BY teacher,category),
 messages AS (SELECT teacher,string_agg((CASE category WHEN 'school' THEN 'เข้าโรงเรียน' WHEN 'lesson' THEN 'รายคาบ' ELSE 'รอบพิเศษ' END)||': มา '||present||' สาย '||late||' ขาด '||absent||' หนีเรียน '||skipped||' ลาป่วย '||sick_leave||' ลากิจ '||personal_leave||' กิจกรรม '||activity||' ยังไม่เช็ค '||unchecked,E'\n' ORDER BY category) message FROM summaries GROUP BY teacher)
 , inserted AS (
 INSERT INTO attendance_notifications(academic_term_id,recipient_id,dedup_key,notification_id)
 SELECT $1,teacher,'digest:'||$1::text||':'||$2::text||':'||$3::text,uuidv7() FROM messages JOIN users u ON u.id=teacher AND u.status='active'
 ON CONFLICT(recipient_id,dedup_key) DO NOTHING RETURNING id,notification_id,recipient_id,dedup_key)
 SELECT i.id,i.notification_id,i.recipient_id,i.dedup_key,'สรุปเช็คชื่อ '||$2::text title,m.message,
 '/staff/attendance?academicTermId='||$1::text||'&date='||$2::text link FROM inserted i JOIN messages m ON m.teacher=i.recipient_id"#)
        .bind(term).bind(date).bind(time).bind(Json(expected)).fetch_all(&mut *tx).await?;
    enqueue_new_ledger(&mut tx, rows).await?;
    tx.commit().await?;
    Ok(())
}
