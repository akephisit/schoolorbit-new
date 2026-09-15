use super::*;
use crate::modules::academic::core::services_tests::prepare_core_fixture;
use school_errors::AppError;
use uuid::Uuid;

#[tokio::test]
async fn promotion_student_projection_keeps_source_context_and_reports_existing_target_without_adopting_it(
) {
    let pool = prepare_core_fixture("promotion_student_projection").await;
    let year: Uuid = sqlx::query_scalar("SELECT id FROM academic_years WHERE status='active'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let student: Uuid = sqlx::query_scalar(
        "SELECT id FROM student_academic_years WHERE academic_year_id=$1 ORDER BY id LIMIT 1",
    )
    .bind(year)
    .fetch_one(&pool)
    .await
    .unwrap();
    let target: Uuid = sqlx::query_scalar(
        "INSERT INTO academic_years(year,name,start_date,end_date,school_days,status) \
         SELECT max(year)+1,'E2E-LIFECYCLE-target',max(end_date)+1,max(end_date)+366,'MON','planning' \
         FROM academic_years RETURNING id",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let rows = read_promotion_students(&mut tx, year, target, &[student])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].student_academic_year_id, student);
    assert!(!rows[0].student_name.is_empty());
    assert!(rows[0].existing_target_student_year_id.is_none());
    assert!(rows[0].existing_target_row_version.is_none());
    for invalid in [
        vec![],
        vec![student, student],
        vec![Uuid::new_v4()],
        vec![student; 501],
    ] {
        assert!(read_promotion_students(&mut tx, year, target, &invalid)
            .await
            .is_err());
    }
    assert!(read_promotion_students(&mut tx, target, year, &[student])
        .await
        .is_err());
    assert!(read_promotion_students(&mut tx, year, year, &[student])
        .await
        .is_err());
    tx.rollback().await.unwrap();

    let foreign_target: Uuid = sqlx::query_scalar(
        "INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) \
         SELECT gen_random_uuid(),student_id,$1,grade_level_id,study_program_id,'planned' \
         FROM student_academic_years WHERE id=$2 RETURNING id",
    )
    .bind(target)
    .bind(student)
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let rows = read_promotion_students(&mut tx, year, target, &[student])
        .await
        .unwrap();
    assert_eq!(
        rows[0].existing_target_student_year_id,
        Some(foreign_target)
    );
    assert_eq!(rows[0].existing_target_row_version, Some(1));
    assert_eq!(rows[0].student_academic_year_id, student);
    let target_state: (String, i64) =
        sqlx::query_as("SELECT status,row_version FROM student_academic_years WHERE id=$1")
            .bind(foreign_target)
            .fetch_one(&mut *tx)
            .await
            .unwrap();
    assert_eq!(target_state, ("planned".into(), 1));
}

#[tokio::test]
async fn promotion_run_context_never_treats_same_or_earlier_year_as_destination() {
    let pool = prepare_core_fixture("promotion_run_year_order").await;
    let year: Uuid = sqlx::query_scalar("SELECT id FROM academic_years WHERE status='active'")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    assert!(matches!(
        validate_run_years(&mut tx, year, year).await,
        Err(AppError::ValidationError(_))
    ));
    assert!(matches!(
        validate_run_years(&mut tx, year, Uuid::new_v4()).await,
        Err(AppError::NotFound(_))
    ));
}
