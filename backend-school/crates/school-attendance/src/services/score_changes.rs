//! Attendance-owned durable dirty-day queue. Consumers reconcile the current day,
//! then acknowledge the exact revision; a concurrent correction stays queued.
use crate::models::{AttendanceKind, AttendanceResult, AttendanceScoreRule};
use chrono::{DateTime, NaiveDate, Utc};
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{FromRow, PgConnection, Postgres, Transaction};
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct AttendanceScoreFact {
    pub student_id: Uuid,
    pub student_academic_year_id: Uuid,
    pub source_key: String,
    pub kind: AttendanceKind,
    pub result: AttendanceResult,
    pub counted: bool,
    pub grade_level_id: Uuid,
    pub homeroom_id: Option<Uuid>,
    #[sqlx(json(nullable))]
    pub scoring: Option<Vec<AttendanceScoreRule>>,
}

/// Complete canonical day's facts. An omitted cancelled/excluded fact tells the
/// consumer to remove its prior effect; unchecked coverage is never a score fact.
pub async fn day_facts_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
) -> Result<Vec<AttendanceScoreFact>, AppError> {
    let facts: Vec<AttendanceScoreFact> = DbSession::new(&mut *connection).query_as("attendance.services.score_changes.day_facts_on_connection", r#"WITH eligible AS (
        SELECT r.student_id,r.student_academic_year_id,s.id session_id,s.kind,r.result,
            y.grade_level_id,p.homeroom_id,template.definition->'scoring' scoring
        FROM attendance_sessions s JOIN attendance_records r ON r.session_id=s.id
        JOIN student_academic_years y ON y.id=r.student_academic_year_id
        LEFT JOIN LATERAL (SELECT h.homeroom_id FROM homeroom_placements h
            WHERE h.student_academic_year_id=r.student_academic_year_id AND h.status IN ('current','ended')
            AND h.start_date<=$2 AND (h.end_date IS NULL OR h.end_date>=$2)
            ORDER BY h.start_date DESC,h.id LIMIT 1) p ON true
        LEFT JOIN attendance_days d ON d.academic_term_id=s.academic_term_id AND d.date=s.date
        LEFT JOIN attendance_settings c ON c.academic_term_id=s.academic_term_id
        LEFT JOIN attendance_special_rounds special ON special.id=s.special_round_id
        LEFT JOIN attendance_special_templates template ON template.id=special.template_id
        WHERE s.academic_term_id=$1 AND s.date=$2 AND NOT s.cancelled
          AND COALESCE(s.count_override,d.counted,COALESCE(c.configuration->'weekdays','[1,2,3,4,5]'::jsonb) @> to_jsonb(EXTRACT(ISODOW FROM s.date)::integer))
    ), daily AS (SELECT DISTINCT ON(student_id) * FROM eligible WHERE kind='arrival'
        ORDER BY student_id,(result IN ('present','late')) DESC,result,session_id)
    SELECT student_id,student_academic_year_id,'arrival'::text source_key,kind,result,true counted,
        grade_level_id,homeroom_id,NULL::jsonb scoring FROM daily
    UNION ALL SELECT student_id,student_academic_year_id,session_id::text source_key,kind,result,true counted,
        grade_level_id,homeroom_id,NULLIF(scoring,'null'::jsonb) scoring FROM eligible WHERE kind<>'arrival'
    ORDER BY student_id,kind,source_key LIMIT 200001"#)
        .bind(term).bind(date).fetch_all().await?;
    if facts.len() > 200_000 {
        return Err(crate::rules::invalid("ผลเช็กชื่อรายวันเกินขนาดที่รองรับ"));
    }
    Ok(facts)
}

#[derive(Debug, Clone, FromRow)]
pub struct AttendanceScoreChange {
    pub generation: Uuid,
    pub academic_term_id: Uuid,
    pub date: NaiveDate,
    pub row_version: i64,
    pub queued_at: DateTime<Utc>,
    pub reset: bool,
}

pub async fn enqueue_day(
    tx: &mut Transaction<'_, Postgres>,
    term: Uuid,
    date: NaiveDate,
) -> Result<(), AppError> {
    let generation = super::sessions::module_generation_in_tx(tx).await?;
    enqueue_with_generation(tx, generation, term, date, false).await
}

pub async fn enqueue_dates(
    tx: &mut Transaction<'_, Postgres>,
    term: Uuid,
    dates: &[NaiveDate],
) -> Result<(), AppError> {
    let generation = super::sessions::module_generation_in_tx(tx).await?;
    DbSession::new(&mut **tx).query("attendance.services.score_changes.enqueue_dates", "INSERT INTO attendance_score_changes(generation,academic_term_id,date) SELECT $1,$2,date FROM (SELECT DISTINCT unnest($3::date[]) date) dates ON CONFLICT(generation,academic_term_id,date) DO UPDATE SET row_version=nextval('attendance_score_revision_seq'),queued_at=now()")
        .bind(generation).bind(term).bind(dates).execute().await?;
    Ok(())
}

