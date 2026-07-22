use crate::error::AppError;
use crate::modules::academic::models::study_plans::{
    AddSubjectsToVersionRequest, CatalogDefaultInstructorInput, CreateCatalogRequest,
    CreatePlanActivityRequest, GenerateActivitiesFromPlanRequest, StudyPlanSubjectQuery,
    SubjectInPlan, UpdateCatalogRequest, UpdatePlanActivityRequest, UpdateStudyPlanRequest,
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

fn empty_plan_activity_update() -> UpdatePlanActivityRequest {
    UpdatePlanActivityRequest {
        term: None,
        display_order: Some(1),
    }
}

fn empty_catalog_update() -> UpdateCatalogRequest {
    UpdateCatalogRequest {
        name: None,
        activity_type: None,
        description: None,
        periods_per_week: None,
        scheduling_mode: None,
        is_active: None,
        term: None,
        grade_level_ids: None,
    }
}

async fn insert_activity_template_fixture(
    pool: &sqlx::PgPool,
    year: i32,
) -> (Uuid, Uuid, Uuid, Uuid, Uuid, Uuid) {
    let year_id = Uuid::new_v4();
    let plan_id = Uuid::new_v4();
    let version_id = Uuid::new_v4();
    let catalog_id = Uuid::new_v4();
    let instructor_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO academic_years (id, year, name, start_date, end_date)
         VALUES ($1, $2, $3, '9690-01-01', '9690-12-31')",
    )
    .bind(year_id)
    .bind(year)
    .bind(format!("Activity Template Contract {year}"))
    .execute(pool)
    .await
    .expect("academic year should insert");
    sqlx::query(
        "INSERT INTO study_plans (id, code, name_th) VALUES ($1, $2, 'Activity Template Plan')",
    )
    .bind(plan_id)
    .bind(format!("A{}", &plan_id.simple().to_string()[..8]))
    .execute(pool)
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
    .execute(pool)
    .await
    .expect("study-plan version should insert");
    sqlx::query(
        "INSERT INTO activity_catalog
            (id, name, start_academic_year_id, activity_type)
         VALUES ($1, $2, $3, 'club')",
    )
    .bind(catalog_id)
    .bind(format!("Activity {catalog_id}"))
    .bind(year_id)
    .execute(pool)
    .await
    .expect("activity catalog should insert");
    sqlx::query(
        "INSERT INTO users (id, password_hash, first_name, last_name, user_type)
         VALUES ($1, 'test', 'Activity', 'Teacher', 'staff')",
    )
    .bind(instructor_id)
    .execute(pool)
    .await
    .expect("instructor should insert");
    let grade_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels WHERE level_type = 'primary' AND year = 1")
            .fetch_one(pool)
            .await
            .expect("seeded grade level should exist");

    (
        year_id,
        plan_id,
        version_id,
        catalog_id,
        grade_id,
        instructor_id,
    )
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

#[tokio::test]
async fn missing_activity_template_targets_return_not_found() {
    let pool = migrated_pool().await;
    let missing_id = Uuid::new_v4();
    let missing_instructor_id = Uuid::new_v4();

    assert!(matches!(
        study_plan_service::list_plan_activities(&pool, missing_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::update_plan_activity(&pool, missing_id, empty_plan_activity_update())
            .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::delete_plan_activity(&pool, missing_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::update_activity_catalog(&pool, missing_id, empty_catalog_update())
            .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::delete_activity_catalog(&pool, missing_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::list_catalog_default_instructors(&pool, missing_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::remove_catalog_default_instructor(
            &pool,
            missing_id,
            missing_instructor_id,
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::update_catalog_default_instructor_role(
            &pool,
            missing_id,
            missing_instructor_id,
            "primary",
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::generate_activities_from_plan(
            &pool,
            GenerateActivitiesFromPlanRequest {
                study_plan_version_id: missing_id,
                semester_id: Uuid::new_v4(),
            },
            Some(Uuid::new_v4()),
        )
        .await,
        Err(AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn activity_template_references_must_exist() {
    let pool = migrated_pool().await;
    let (year_id, _plan_id, version_id, catalog_id, grade_id, instructor_id) =
        insert_activity_template_fixture(&pool, 9691).await;

    let request = |activity_catalog_id, grade_level_id| CreatePlanActivityRequest {
        activity_catalog_id,
        grade_level_id,
        term: Some("1".to_string()),
        display_order: Some(1),
    };

    assert!(matches!(
        study_plan_service::add_plan_activity(
            &pool,
            Uuid::new_v4(),
            request(catalog_id, grade_id),
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::add_plan_activity(
            &pool,
            version_id,
            request(Uuid::new_v4(), grade_id),
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::add_plan_activity(
            &pool,
            version_id,
            request(catalog_id, Uuid::new_v4()),
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::generate_activities_from_plan(
            &pool,
            GenerateActivitiesFromPlanRequest {
                study_plan_version_id: version_id,
                semester_id: Uuid::new_v4(),
            },
            Some(instructor_id),
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::create_activity_catalog(
            &pool,
            CreateCatalogRequest {
                name: "Missing Year Activity".to_string(),
                start_academic_year_id: Uuid::new_v4(),
                activity_type: "club".to_string(),
                description: None,
                periods_per_week: None,
                scheduling_mode: None,
                term: None,
                grade_level_ids: None,
                default_instructors: None,
            },
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::create_activity_catalog(
            &pool,
            CreateCatalogRequest {
                name: "Missing Grade Activity".to_string(),
                start_academic_year_id: year_id,
                activity_type: "club".to_string(),
                description: None,
                periods_per_week: None,
                scheduling_mode: None,
                term: None,
                grade_level_ids: Some(vec![Uuid::new_v4()]),
                default_instructors: None,
            },
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::create_activity_catalog(
            &pool,
            CreateCatalogRequest {
                name: "Missing Instructor Activity".to_string(),
                start_academic_year_id: year_id,
                activity_type: "club".to_string(),
                description: None,
                periods_per_week: None,
                scheduling_mode: None,
                term: None,
                grade_level_ids: Some(vec![grade_id]),
                default_instructors: Some(vec![CatalogDefaultInstructorInput {
                    instructor_id: Uuid::new_v4(),
                    role: "secondary".to_string(),
                }]),
            },
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::add_catalog_default_instructor(
            &pool,
            catalog_id,
            Uuid::new_v4(),
            "secondary",
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        study_plan_service::add_catalog_default_instructor(
            &pool,
            Uuid::new_v4(),
            instructor_id,
            "secondary",
        )
        .await,
        Err(AppError::NotFound(_))
    ));

    let year_still_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_years WHERE id = $1)")
            .bind(year_id)
            .fetch_one(&pool)
            .await
            .expect("fixture year lookup should succeed");
    assert!(year_still_exists);
}

#[tokio::test]
async fn updating_a_missing_catalog_instructor_does_not_demote_the_primary() {
    let pool = migrated_pool().await;
    let (_year_id, _plan_id, _version_id, catalog_id, _grade_id, primary_id) =
        insert_activity_template_fixture(&pool, 9692).await;

    sqlx::query(
        "INSERT INTO activity_catalog_default_instructors (catalog_id, instructor_id, role)
         VALUES ($1, $2, 'primary')",
    )
    .bind(catalog_id)
    .bind(primary_id)
    .execute(&pool)
    .await
    .expect("primary assignment should insert");

    let result = study_plan_service::update_catalog_default_instructor_role(
        &pool,
        catalog_id,
        Uuid::new_v4(),
        "primary",
    )
    .await;

    assert!(matches!(result, Err(AppError::NotFound(_))));
    let role: String = sqlx::query_scalar(
        "SELECT role FROM activity_catalog_default_instructors
         WHERE catalog_id = $1 AND instructor_id = $2",
    )
    .bind(catalog_id)
    .bind(primary_id)
    .fetch_one(&pool)
    .await
    .expect("primary assignment should remain");
    assert_eq!(role, "primary");
}
