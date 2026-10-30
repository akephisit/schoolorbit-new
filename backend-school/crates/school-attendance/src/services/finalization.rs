//! Persist overdue absence facts; reads and calendar browsing never call this owner.
use crate::{
    models::*,
    rules::{counted, invalid},
};
use chrono::{DateTime, NaiveDate, Utc};
use school_database::DbSession;
use school_errors::AppError;
use sqlx::{PgConnection, Postgres, Transaction, types::Json};
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

#[derive(Debug, Clone, Default)]
pub struct FinalizationOutcome {
    pub inserted_results: u64,
    pub already_completed: bool,
}

pub fn date_is_due(date: NaiveDate, start: NaiveDate, now: DateTime<Utc>) -> bool {
    date >= start && date < now.with_timezone(&chrono_tz::Asia::Bangkok).date_naive()
}

pub async fn finalize_date_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    term: Uuid,
    date: NaiveDate,
    generation: Uuid,
    seeds: &[AttendanceSeed],
    processed_at: DateTime<Utc>,
) -> Result<FinalizationOutcome, AppError> {
    let guard = guard_finalization(tx, term).await?;
    finalize_guarded(tx, &guard, date, generation, seeds, processed_at).await
}

pub(crate) struct GuardedFinalization {
    pub context: super::settings::AttendanceReadContext,
}
pub(crate) async fn guard_finalization(
    tx: &mut Transaction<'_, Postgres>,
    term: Uuid,
) -> Result<GuardedFinalization, AppError> {
    super::lock_term(tx, term, true, true).await?;
    Ok(GuardedFinalization {
        context: super::settings::AttendanceReadContext::load(&mut *tx, term).await?,
    })
}
pub(crate) async fn finalize_guarded(
    tx: &mut Transaction<'_, Postgres>,
    guard: &GuardedFinalization,
    date: NaiveDate,
    generation: Uuid,
    seeds: &[AttendanceSeed],
    processed_at: DateTime<Utc>,
) -> Result<FinalizationOutcome, AppError> {
    let term = guard.context.settings.academic_term_id;
    let (actual, start, coverage): (Uuid, NaiveDate, NaiveDate) = DbSession::new(&mut **tx).query_as("attendance.services.finalization.finalize_guarded",
        "SELECT generation,auto_finalize_from,coverage_start FROM attendance_module_state WHERE singleton",
    )
    .fetch_one()
    .await?;
    if actual != generation {
        return Err(AppError::Conflict(
            "ระบบเช็กชื่อถูกรีเซ็ตแล้ว กรุณาประมวลผลใหม่".into(),
        ));
    }
    if !date_is_due(date, start.max(coverage), processed_at) {
        return Err(invalid("วันที่นี้ยังไม่ถึงกำหนดปิดยอดอัตโนมัติ"));
    }
    let valid: bool = DbSession::new(&mut **tx)
        .scalar(
            "attendance.services.finalization.finalize_guarded",
            "SELECT EXISTS(SELECT 1 FROM attendance_days WHERE academic_term_id=$1 AND date=$2)",
        )
        .bind(term)
        .bind(date)
        .fetch_one()
        .await?;
    if !valid {
        return Err(invalid("วันปิดยอดไม่อยู่ในภาคเรียน"));
    }
    let complete: bool = DbSession::new(&mut **tx).scalar("attendance.services.finalization.finalize_guarded", "SELECT EXISTS(SELECT 1 FROM attendance_finalized_dates WHERE generation=$1 AND academic_term_id=$2 AND date=$3)")
        .bind(generation).bind(term).bind(date).fetch_one().await?;
    if complete {
        return Ok(FinalizationOutcome {
            inserted_results: 0,
            already_completed: true,
        });
    }
    let settings = &guard.context.settings;
    let day: Option<bool> = DbSession::new(&mut **tx)
        .scalar(
            "attendance.services.finalization.finalize_guarded",
            "SELECT counted FROM attendance_days WHERE academic_term_id=$1 AND date=$2",
        )
        .bind(term)
        .bind(date)
        .fetch_optional()
        .await?;
    let counted_day = counted(&settings.configuration, date, day);
    let mut rounds = BTreeMap::new();
    for seed in seeds {
        if seed.session.academic_term_id != term
            || seed.session.date != date
            || seed.students.len() > 2000
        {
            return Err(invalid("รายชื่อปิดยอดไม่ตรงกับวันที่หรือภาคเรียน"));
        }
        let mut seen = HashSet::new();
        if seed.students.iter().any(|s| !seen.insert(s.student_id)) {
            return Err(invalid("รายชื่อปิดยอดมีนักเรียนซ้ำ"));
        }
        if rounds.insert(seed.session.id, seed.clone()).is_some() {
            return Err(invalid("รอบปิดยอดซ้ำ"));
        }
    }
    let sql = format!(
        "SELECT {} FROM attendance_sessions WHERE academic_term_id=$1 AND date=$2 ORDER BY id FOR UPDATE",
        super::sessions::COLUMNS
    );
    let saved: Vec<AttendanceSession> = DbSession::new(&mut **tx)
        .query_as(
            "attendance.services.finalization.finalize_guarded",
            sqlx::AssertSqlSafe(sql),
        )
        .bind(term)
        .bind(date)
        .fetch_all()
        .await?;
    for session in saved {
        rounds.insert(
            session.id,
            AttendanceSeed {
                students: session.roster.clone(),
                session,
            },
        );
    }
    if rounds.len() > 2000 || rounds.values().map(|r| r.students.len()).sum::<usize>() > 200_000 {
        return Err(invalid("รอบปิดยอดเกินขนาดที่รองรับ"));
    }
    let mut outcome = FinalizationOutcome::default();
    for seed in rounds.values() {
        if seed.session.cancelled
            || !seed.session.count_override.unwrap_or(counted_day)
            || seed.students.is_empty()
        {
            continue;
        }
        super::sessions::insert_seed(tx, seed).await?;
        let inserted = DbSession::new(&mut **tx).query("attendance.services.finalization.finalize_guarded", r#"INSERT INTO attendance_records(session_id,student_id,student_academic_year_id,special_round_id,class_number,result,origin,processed_at)
            SELECT $1,"studentId","studentAcademicYearId",$2,"classNumber",'absent','automatic',$3
            FROM jsonb_to_recordset($4) AS p("studentId" uuid,"studentAcademicYearId" uuid,"classNumber" integer)
            ON CONFLICT(session_id,student_id) DO NOTHING"#)
            .bind(seed.session.id).bind(seed.session.special_round_id).bind(processed_at).bind(Json(&seed.students))
            .execute().await?.rows_affected();
        if inserted > 0 {
            DbSession::new(&mut **tx).query("attendance.services.finalization.finalize_guarded",
                "UPDATE attendance_sessions SET saved_at=$2,row_version=row_version+1 WHERE id=$1",
            )
            .bind(seed.session.id)
            .bind(processed_at)
            .execute()
            .await?;
            outcome.inserted_results += inserted;
        }
    }
    DbSession::new(&mut **tx).query("attendance.services.finalization.finalize_guarded", "INSERT INTO attendance_finalized_dates(generation,academic_term_id,date,processed_at) VALUES($1,$2,$3,$4)")
        .bind(generation).bind(term).bind(date).bind(processed_at).execute().await?;
    super::score_changes::enqueue_day(tx, term, date).await?;
    DbSession::new(&mut **tx).query("attendance.services.finalization.finalize_guarded", "INSERT INTO audit_logs(action,entity_type,entity_id,new_values,description) VALUES('create','attendance_finalization',$1,$2,'automatic daily finalization')")
        .bind(term).bind(Json(serde_json::json!({"date":date,"generation":generation,"insertedResults":outcome.inserted_results})))
        .execute().await?;
    Ok(outcome)
}

/// Called under lifecycle locks with the actual closure operation date.
pub async fn require_finalized_through(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
) -> Result<(), AppError> {
    let configured: bool = DbSession::new(&mut *connection)
        .scalar(
            "attendance.services.finalization.require_finalized_through",
            "SELECT EXISTS(SELECT 1 FROM attendance_settings WHERE academic_term_id=$1)",
        )
        .bind(term)
        .fetch_one()
        .await?;
    if !configured {
        return Ok(());
    }
    let pending: bool = DbSession::new(&mut *connection).scalar("attendance.services.finalization.require_finalized_through", "SELECT EXISTS(SELECT 1 FROM attendance_days day CROSS JOIN attendance_module_state module WHERE day.academic_term_id=$1 AND ((day.counted AND day.date>$2) OR (day.date BETWEEN GREATEST(module.auto_finalize_from,module.coverage_start) AND $2 AND NOT EXISTS(SELECT 1 FROM attendance_finalized_dates finalized WHERE finalized.generation=module.generation AND finalized.academic_term_id=day.academic_term_id AND finalized.date=day.date))))")
        .bind(term).bind(date).fetch_one().await?;
    if pending {
        return Err(AppError::Conflict(
            "ยังปิดยอดเช็กชื่อไม่ครบถึงวันสุดท้าย กรุณารอการปิดยอดหลังเที่ยงคืน".into(),
        ));
    }
    Ok(())
}

/// Bounded persistent catch-up selection includes the last date of an ended term.
pub async fn due_dates_on_connection(
    connection: &mut PgConnection,
    now: DateTime<Utc>,
    limit: i64,
) -> Result<Vec<(Uuid, NaiveDate, Uuid)>, AppError> {
    if !(1..=31).contains(&limit) {
        return Err(invalid("ขนาดชุดปิดยอดไม่ถูกต้อง"));
    }
    let yesterday = now
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive()
        .pred_opt()
        .ok_or_else(|| invalid("วันที่ปิดยอดไม่ถูกต้อง"))?;
    Ok(DbSession::new(&mut *connection).query_as("attendance.services.finalization.due_dates_on_connection", "SELECT t.id,day.date,m.generation FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id JOIN attendance_settings s ON s.academic_term_id=t.id JOIN attendance_days day ON day.academic_term_id=t.id CROSS JOIN attendance_module_state m WHERE day.date BETWEEN GREATEST(m.auto_finalize_from,m.coverage_start) AND $1 AND t.status IN ('active','closing') AND y.status NOT IN ('closed','archived') AND NOT EXISTS(SELECT 1 FROM attendance_term_archives a WHERE a.academic_term_id=t.id) AND NOT EXISTS(SELECT 1 FROM attendance_finalized_dates f WHERE f.generation=m.generation AND f.academic_term_id=t.id AND f.date=day.date) ORDER BY day.date,t.id LIMIT $2")
        .bind(yesterday).bind(limit).fetch_all().await?)
}

pub async fn ready_through_on_connection(
    connection: &mut PgConnection,
    today: NaiveDate,
) -> Result<NaiveDate, AppError> {
    let yesterday = today.pred_opt().ok_or_else(|| invalid("วันที่ไม่ถูกต้อง"))?;
    let earliest: Option<NaiveDate> = DbSession::new(&mut *connection).scalar("attendance.services.finalization.ready_through_on_connection", "SELECT min(day.date) FROM academic_terms t JOIN academic_years y ON y.id=t.academic_year_id JOIN attendance_settings s ON s.academic_term_id=t.id JOIN attendance_days day ON day.academic_term_id=t.id CROSS JOIN attendance_module_state m WHERE day.date BETWEEN GREATEST(m.auto_finalize_from,m.coverage_start) AND $1 AND t.status IN ('active','closing') AND y.status NOT IN ('closed','archived') AND NOT EXISTS(SELECT 1 FROM attendance_term_archives a WHERE a.academic_term_id=t.id) AND NOT EXISTS(SELECT 1 FROM attendance_finalized_dates f WHERE f.generation=m.generation AND f.academic_term_id=t.id AND f.date=day.date)")
        .bind(yesterday).fetch_one().await?;
    Ok(earliest
        .and_then(|d| d.pred_opt())
        .map_or(yesterday, |d| d.min(yesterday)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    #[test]
    fn bangkok_midnight_and_activation_define_absence_cutoff() {
        let day = NaiveDate::from_ymd_opt(2026, 10, 14).unwrap();
        let before = Utc.with_ymd_and_hms(2026, 10, 14, 16, 59, 59).unwrap();
        let midnight = Utc.with_ymd_and_hms(2026, 10, 14, 17, 0, 0).unwrap();
        assert!(!date_is_due(day, day, before));
        assert!(date_is_due(day, day, midnight));
        assert!(!date_is_due(day.pred_opt().unwrap(), day, midnight));
        assert!(date_is_due(day, day, midnight + chrono::Duration::days(3)));
        assert!(!date_is_due(day.succ_opt().unwrap(), day, midnight));
    }
}
