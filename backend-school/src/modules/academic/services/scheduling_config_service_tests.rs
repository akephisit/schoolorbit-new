use std::sync::atomic::{AtomicI32, Ordering};

use crate::error::AppError;
use crate::modules::academic::models::scheduling::TimeSlot;
use crate::modules::academic::models::scheduling_config::{
    ClassroomCourseConstraintPatch, ClassroomCoursePreferredRoomsPatch, InstructorConstraintPatch,
    Patch, PreferredRoomInput, SaveSchedulingConfigurationRequest, SchedulerSettingsPatch,
    SubjectConstraintPatch,
};
use crate::test_helpers::{create_test_pool, run_test_migrations};
use sqlx::types::Json;
use uuid::Uuid;

use super::scheduling_config_service;

static NEXT_YEAR: AtomicI32 = AtomicI32::new(18_000);

struct Fixture {
    academic_year_id: Uuid,
    instructor_id: Uuid,
    subject_id: Uuid,
    room_id: Uuid,
    classroom_course_id: Uuid,
    period_id: Uuid,
}

async fn migrated_pool() -> sqlx::PgPool {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;
    pool
}

async fn insert_fixture(pool: &sqlx::PgPool) -> Fixture {
    let year = NEXT_YEAR.fetch_add(1, Ordering::Relaxed);
    let academic_year_id = Uuid::new_v4();
    let semester_id = Uuid::new_v4();
    let study_plan_id = Uuid::new_v4();
    let study_plan_version_id = Uuid::new_v4();
    let classroom_id = Uuid::new_v4();
    let instructor_id = Uuid::new_v4();
    let subject_id = Uuid::new_v4();
    let room_id = Uuid::new_v4();
    let classroom_course_id = Uuid::new_v4();
    let period_id = Uuid::new_v4();
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY created_at, id LIMIT 1")
            .fetch_one(pool)
            .await
            .expect("baseline grade level should exist");

    sqlx::query("UPDATE academic_years SET is_active = false WHERE is_active = true")
        .execute(pool)
        .await
        .expect("existing academic years should deactivate");
    sqlx::query(
        "INSERT INTO academic_years (id, year, name, start_date, end_date, is_active)
         VALUES ($1, $2, $3, '9800-01-01', '9800-12-31', true)",
    )
    .bind(academic_year_id)
    .bind(year)
    .bind(format!("Scheduling Configuration {year}"))
    .execute(pool)
    .await
    .expect("active academic year should insert");
    sqlx::query(
        "INSERT INTO academic_semesters
            (id, academic_year_id, term, name, start_date, end_date)
         VALUES ($1, $2, '1', 'Semester 1', '9800-01-01', '9800-06-30')",
    )
    .bind(semester_id)
    .bind(academic_year_id)
    .execute(pool)
    .await
    .expect("semester should insert");
    sqlx::query("INSERT INTO study_plans (id, code, name_th) VALUES ($1, $2, 'Scheduling Test')")
        .bind(study_plan_id)
        .bind(format!("SC-{study_plan_id}"))
        .execute(pool)
        .await
        .expect("study plan should insert");
    sqlx::query(
        "INSERT INTO study_plan_versions
            (id, study_plan_id, version_name, start_academic_year_id)
         VALUES ($1, $2, 'Scheduling Version', $3)",
    )
    .bind(study_plan_version_id)
    .bind(study_plan_id)
    .bind(academic_year_id)
    .execute(pool)
    .await
    .expect("study plan version should insert");
    sqlx::query(
        "INSERT INTO class_rooms
            (id, code, name, academic_year_id, grade_level_id, study_plan_version_id)
         VALUES ($1, $2, 'Scheduling Classroom', $3, $4, $5)",
    )
    .bind(classroom_id)
    .bind(format!("SC-{classroom_id}"))
    .bind(academic_year_id)
    .bind(grade_level_id)
    .bind(study_plan_version_id)
    .execute(pool)
    .await
    .expect("classroom should insert");
    sqlx::query(
        "INSERT INTO users (id, password_hash, first_name, last_name, user_type, status)
         VALUES ($1, 'test', 'Scheduling', 'Instructor', 'staff', 'active')",
    )
    .bind(instructor_id)
    .execute(pool)
    .await
    .expect("instructor should insert");
    sqlx::query(
        "INSERT INTO subjects
            (id, code, name_th, type, start_academic_year_id, periods_per_week)
         VALUES ($1, $2, 'Scheduling Subject', 'BASIC', $3, 3)",
    )
    .bind(subject_id)
    .bind(format!("SC-{}", &subject_id.to_string()[..8]))
    .bind(academic_year_id)
    .execute(pool)
    .await
    .expect("subject should insert");
    sqlx::query(
        "INSERT INTO rooms (id, name_th, code, status) VALUES ($1, 'Scheduling Room', $2, 'ACTIVE')",
    )
    .bind(room_id)
    .bind(format!("SC-{}", &room_id.to_string()[..8]))
    .execute(pool)
    .await
    .expect("room should insert");
    sqlx::query(
        "INSERT INTO classroom_courses
            (id, classroom_id, subject_id, academic_semester_id, primary_instructor_id)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(classroom_course_id)
    .bind(classroom_id)
    .bind(subject_id)
    .bind(semester_id)
    .bind(instructor_id)
    .execute(pool)
    .await
    .expect("classroom course should insert");
    sqlx::query(
        "INSERT INTO academic_periods
            (id, academic_year_id, name, start_time, end_time, order_index)
         VALUES ($1, $2, 'Period 1', '08:00', '08:50', 1)",
    )
    .bind(period_id)
    .bind(academic_year_id)
    .execute(pool)
    .await
    .expect("period should insert");

    Fixture {
        academic_year_id,
        instructor_id,
        subject_id,
        room_id,
        classroom_course_id,
        period_id,
    }
}

fn slot(period_id: Uuid) -> TimeSlot {
    TimeSlot {
        day: "MON".to_string(),
        period_id,
    }
}

#[tokio::test]
async fn aggregate_validation_failure_writes_nothing() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let request = SaveSchedulingConfigurationRequest {
        scheduler_settings: Some(SchedulerSettingsPatch {
            default_max_consecutive: Patch::Set(7),
        }),
        instructors: vec![InstructorConstraintPatch {
            id: fixture.instructor_id,
            hard_unavailable_slots: Patch::Set(vec![slot(fixture.period_id)]),
            ..Default::default()
        }],
        classroom_courses: vec![ClassroomCourseConstraintPatch {
            id: Uuid::new_v4(),
            consecutive_pattern: Patch::Set(vec![2, 1]),
            ..Default::default()
        }],
        ..Default::default()
    };

    assert!(matches!(
        scheduling_config_service::save_scheduling_configuration(&pool, request).await,
        Err(AppError::NotFound(_))
    ));
    let setting: Json<i32> = sqlx::query_scalar(
        "SELECT value FROM scheduler_settings WHERE key = 'default_max_consecutive'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(setting.0, 4);
    let preference_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM instructor_preferences WHERE instructor_id = $1 AND academic_year_id = $2",
    )
    .bind(fixture.instructor_id)
    .bind(fixture.academic_year_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(preference_count, 0);
}

#[tokio::test]
async fn aggregate_save_is_idempotent_and_clear_resets_values() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let request_json = format!(
        r#"{{
            "scheduler_settings":{{"default_max_consecutive":6}},
            "instructor_order":["{instructor}"],
            "instructors":[{{
                "id":"{instructor}",
                "hard_unavailable_slots":[{{"day":"MON","period_id":"{period}"}}],
                "max_periods_per_day":5,
                "assigned_room_id":"{room}"
            }}],
            "subjects":[{{
                "id":"{subject}",
                "min_consecutive_periods":2,
                "allowed_period_ids":["{period}"],
                "allowed_days":["MON"]
            }}],
            "classroom_courses":[{{
                "id":"{course}",
                "consecutive_pattern":[2,1],
                "same_day_unique":false
            }}],
            "preferred_rooms":[{{
                "classroom_course_id":"{course}",
                "rooms":[{{"room_id":"{room}","rank":1,"is_required":true}}]
            }}]
        }}"#,
        instructor = fixture.instructor_id,
        subject = fixture.subject_id,
        room = fixture.room_id,
        course = fixture.classroom_course_id,
        period = fixture.period_id,
    );
    let first_request: SaveSchedulingConfigurationRequest =
        serde_json::from_str(&request_json).unwrap();
    let first = scheduling_config_service::save_scheduling_configuration(&pool, first_request)
        .await
        .unwrap();
    assert!(first.changed);
    assert!(first.scheduler_settings_changed);
    assert_eq!(first.instructor_order_updated, 1);
    assert_eq!(first.instructor_constraints_updated, 1);
    assert_eq!(first.subject_constraints_updated, 1);
    assert_eq!(first.classroom_course_constraints_updated, 1);
    assert_eq!(first.preferred_room_sets_updated, 1);

    let repeated_request: SaveSchedulingConfigurationRequest =
        serde_json::from_str(&request_json).unwrap();
    let repeated =
        scheduling_config_service::save_scheduling_configuration(&pool, repeated_request)
            .await
            .unwrap();
    assert!(!repeated.changed);

    let clear_request: SaveSchedulingConfigurationRequest = serde_json::from_str(&format!(
        r#"{{
            "scheduler_settings":{{"default_max_consecutive":null}},
            "instructor_order":null,
            "instructors":[{{
                "id":"{}",
                "hard_unavailable_slots":null,
                "max_periods_per_day":null,
                "assigned_room_id":null
            }}],
            "subjects":[{{
                "id":"{}",
                "max_consecutive_periods":null,
                "allowed_period_ids":null,
                "allowed_days":null
            }}],
            "classroom_courses":[{{
                "id":"{}",
                "consecutive_pattern":null,
                "same_day_unique":null,
                "hard_unavailable_slots":null
            }}],
            "preferred_rooms":[{{"classroom_course_id":"{}","rooms":[]}}]
        }}"#,
        fixture.instructor_id,
        fixture.subject_id,
        fixture.classroom_course_id,
        fixture.classroom_course_id,
    ))
    .unwrap();
    let cleared = scheduling_config_service::save_scheduling_configuration(&pool, clear_request)
        .await
        .unwrap();
    assert!(cleared.changed);

    let setting: Json<i32> = sqlx::query_scalar(
        "SELECT value FROM scheduler_settings WHERE key = 'default_max_consecutive'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(setting.0, 4);
    let (hard_slots, max_periods, priority): (Json<Vec<TimeSlot>>, Option<i32>, i32) =
        sqlx::query_as(
            "SELECT hard_unavailable_slots, max_periods_per_day, priority FROM instructor_preferences WHERE instructor_id = $1 AND academic_year_id = $2",
        )
        .bind(fixture.instructor_id)
        .bind(fixture.academic_year_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(hard_slots.0.is_empty());
    assert_eq!(max_periods, Some(7));
    assert_eq!(priority, 100);
    let assigned_room_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM instructor_room_assignments WHERE instructor_id = $1 AND academic_year_id = $2 AND is_required = true",
    )
    .bind(fixture.instructor_id)
    .bind(fixture.academic_year_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(assigned_room_count, 0);
    let preferred_room_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM classroom_course_preferred_rooms WHERE classroom_course_id = $1",
    )
    .bind(fixture.classroom_course_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(preferred_room_count, 0);
}

#[tokio::test]
async fn duplicate_targets_are_rejected_before_settings_change() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let duplicate = SubjectConstraintPatch {
        id: fixture.subject_id,
        min_consecutive_periods: Patch::Set(2),
        ..Default::default()
    };
    let request = SaveSchedulingConfigurationRequest {
        scheduler_settings: Some(SchedulerSettingsPatch {
            default_max_consecutive: Patch::Set(8),
        }),
        subjects: vec![
            duplicate,
            SubjectConstraintPatch {
                id: fixture.subject_id,
                ..Default::default()
            },
        ],
        preferred_rooms: vec![ClassroomCoursePreferredRoomsPatch {
            classroom_course_id: fixture.classroom_course_id,
            rooms: vec![PreferredRoomInput {
                room_id: fixture.room_id,
                rank: 1,
                is_required: false,
            }],
        }],
        ..Default::default()
    };

    assert!(matches!(
        scheduling_config_service::save_scheduling_configuration(&pool, request).await,
        Err(AppError::BadRequest(_))
    ));
    let setting: Json<i32> = sqlx::query_scalar(
        "SELECT value FROM scheduler_settings WHERE key = 'default_max_consecutive'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(setting.0, 4);
}
