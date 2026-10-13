use super::super::db_tests::{fixture, STUDENT, TERM};
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
    sqlx::query("INSERT INTO attendance_settings(academic_term_id,configuration) VALUES($1,$2)")
        .bind(TERM)
        .bind(sqlx::types::Json(AttendanceConfiguration::default()))
        .execute(&f.pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_terms SET planned_end_date=$2 WHERE id=$1")
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
