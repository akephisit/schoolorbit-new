use chrono::NaiveDate;
use school_academic_core::{
    models::{AcademicYearStatus, CreateAcademicYearRequest},
    services::years_terms,
};
use uuid::Uuid;

#[tokio::test]
async fn academic_year_command_persists_through_the_core_crate_boundary() {
    let pool = school_test_db::create_named_test_pool("academic_core_crate_persistence").await;
    school_test_db::run_test_migrations(&pool).await;
    let actor = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users(id,username,password_hash,first_name,last_name,user_type)
         VALUES($1,'academic-core-crate-test','!','Academic','Core','staff')",
    )
    .bind(actor)
    .execute(&pool)
    .await
    .unwrap();

    let created = years_terms::create_year(
        &pool,
        actor,
        CreateAcademicYearRequest {
            year: 2701,
            custom_name: Some("ปีทดสอบ crate".into()),
            start_date: NaiveDate::from_ymd_opt(2158, 5, 1).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2159, 4, 30).unwrap(),
            school_days: vec!["MON".into(), "TUE".into(), "WED".into()],
        },
    )
    .await
    .unwrap();

    assert_eq!(created.status, AcademicYearStatus::Planning);
    assert_eq!(
        years_terms::get_year(&pool, created.id).await.unwrap().id,
        created.id
    );
    let audit_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_audit_events
         WHERE entity_type='academic_year' AND entity_id=$1 AND actor_user_id=$2",
    )
    .bind(created.id)
    .bind(actor)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_count, 1);
}
