use crate::error::AppError;
use crate::test_helpers::{create_test_pool, run_test_migrations};
use uuid::Uuid;

use super::models::{CreateParentRequest, UpdateStudentRequest};
use super::services;

#[tokio::test]
async fn update_missing_student_returns_not_found() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;

    let result = services::update_student(
        &pool,
        Uuid::new_v4(),
        UpdateStudentRequest {
            email: None,
            first_name: Some("Missing".to_string()),
            last_name: None,
            phone: None,
            address: None,
            student_number: None,
        },
    )
    .await;

    assert!(matches!(result, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn delete_missing_student_returns_not_found() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;

    let result = services::delete_student(&pool, Uuid::new_v4()).await;

    assert!(matches!(result, Err(AppError::NotFound(_))));
}

#[tokio::test]
async fn add_parent_to_missing_student_returns_not_found_without_creating_parent() {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;

    let phone = format!("09{}", &Uuid::new_v4().simple().to_string()[..8]);
    let result = services::add_parent_to_student(
        &pool,
        Uuid::new_v4(),
        CreateParentRequest {
            title: None,
            first_name: "Missing".to_string(),
            last_name: "Parent".to_string(),
            phone: phone.clone(),
            relationship: "parent".to_string(),
            national_id: None,
            email: None,
        },
    )
    .await;

    assert!(matches!(result, Err(AppError::NotFound(_))));
    let created: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM users WHERE username = $1)")
            .bind(phone)
            .fetch_one(&pool)
            .await
            .expect("parent existence should load");
    assert!(!created);
}