pub async fn enqueue_term_days(
    tx: &mut Transaction<'_, Postgres>,
    term: Uuid,
) -> Result<(), AppError> {
    let dates: Vec<NaiveDate> = DbSession::new(&mut **tx).scalar("attendance.services.score_changes.enqueue_term_days", "SELECT date FROM attendance_sessions WHERE academic_term_id=$1 UNION SELECT date FROM attendance_finalized_dates WHERE academic_term_id=$1 AND generation=(SELECT generation FROM attendance_module_state WHERE singleton)")
        .bind(term).fetch_all().await?;
    enqueue_dates(tx, term, &dates).await
}

pub async fn enqueue_with_generation(
    tx: &mut Transaction<'_, Postgres>,
    generation: Uuid,
    term: Uuid,
    date: NaiveDate,
    reset: bool,
) -> Result<(), AppError> {
    DbSession::new(&mut **tx).query("attendance.services.score_changes.enqueue_with_generation", "INSERT INTO attendance_score_changes(generation,academic_term_id,date,reset) VALUES($1,$2,$3,$4) ON CONFLICT(generation,academic_term_id,date) DO UPDATE SET row_version=nextval('attendance_score_revision_seq'),queued_at=now(),reset=attendance_score_changes.reset OR excluded.reset")
        .bind(generation).bind(term).bind(date).bind(reset).execute().await?;
    Ok(())
}

pub async fn pending_on_connection(
    connection: &mut PgConnection,
    limit: i64,
) -> Result<Vec<AttendanceScoreChange>, AppError> {
    if !(1..=100).contains(&limit) {
        return Err(crate::rules::invalid("ขนาดชุดงานคะแนนไม่ถูกต้อง"));
    }
    Ok(DbSession::new(&mut *connection).query_as("attendance.services.score_changes.pending_on_connection", "SELECT generation,academic_term_id,date,row_version,queued_at,reset FROM attendance_score_changes ORDER BY queued_at,generation,academic_term_id,date LIMIT $1")
        .bind(limit).fetch_all().await?)
}

/// Reload a prefetched key after the caller acquires Attendance's module/term
/// locks. Reset can change both the revision and the meaning of a queued day;
/// the prefetched payload is never authority for selecting current source facts.
pub async fn current_on_connection(
    connection: &mut PgConnection,
    prefetched: &AttendanceScoreChange,
) -> Result<Option<AttendanceScoreChange>, AppError> {
    let current: Option<AttendanceScoreChange> = DbSession::new(&mut *connection).query_as("attendance.services.score_changes.current_on_connection", "SELECT generation,academic_term_id,date,row_version,queued_at,reset FROM attendance_score_changes WHERE generation=$1 AND academic_term_id=$2 AND date=$3")
        .bind(prefetched.generation).bind(prefetched.academic_term_id).bind(prefetched.date)
        .fetch_optional().await?;
    let Some(current) = current else {
        return Ok(None);
    };
    if !current.reset {
        let generation: Uuid = DbSession::new(&mut *connection)
            .scalar(
                "attendance.services.score_changes.current_on_connection",
                "SELECT generation FROM attendance_module_state WHERE singleton",
            )
            .fetch_one()
            .await?;
        if current.generation != generation {
            return Err(AppError::Conflict(
                "งานคะแนนอ้างอิงเช็กชื่อรุ่นเก่าโดยไม่มีคำสั่งรีเซ็ต จึงยังไม่อ่านผลรุ่นใหม่".into(),
            ));
        }
    }
    Ok(Some(current))
}

pub async fn acknowledge(
    tx: &mut Transaction<'_, Postgres>,
    change: &AttendanceScoreChange,
) -> Result<bool, AppError> {
    Ok(DbSession::new(&mut **tx).query("attendance.services.score_changes.acknowledge", "DELETE FROM attendance_score_changes WHERE generation=$1 AND academic_term_id=$2 AND date=$3 AND row_version=$4")
        .bind(change.generation).bind(change.academic_term_id).bind(change.date).bind(change.row_version)
        .execute().await?.rows_affected() == 1)
}

pub async fn require_drained(connection: &mut PgConnection, term: Uuid) -> Result<(), AppError> {
    let pending: bool = DbSession::new(&mut *connection)
        .scalar(
            "attendance.services.score_changes.require_drained",
            "SELECT EXISTS(SELECT 1 FROM attendance_score_changes WHERE academic_term_id=$1)",
        )
        .bind(term)
        .fetch_one()
        .await?;
    if pending {
        return Err(AppError::Conflict(
            "รอระบบคำนวณคะแนนจากเช็กชื่อให้ครบก่อนปิดหรือล้างภาคเรียน".into(),
        ));
    }
    Ok(())
}

pub async fn delivered_through_on_connection(
    connection: &mut PgConnection,
    today: NaiveDate,
) -> Result<NaiveDate, AppError> {
    let yesterday = today
        .pred_opt()
        .ok_or_else(|| crate::rules::invalid("วันที่ไม่ถูกต้อง"))?;
    let earliest: Option<NaiveDate> = DbSession::new(&mut *connection)
        .scalar(
            "attendance.services.score_changes.delivered_through_on_connection",
            "SELECT min(date) FROM attendance_score_changes WHERE date<$1",
        )
        .bind(today)
        .fetch_one()
        .await?;
    Ok(earliest
        .and_then(|d| d.pred_opt())
        .map_or(yesterday, |d| d.min(yesterday)))
}
