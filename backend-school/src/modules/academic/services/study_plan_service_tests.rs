use crate::error::AppError;
use crate::modules::academic::models::study_plans::{
    AddSubjectsToVersionRequest, StudyPlanSubjectQuery, SubjectInPlan, UpdateStudyPlanRequest,
    UpdateStudyPlanVersionRequest,
};
use crate::test_helpers::{create_test_pool, run_test_migrations};
use uuid::Uuid;

use super::study_plan_service;

async fn migrated_pool() -> sqlx::PgPool {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;
    pool
}

fn empty_plan_update() -> UpdateStudyPlanRequest {
    UpdateStudyPlanRequest {
        code: None,
        name_th: Some("Missing plan".to_string()),
        name_en: None,
        description: None,
        grade_level_ids: None,
        is_active: None,
    }
}

fn empty_version_update() -> UpdateStudyPlanVersionRequest {
    UpdateStudyPlanVersionRequest {
        version_name: Some("Missing version".to_string()),
        start_academic_year_id: None,
        end_academic_year_id: None,
        description: None,
        is_active: None,
    }
}

#[tokio::test]
async fn missing_plan_and_version_targets_return_not_found() {
    let pool = migrated_pool().await;
    let plan_id = Uuid::new_v4();
    let version_id = Uuid::new_v4();

    assert!(matches!(
        study_plan_service::get_plan(&pool, plan_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::update_plan(&pool, plan_id, empty_plan_update()).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::delete_plan(&pool, plan_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::get_version(&pool, version_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::update_version(&pool, version_id, empty_version_update()).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::delete_version(&pool, version_id).await,
        Err(AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn missing_plan_subject_parent_and_row_return_not_found() {
    let pool = migrated_pool().await;
    let version_id = Uuid::new_v4();

    assert!(matches!(
        study_plan_service::list_plan_subjects(
            &pool,
            StudyPlanSubjectQuery {
                study_plan_version_id: Some(version_id),
                grade_level_id: None,
                term: None,
            }
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::add_subjects_to_version(
            &pool,
            version_id,
            AddSubjectsToVersionRequest {
                subjects: Vec::new()
            }
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::delete_plan_subject(&pool, Uuid::new_v4()).await,
        Err(AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn add_plan_subjects_reports_only_rows_that_were_inserted() {
    let pool = migrated_pool().await;
    let year_id = Uuid::new_v4();
    let plan_id = Uuid::new_v4();
    let version_id = Uuid::new_v4();
    let subject_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO academic_years (id, year, name, start_date, end_date)
         VALUES ($1, 9797, 'Plan Contract Test', '9797-01-01', '9797-12-31')",
    )
    .bind(year_id)
    .execute(&pool)
    .await
    .expect("academic year should insert");
    sqlx::query(
        "INSERT INTO study_plans (id, code, name_th) VALUES ($1, $2, 'Plan Contract Test')",
    )
    .bind(plan_id)
    .bind(format!("P{}", &plan_id.simple().to_string()[..8]))
    .execute(&pool)
    .await
    .expect("study plan should insert");
    sqlx::query(
        "INSERT INTO study_plan_versions
            (id, study_plan_id, version_name, start_academic_year_id)
         VALUES ($1, $2, 'v1', $3)",
    )
    .bind(version_id)
    .bind(plan_id)
    .bind(year_id)
    .execute(&pool)
    .await
    .expect("study-plan version should insert");
    let grade_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels WHERE level_type = 'primary' AND year = 1")
            .fetch_one(&pool)
            .await
            .expect("seeded grade level should exist");
    sqlx::query(
        "INSERT INTO subjects (id, code, name_th, type, start_academic_year_id)
         VALUES ($1, $2, 'Plan Subject', 'BASIC', $3)",
    )
    .bind(subject_id)
    .bind(format!("S{}", &subject_id.simple().to_string()[..8]))
    .bind(year_id)
    .execute(&pool)
    .await
    .expect("subject should insert");

    let request = || AddSubjectsToVersionRequest {
        subjects: vec![SubjectInPlan {
            grade_level_id: grade_id,
            term: "1".to_string(),
            subject_id,
            display_order: Some(1),
        }],
    };

    assert_eq!(
        study_plan_service::add_subjects_to_version(&pool, version_id, request())
            .await
            .expect("first insert should succeed"),
        1
    );
    assert_eq!(
        study_plan_service::add_subjects_to_version(&pool, version_id, request())
            .await
            .expect("duplicate insert should be skipped"),
        0
    );
}
