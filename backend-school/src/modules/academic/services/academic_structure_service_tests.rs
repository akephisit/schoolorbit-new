use crate::error::AppError;
use crate::test_helpers::{create_test_pool, run_test_migrations};
use uuid::Uuid;

use super::academic_structure_service;
use crate::modules::academic::models::UpdateSemesterRequest;

async fn migrated_pool() -> sqlx::PgPool {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;
    pool
}

#[tokio::test]
async fn toggle_missing_year_returns_not_found_without_deactivating_current_year() {
    let pool = migrated_pool().await;
    let active_year_id = Uuid::new_v4();

    sqlx::query("UPDATE academic_years SET is_active = false")
        .execute(&pool)
        .await
        .expect("existing years should deactivate for isolated setup");
    sqlx::query(
        "INSERT INTO academic_years
            (id, year, name, start_date, end_date, is_active)
         VALUES ($1, 9999, 'Contract Test', '9999-01-01', '9999-12-31', true)",
    )
    .bind(active_year_id)
    .execute(&pool)
    .await
    .expect("active academic year should insert");

    let result = academic_structure_service::toggle_active_year(&pool, Uuid::new_v4()).await;

    assert!(matches!(result, Err(AppError::NotFound(_))));
    let remains_active: bool =
        sqlx::query_scalar("SELECT is_active FROM academic_years WHERE id = $1")
            .bind(active_year_id)
            .fetch_one(&pool)
            .await
            .expect("active academic year should still exist");
    assert!(remains_active);
}

#[tokio::test]
async fn delete_missing_semester_returns_not_found() {
    let pool = migrated_pool().await;
    let result = academic_structure_service::delete_semester(&pool, Uuid::new_v4()).await;
    assert!(matches!(result, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn update_missing_semester_returns_not_found() {
    let pool = migrated_pool().await;
    let result = academic_structure_service::update_semester(
        &pool,
        Uuid::new_v4(),
        UpdateSemesterRequest {
            term: Some("1".to_string()),
            name: None,
            start_date: None,
            end_date: None,
            is_active: None,
        },
    )
    .await;
    assert!(matches!(result, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn delete_missing_grade_level_returns_not_found() {
    let pool = migrated_pool().await;
    let result = academic_structure_service::delete_grade_level(&pool, Uuid::new_v4()).await;
    assert!(matches!(result, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn get_levels_for_missing_year_returns_not_found() {
    let pool = migrated_pool().await;
    let result = academic_structure_service::get_year_levels(&pool, Uuid::new_v4()).await;
    assert!(matches!(result, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn update_levels_for_missing_year_returns_not_found() {
    let pool = migrated_pool().await;
    let result =
        academic_structure_service::update_year_levels(&pool, Uuid::new_v4(), Vec::new()).await;
    assert!(matches!(result, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn delete_missing_enrollment_returns_not_found() {
    let pool = migrated_pool().await;
    let result = academic_structure_service::remove_enrollment(&pool, Uuid::new_v4()).await;
    assert!(matches!(result, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn update_number_for_missing_enrollment_returns_not_found() {
    let pool = migrated_pool().await;
    let result =
        academic_structure_service::update_enrollment_number(&pool, Uuid::new_v4(), Some(1)).await;
    assert!(matches!(result, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn list_enrollments_for_missing_classroom_returns_not_found() {
    let pool = migrated_pool().await;
    let result = academic_structure_service::get_class_enrollments(&pool, Uuid::new_v4()).await;
    assert!(matches!(result, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn auto_number_missing_classroom_returns_not_found() {
    let pool = migrated_pool().await;
    let result =
        academic_structure_service::auto_assign_class_numbers(&pool, Uuid::new_v4(), "name").await;
    assert!(matches!(result, Err(AppError::NotFound(_))));
}
