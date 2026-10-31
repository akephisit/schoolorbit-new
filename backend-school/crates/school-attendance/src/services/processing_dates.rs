//! Finite Attendance work: explicit calendar entries and real counted round rosters.
//! A persisted session owns its special snapshot, including cancellation/false overrides.
use chrono::NaiveDate;
use school_database::DbSession;
use school_errors::AppError;
use sqlx::PgConnection;
use uuid::Uuid;

const SOURCE: &str = r#"WITH date_scope AS (
 SELECT $1::uuid term,$2::date starts_on,$3::date ends_on
), date_sources AS (
 SELECT d.academic_term_id,d.date,d.counted calendar_counted,false counted_round
 FROM attendance_days d CROSS JOIN date_scope scope
 WHERE (scope.term IS NULL OR d.academic_term_id=scope.term)
 AND (scope.starts_on IS NULL OR d.date>=scope.starts_on)
 AND (scope.ends_on IS NULL OR d.date<=scope.ends_on)
 UNION ALL
 SELECT s.academic_term_id,s.date,false,true
 FROM attendance_sessions s CROSS JOIN date_scope scope
 LEFT JOIN attendance_days d ON d.academic_term_id=s.academic_term_id AND d.date=s.date
 WHERE (scope.term IS NULL OR s.academic_term_id=scope.term)
 AND (scope.starts_on IS NULL OR s.date>=scope.starts_on)
 AND (scope.ends_on IS NULL OR s.date<=scope.ends_on)
 AND NOT s.cancelled AND jsonb_array_length(s.roster)>0
 AND COALESCE(s.count_override,d.counted,false)
 UNION ALL
 SELECT t.academic_term_id,r.date,false,true
 FROM attendance_special_rounds r JOIN attendance_special_templates t ON t.id=r.template_id
 CROSS JOIN date_scope scope
 CROSS JOIN LATERAL jsonb_to_recordset(r.participants) p("sessionId" uuid,"countOverride" boolean,students jsonb)
 LEFT JOIN attendance_days d ON d.academic_term_id=t.academic_term_id AND d.date=r.date
 WHERE (scope.term IS NULL OR t.academic_term_id=scope.term)
 AND (scope.starts_on IS NULL OR r.date>=scope.starts_on)
 AND (scope.ends_on IS NULL OR r.date<=scope.ends_on)
 AND jsonb_array_length(p.students)>0 AND COALESCE(p."countOverride",d.counted,false)
 AND NOT EXISTS(SELECT 1 FROM attendance_sessions s WHERE s.id=p."sessionId")
), processing_dates AS (
 SELECT academic_term_id,date,bool_or(calendar_counted) calendar_counted,bool_or(counted_round) counted_round
 FROM date_sources GROUP BY academic_term_id,date
) "#;

// Both pieces are server-owned static SQL. Values always use the three scope binds.
pub(crate) fn query(selection: &'static str) -> sqlx::AssertSqlSafe<String> {
    sqlx::AssertSqlSafe(format!("{SOURCE}{selection}"))
}

#[derive(Debug, sqlx::FromRow)]
pub(crate) struct ProcessingDate {
    pub calendar_counted: bool,
    pub counted_round: bool,
}
impl ProcessingDate {
    pub fn counted(&self) -> bool {
        self.calendar_counted || self.counted_round
    }
}

pub(crate) async fn on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    date: NaiveDate,
) -> Result<Option<ProcessingDate>, AppError> {
    Ok(DbSession::new(connection)
        .query_as(
            "attendance.processing_dates.day",
            query("SELECT calendar_counted,counted_round FROM processing_dates"),
        )
        .bind(Some(term))
        .bind(Some(date))
        .bind(Some(date))
        .fetch_optional()
        .await?)
}

pub(crate) async fn bounds_on_connection(
    connection: &mut PgConnection,
    term: Uuid,
    range: Option<(NaiveDate, NaiveDate)>,
) -> Result<(Option<NaiveDate>, Option<NaiveDate>, NaiveDate), AppError> {
    Ok(DbSession::new(connection).query_as("attendance.processing_dates.bounds",
        query("SELECT min(d.date),max(d.date),m.coverage_start FROM attendance_module_state m LEFT JOIN processing_dates d ON true WHERE m.singleton GROUP BY m.coverage_start"))
        .bind(Some(term)).bind(range.map(|r|r.0)).bind(range.map(|r|r.1)).fetch_one().await?)
}

#[cfg(test)]
mod tests {
    use super::ProcessingDate;
    #[test]
    fn real_counted_rounds_and_calendar_are_independent() {
        for (calendar_counted, counted_round, expected) in [
            (false, false, false),
            (true, false, true),
            (false, true, true),
            (true, true, true),
        ] {
            assert_eq!(
                ProcessingDate {
                    calendar_counted,
                    counted_round
                }
                .counted(),
                expected
            );
        }
    }
}
