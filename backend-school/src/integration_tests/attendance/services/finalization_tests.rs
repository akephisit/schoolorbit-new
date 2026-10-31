use super::super::db_tests::{fixture, STUDENT, TEACHER, TERM};
use chrono::{NaiveDate, Utc};
use school_attendance::models::AttendanceConfiguration;

#[tokio::test]
async fn attendance_finalization_catches_up_on_one_connection_with_historical_inactive_room() {
    use chrono::TimeZone;
    let f = fixture("attendance_finalization_one_connection").await;
    let date = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    sqlx::query("UPDATE attendance_module_state SET auto_finalize_from=$1,coverage_start=$1")
        .bind(date)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO attendance_settings(academic_term_id,configuration,updated_by) VALUES($1,$2,$3)")
        .bind(TERM)
        .bind(sqlx::types::Json(AttendanceConfiguration::default()))
        .bind(TEACHER)
        .execute(&f.pool)
        .await
        .unwrap();
    // Finalization follows this explicit Attendance calendar, independently of term identity.
    sqlx::query("DELETE FROM attendance_days WHERE academic_term_id=$1")
        .bind(TERM)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO attendance_days(academic_term_id,date,counted) VALUES($1,$2,true)")
        .bind(TERM)
        .bind(date)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE homerooms SET is_active=false WHERE id IN (SELECT homeroom_id FROM homeroom_placements WHERE student_academic_year_id=$1)")
        .bind(f.students[0].student_academic_year_id).execute(&f.pool).await.unwrap();
    let schema: String = sqlx::query_scalar("SELECT current_schema()")
        .fetch_one(&f.pool)
        .await
        .unwrap();
    let options = (*f.pool.connect_options())
        .clone()
        .options([("search_path", format!("{schema},public"))]);
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(1)
        .acquire_timeout(std::time::Duration::from_secs(2))
        .connect_with(options)
        .await
        .unwrap();
    let now = Utc.with_ymd_and_hms(2025, 10, 9, 17, 0, 0).unwrap();
    let count = tokio::time::timeout(
        std::time::Duration::from_secs(15),
        super::catch_up(&pool, now, 4),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(count, 1);
    assert!(sqlx::query_scalar::<_,bool>("SELECT EXISTS(SELECT 1 FROM attendance_records WHERE student_id=$1 AND result='absent' AND origin='automatic')")
        .bind(STUDENT).fetch_one(&pool).await.unwrap());
    assert_eq!(
        super::catch_up(&pool, now + chrono::Duration::days(2), 4)
            .await
            .unwrap(),
        0
    );
    school_attendance::services::finalization::require_finalized_through(
        &mut *pool.acquire().await.unwrap(),
        TERM,
        date,
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn attendance_finalization_never_infers_missing_calendar_days_and_preserves_uncounted_days() {
    use chrono::TimeZone;
    let f = fixture("attendance_finalization_explicit_calendar").await;
    let uncounted = NaiveDate::from_ymd_opt(2025, 10, 9).unwrap();
    let counted = uncounted.succ_opt().unwrap();
    sqlx::query("UPDATE attendance_module_state SET auto_finalize_from=$1,coverage_start=$1")
        .bind(uncounted)
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO attendance_settings(academic_term_id,configuration,updated_by) VALUES($1,$2,$3)")
        .bind(TERM).bind(sqlx::types::Json(AttendanceConfiguration::default())).bind(TEACHER)
        .execute(&f.pool).await.unwrap();
    sqlx::query("DELETE FROM attendance_days WHERE academic_term_id=$1")
        .bind(TERM)
        .execute(&f.pool)
        .await
        .unwrap();
    // UTC 17:00 is the next Bangkok midnight, when the preceding day becomes due.
    let midnight = Utc.with_ymd_and_hms(2025, 10, 9, 17, 0, 0).unwrap();
    assert_eq!(super::catch_up(&f.pool, midnight, 4).await.unwrap(), 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT count(*) FROM attendance_days WHERE academic_term_id=$1"
        )
        .bind(TERM)
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        0,
        "catch-up must never manufacture a weekday calendar"
    );
    sqlx::query("INSERT INTO attendance_days(academic_term_id,date,counted) VALUES($1,$2,false),($1,$3,true)")
        .bind(TERM).bind(uncounted).bind(counted).execute(&f.pool).await.unwrap();
    assert_eq!(super::catch_up(&f.pool, midnight, 4).await.unwrap(), 1);
    let automatic_on_uncounted: i64 = sqlx::query_scalar("SELECT count(*) FROM attendance_records record JOIN attendance_sessions session ON session.id=record.session_id WHERE session.academic_term_id=$1 AND session.date=$2 AND record.origin='automatic'")
        .bind(TERM).bind(uncounted).fetch_one(&f.pool).await.unwrap();
    assert_eq!(automatic_on_uncounted, 0);
    assert_eq!(
        super::catch_up(&f.pool, midnight + chrono::Duration::days(1), 4)
            .await
            .unwrap(),
        1
    );
    assert!(sqlx::query_scalar::<_, bool>("SELECT EXISTS(SELECT 1 FROM attendance_records record JOIN attendance_sessions session ON session.id=record.session_id WHERE session.academic_term_id=$1 AND session.date=$2 AND record.student_id=$3 AND record.result='absent' AND record.origin='automatic')")
        .bind(TERM).bind(counted).bind(STUDENT).fetch_one(&f.pool).await.unwrap());
    assert_eq!(
        super::catch_up(&f.pool, midnight + chrono::Duration::days(3), 4)
            .await
            .unwrap(),
        0
    );
    school_attendance::services::finalization::require_finalized_through(
        &mut *f.pool.acquire().await.unwrap(),
        TERM,
        counted,
    )
    .await
    .unwrap();
}
