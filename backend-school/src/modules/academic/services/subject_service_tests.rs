use crate::error::AppError;
use crate::modules::academic::models::curriculum::UpdateSubjectRequest;
use crate::test_helpers::{create_test_pool, run_test_migrations};
use uuid::Uuid;

use super::subject_service;

async fn migrated_pool() -> sqlx::PgPool {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;
    pool
}

fn empty_update() -> UpdateSubjectRequest {
    UpdateSubjectRequest {
        code: None,
        name_th: Some("Missing subject".to_string()),
        name_en: None,
        credit: None,
        hours_per_semester: None,
        subject_type: None,
        group_id: None,
        description: None,
        is_active: None,
        start_academic_year_id: None,
        grade_level_ids: None,
        term: None,
        default_instructors: None,
    }
}

#[tokio::test]
async fn missing_subject_mutations_and_instructor_list_return_not_found() {
    let pool = migrated_pool().await;
    let subject_id = Uuid::new_v4();

    assert!(matches!(
        subject_service::update_subject(&pool, subject_id, empty_update()).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        subject_service::delete_subject(&pool, subject_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        subject_service::list_subject_default_instructors(&pool, subject_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        subject_service::remove_subject_default_instructor(&pool, subject_id, Uuid::new_v4()).await,
        Err(AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn updating_a_missing_instructor_assignment_does_not_demote_the_primary() {
    let pool = migrated_pool().await;
    let year_id = Uuid::new_v4();
    let subject_id = Uuid::new_v4();
    let primary_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO academic_years (id, year, name, start_date, end_date)
         VALUES ($1, 9898, 'Subject Contract Test', '9898-01-01', '9898-12-31')",
    )
    .bind(year_id)
    .execute(&pool)
    .await
    .expect("academic year should insert");
    sqlx::query(
        "INSERT INTO subjects (id, code, name_th, type, start_academic_year_id)
         VALUES ($1, $2, 'Subject Contract Test', 'BASIC', $3)",
    )
    .bind(subject_id)
    .bind(format!("T{}", &subject_id.simple().to_string()[..8]))
    .bind(year_id)
    .execute(&pool)
    .await
    .expect("subject should insert");
    sqlx::query(
        "INSERT INTO users (id, password_hash, first_name, last_name, user_type)
         VALUES ($1, 'test', 'Primary', 'Teacher', 'staff')",
    )
    .bind(primary_id)
    .execute(&pool)
    .await
    .expect("primary instructor should insert");
    sqlx::query(
        "INSERT INTO subject_default_instructors (subject_id, instructor_id, role)
         VALUES ($1, $2, 'primary')",
    )
    .bind(subject_id)
    .bind(primary_id)
    .execute(&pool)
    .await
    .expect("primary assignment should insert");

    let result = subject_service::update_subject_default_instructor_role(
        &pool,
        subject_id,
        Uuid::new_v4(),
        "primary",
    )
    .await;

    assert!(matches!(result, Err(AppError::NotFound(_))));
    let role: String = sqlx::query_scalar(
        "SELECT role FROM subject_default_instructors
         WHERE subject_id = $1 AND instructor_id = $2",
    )
    .bind(subject_id)
    .bind(primary_id)
    .fetch_one(&pool)
    .await
    .expect("primary assignment should remain");
    assert_eq!(role, "primary");
}
