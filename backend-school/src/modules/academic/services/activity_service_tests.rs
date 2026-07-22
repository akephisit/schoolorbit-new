use crate::error::AppError;
use crate::middleware::permission::ActorContext;
use crate::modules::academic::models::activity::{
    BatchUpsertSlotClassroomAssignmentsRequest, UpdateActivitySlotRequest,
    UpsertSlotClassroomAssignmentRequest,
};
use crate::permissions::registry::codes;
use crate::test_helpers::{create_test_pool, run_test_migrations};
use uuid::Uuid;

use super::activity_service;

struct ActivityFixture {
    slot_id: Uuid,
    group_id: Uuid,
    teacher_id: Uuid,
}

async fn migrated_pool() -> sqlx::PgPool {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;
    pool
}

fn manage_all_actor(user_id: Uuid) -> ActorContext {
    ActorContext {
        user_id,
        permissions: vec![codes::ACTIVITY_MANAGE_ALL.to_string()],
    }
}

fn empty_slot_update() -> UpdateActivitySlotRequest {
    UpdateActivitySlotRequest {
        registration_type: None,
        teacher_reg_open: None,
        student_reg_open: None,
        student_reg_start: None,
        student_reg_end: None,
        is_active: Some(false),
    }
}

async fn insert_user(pool: &sqlx::PgPool, user_type: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, password_hash, first_name, last_name, user_type)
         VALUES ($1, 'test', 'Activity', 'Workspace', $2)",
    )
    .bind(id)
    .bind(user_type)
    .execute(pool)
    .await
    .expect("activity user should insert");
    id
}

async fn insert_activity_fixture(pool: &sqlx::PgPool, year: i32) -> ActivityFixture {
    let year_id = Uuid::new_v4();
    let semester_id = Uuid::new_v4();
    let catalog_id = Uuid::new_v4();
    let slot_id = Uuid::new_v4();
    let group_id = Uuid::new_v4();
    let teacher_id = insert_user(pool, "staff").await;

    sqlx::query(
        "INSERT INTO academic_years (id, year, name, start_date, end_date)
         VALUES ($1, $2, $3, '9800-01-01', '9800-12-31')",
    )
    .bind(year_id)
    .bind(year)
    .bind(format!("Activity Workspace {year}"))
    .execute(pool)
    .await
    .expect("academic year should insert");
    sqlx::query(
        "INSERT INTO academic_semesters
            (id, academic_year_id, term, name, start_date, end_date)
         VALUES ($1, $2, '1', 'Semester 1', '9800-01-01', '9800-06-30')",
    )
    .bind(semester_id)
    .bind(year_id)
    .execute(pool)
    .await
    .expect("semester should insert");
    sqlx::query(
        "INSERT INTO activity_catalog
            (id, name, start_academic_year_id, activity_type)
         VALUES ($1, $2, $3, 'club')",
    )
    .bind(catalog_id)
    .bind(format!("Workspace {catalog_id}"))
    .bind(year_id)
    .execute(pool)
    .await
    .expect("activity catalog should insert");
    sqlx::query(
        "INSERT INTO activity_slots
            (id, activity_catalog_id, semester_id, registration_type,
             teacher_reg_open, student_reg_open)
         VALUES ($1, $2, $3, 'self', true, true)",
    )
    .bind(slot_id)
    .bind(catalog_id)
    .bind(semester_id)
    .execute(pool)
    .await
    .expect("activity slot should insert");
    sqlx::query(
        "INSERT INTO activity_groups
            (id, slot_id, name, instructor_id, created_by, registration_open)
         VALUES ($1, $2, 'Workspace Group', $3, $3, true)",
    )
    .bind(group_id)
    .bind(slot_id)
    .bind(teacher_id)
    .execute(pool)
    .await
    .expect("activity group should insert");

    ActivityFixture {
        slot_id,
        group_id,
        teacher_id,
    }
}

#[tokio::test]
async fn missing_activity_slot_targets_return_not_found() {
    let pool = migrated_pool().await;
    let slot_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();

    assert!(matches!(
        activity_service::update_slot(&pool, slot_id, empty_slot_update()).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        activity_service::delete_slot(&pool, slot_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        activity_service::list_slot_instructors(&pool, slot_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        activity_service::add_slot_instructor(&pool, slot_id, user_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        activity_service::add_slot_instructors_batch(&pool, slot_id, vec![]).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        activity_service::remove_slot_instructor(&pool, slot_id, user_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        activity_service::delete_slot_timetable_entries(&pool, slot_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        activity_service::delete_all_slot_groups(&pool, slot_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        activity_service::remove_all_slot_instructors(&pool, slot_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        activity_service::list_slot_classroom_assignments(&pool, slot_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        activity_service::delete_all_slot_classroom_assignments(&pool, slot_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        activity_service::delete_slot_classroom_assignment(&pool, slot_id, Uuid::new_v4()).await,
        Err(AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn slot_instructor_batch_reports_actual_inserts_and_validates_users() {
    let pool = migrated_pool().await;
    let fixture = insert_activity_fixture(&pool, 9811).await;
    let second_teacher = insert_user(&pool, "staff").await;

    sqlx::query("INSERT INTO activity_slot_instructors (slot_id, user_id) VALUES ($1, $2)")
        .bind(fixture.slot_id)
        .bind(fixture.teacher_id)
        .execute(&pool)
        .await
        .expect("existing slot instructor should insert");

    let added = activity_service::add_slot_instructors_batch(
        &pool,
        fixture.slot_id,
        vec![fixture.teacher_id, second_teacher, second_teacher],
    )
    .await
    .expect("batch should succeed");
    assert_eq!(added, 1);

    assert!(matches!(
        activity_service::add_slot_instructor(&pool, fixture.slot_id, Uuid::new_v4()).await,
        Err(AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn missing_group_children_and_non_student_self_enrollment_are_rejected() {
    let pool = migrated_pool().await;
    let fixture = insert_activity_fixture(&pool, 9812).await;
    let actor = manage_all_actor(fixture.teacher_id);
    let missing_user_id = Uuid::new_v4();

    assert!(matches!(
        activity_service::remove_group_instructor(
            &pool,
            &actor,
            fixture.group_id,
            missing_user_id,
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        activity_service::self_unenroll(&pool, fixture.group_id, missing_user_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        activity_service::remove_member(&pool, fixture.group_id, missing_user_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        activity_service::update_member_result(&pool, Uuid::new_v4(), "pass").await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        activity_service::add_group_instructor(
            &pool,
            &actor,
            fixture.group_id,
            fixture.teacher_id,
            "owner",
        )
        .await,
        Err(AppError::BadRequest(_))
    ));
    assert!(matches!(
        activity_service::self_enroll(&pool, fixture.group_id, fixture.teacher_id).await,
        Err(AppError::BadRequest(_))
    ));
}

#[tokio::test]
async fn classroom_assignment_targets_must_exist() {
    let pool = migrated_pool().await;
    let fixture = insert_activity_fixture(&pool, 9813).await;

    assert!(matches!(
        activity_service::batch_upsert_slot_classroom_assignments(
            &pool,
            Uuid::new_v4(),
            BatchUpsertSlotClassroomAssignmentsRequest {
                assignments: vec![],
            },
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        activity_service::batch_upsert_slot_classroom_assignments(
            &pool,
            fixture.slot_id,
            BatchUpsertSlotClassroomAssignmentsRequest {
                assignments: vec![UpsertSlotClassroomAssignmentRequest {
                    classroom_id: Uuid::new_v4(),
                    instructor_id: Uuid::new_v4(),
                }],
            },
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        activity_service::delete_slot_classroom_assignment(&pool, fixture.slot_id, Uuid::new_v4(),)
            .await,
        Err(AppError::NotFound(_))
    ));
}

#[tokio::test]
async fn invalid_activity_registration_type_is_rejected_before_update() {
    let pool = migrated_pool().await;
    let fixture = insert_activity_fixture(&pool, 9814).await;
    let mut body = empty_slot_update();
    body.registration_type = Some("lottery".to_string());

    assert!(matches!(
        activity_service::update_slot(&pool, fixture.slot_id, body).await,
        Err(AppError::BadRequest(_))
    ));
}
