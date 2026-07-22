use std::sync::atomic::{AtomicI32, Ordering};

use crate::error::AppError;
use crate::modules::academic::models::course_planning::{
    AssignCoursesRequest, ClassroomCourseSettings, OptionalUuidPatch, PlanQuery,
    UpdateCourseRequest,
};
use crate::test_helpers::{create_test_pool, run_test_migrations};
use uuid::Uuid;

use super::course_planning_service;

static NEXT_YEAR: AtomicI32 = AtomicI32::new(12_000);

struct CoursePlanningFixture {
    semester_id: Uuid,
    classroom_id: Uuid,
    subject_id: Uuid,
    second_subject_id: Uuid,
    course_id: Uuid,
    primary_instructor_id: Uuid,
    second_instructor_id: Uuid,
    period_id: Uuid,
}

async fn migrated_pool() -> sqlx::PgPool {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;
    pool
}

async fn insert_user(pool: &sqlx::PgPool, label: &str) -> Uuid {
    let user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, password_hash, first_name, last_name, user_type, status)
         VALUES ($1, 'test', $2, 'Course', 'staff', 'active')",
    )
    .bind(user_id)
    .bind(label)
    .execute(pool)
    .await
    .expect("course-planning user should insert");
    user_id
}

async fn insert_fixture(pool: &sqlx::PgPool) -> CoursePlanningFixture {
    let year = NEXT_YEAR.fetch_add(1, Ordering::Relaxed);
    let year_id = Uuid::new_v4();
    let semester_id = Uuid::new_v4();
    let study_plan_id = Uuid::new_v4();
    let study_plan_version_id = Uuid::new_v4();
    let classroom_id = Uuid::new_v4();
    let subject_id = Uuid::new_v4();
    let second_subject_id = Uuid::new_v4();
    let course_id = Uuid::new_v4();
    let period_id = Uuid::new_v4();
    let primary_instructor_id = insert_user(pool, "Primary").await;
    let second_instructor_id = insert_user(pool, "Secondary").await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY created_at, id LIMIT 1")
            .fetch_one(pool)
            .await
            .expect("baseline grade level should exist");

    sqlx::query(
        "INSERT INTO academic_years (id, year, name, start_date, end_date)
         VALUES ($1, $2, $3, '9800-01-01', '9800-12-31')",
    )
    .bind(year_id)
    .bind(year)
    .bind(format!("Course Planning {year}"))
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
    sqlx::query("INSERT INTO study_plans (id, code, name_th) VALUES ($1, $2, 'Course Plan Test')")
        .bind(study_plan_id)
        .bind(format!("CP-{study_plan_id}"))
        .execute(pool)
        .await
        .expect("study plan should insert");
    sqlx::query(
        "INSERT INTO study_plan_versions
            (id, study_plan_id, version_name, start_academic_year_id)
         VALUES ($1, $2, 'Test Version', $3)",
    )
    .bind(study_plan_version_id)
    .bind(study_plan_id)
    .bind(year_id)
    .execute(pool)
    .await
    .expect("study-plan version should insert");
    sqlx::query(
        "INSERT INTO class_rooms
            (id, code, name, academic_year_id, grade_level_id, study_plan_version_id)
         VALUES ($1, $2, 'Course Planning Room', $3, $4, $5)",
    )
    .bind(classroom_id)
    .bind(format!("CP-{classroom_id}"))
    .bind(year_id)
    .bind(grade_level_id)
    .bind(study_plan_version_id)
    .execute(pool)
    .await
    .expect("classroom should insert");
    for (id, code) in [(subject_id, "ONE"), (second_subject_id, "TWO")] {
        sqlx::query(
            "INSERT INTO subjects (id, code, name_th, type, start_academic_year_id)
             VALUES ($1, $2, $3, 'BASIC', $4)",
        )
        .bind(id)
        .bind(format!("{code}-{}", &id.to_string()[..8]))
        .bind(format!("Course Subject {code}"))
        .bind(year_id)
        .execute(pool)
        .await
        .expect("subject should insert");
    }
    sqlx::query(
        "INSERT INTO classroom_courses
            (id, classroom_id, subject_id, academic_semester_id, primary_instructor_id)
         VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(course_id)
    .bind(classroom_id)
    .bind(subject_id)
    .bind(semester_id)
    .bind(primary_instructor_id)
    .execute(pool)
    .await
    .expect("classroom course should insert");
    sqlx::query(
        "INSERT INTO academic_periods
            (id, academic_year_id, name, start_time, end_time, order_index)
         VALUES ($1, $2, 'Period 1', '08:00', '08:50', 1)",
    )
    .bind(period_id)
    .bind(year_id)
    .execute(pool)
    .await
    .expect("period should insert");

    CoursePlanningFixture {
        semester_id,
        classroom_id,
        subject_id,
        second_subject_id,
        course_id,
        primary_instructor_id,
        second_instructor_id,
        period_id,
    }
}

#[tokio::test]
async fn course_assignment_validates_every_target_and_reports_actual_inserts() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;

    assert!(matches!(
        course_planning_service::assign_courses(
            &pool,
            AssignCoursesRequest {
                classroom_id: fixture.classroom_id,
                academic_semester_id: Uuid::new_v4(),
                subject_ids: vec![fixture.second_subject_id],
            },
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        course_planning_service::assign_courses(
            &pool,
            AssignCoursesRequest {
                classroom_id: fixture.classroom_id,
                academic_semester_id: fixture.semester_id,
                subject_ids: vec![fixture.second_subject_id, Uuid::new_v4()],
            },
        )
        .await,
        Err(AppError::NotFound(_))
    ));

    let inserted = course_planning_service::assign_courses(
        &pool,
        AssignCoursesRequest {
            classroom_id: fixture.classroom_id,
            academic_semester_id: fixture.semester_id,
            subject_ids: vec![fixture.second_subject_id, fixture.second_subject_id],
        },
    )
    .await
    .expect("valid distinct subjects should assign");
    assert_eq!(inserted, 1);

    let inserted_again = course_planning_service::assign_courses(
        &pool,
        AssignCoursesRequest {
            classroom_id: fixture.classroom_id,
            academic_semester_id: fixture.semester_id,
            subject_ids: vec![fixture.second_subject_id],
        },
    )
    .await
    .expect("existing assignment should be idempotent");
    assert_eq!(inserted_again, 0);
}

#[tokio::test]
async fn missing_course_and_instructor_targets_return_not_found() {
    let pool = migrated_pool().await;
    let missing_course_id = Uuid::new_v4();
    let missing_instructor_id = Uuid::new_v4();

    assert!(matches!(
        course_planning_service::update_course(
            &pool,
            missing_course_id,
            UpdateCourseRequest {
                primary_instructor_id: OptionalUuidPatch::Unspecified,
                settings: None,
            },
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        course_planning_service::remove_course(&pool, missing_course_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        course_planning_service::list_course_instructors(&pool, missing_course_id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        course_planning_service::add_course_instructor(
            &pool,
            missing_course_id,
            missing_instructor_id,
            "secondary",
        )
        .await,
        Err(AppError::NotFound(_))
    ));
}

#[test]
fn primary_instructor_patch_distinguishes_omitted_null_and_uuid() {
    let instructor_id = Uuid::new_v4();
    let omitted: UpdateCourseRequest =
        serde_json::from_str("{}").expect("omitted patch should deserialize");
    let cleared: UpdateCourseRequest = serde_json::from_str(r#"{"primary_instructor_id":null}"#)
        .expect("null patch should deserialize");
    let assigned: UpdateCourseRequest = serde_json::from_value(serde_json::json!({
        "primary_instructor_id": instructor_id,
    }))
    .expect("UUID patch should deserialize");

    assert!(matches!(
        omitted.primary_instructor_id,
        OptionalUuidPatch::Unspecified
    ));
    assert!(matches!(
        cleared.primary_instructor_id,
        OptionalUuidPatch::Null
    ));
    assert!(matches!(
        assigned.primary_instructor_id,
        OptionalUuidPatch::Value(id) if id == instructor_id
    ));
}

#[tokio::test]
async fn course_update_assigns_and_clears_primary_team_transactionally() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let entry_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO academic_timetable_entries
            (id, classroom_course_id, day_of_week, period_id, classroom_id,
             academic_semester_id)
         VALUES ($1, $2, 'TUE', $3, $4, $5)",
    )
    .bind(entry_id)
    .bind(fixture.course_id)
    .bind(fixture.period_id)
    .bind(fixture.classroom_id)
    .bind(fixture.semester_id)
    .execute(&pool)
    .await
    .expect("timetable entry should insert");
    sqlx::query(
        "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
         VALUES ($1, $2, 'primary')",
    )
    .bind(entry_id)
    .bind(fixture.primary_instructor_id)
    .execute(&pool)
    .await
    .expect("current primary should be attached to timetable");

    course_planning_service::update_course(
        &pool,
        fixture.course_id,
        UpdateCourseRequest {
            primary_instructor_id: OptionalUuidPatch::Unspecified,
            settings: Some(
                serde_json::from_value::<ClassroomCourseSettings>(
                    serde_json::json!({ "display": "compact" }),
                )
                .expect("object settings should deserialize"),
            ),
        },
    )
    .await
    .expect("settings-only update should keep the teaching team");
    let primary_after_settings: Option<Uuid> =
        sqlx::query_scalar("SELECT primary_instructor_id FROM classroom_courses WHERE id = $1")
            .bind(fixture.course_id)
            .fetch_one(&pool)
            .await
            .expect("course should remain after settings update");
    assert_eq!(primary_after_settings, Some(fixture.primary_instructor_id));

    course_planning_service::update_course(
        &pool,
        fixture.course_id,
        UpdateCourseRequest {
            primary_instructor_id: OptionalUuidPatch::Value(fixture.second_instructor_id),
            settings: None,
        },
    )
    .await
    .expect("new primary should be assigned");

    let promoted_role: Option<String> = sqlx::query_scalar(
        "SELECT role FROM timetable_entry_instructors
         WHERE entry_id = $1 AND instructor_id = $2",
    )
    .bind(entry_id)
    .bind(fixture.second_instructor_id)
    .fetch_optional(&pool)
    .await
    .expect("promoted timetable role should load");
    assert_eq!(promoted_role.as_deref(), Some("primary"));

    course_planning_service::update_course(
        &pool,
        fixture.course_id,
        UpdateCourseRequest {
            primary_instructor_id: OptionalUuidPatch::Null,
            settings: None,
        },
    )
    .await
    .expect("primary should clear");

    let primary_after_clear: Option<Uuid> =
        sqlx::query_scalar("SELECT primary_instructor_id FROM classroom_courses WHERE id = $1")
            .bind(fixture.course_id)
            .fetch_one(&pool)
            .await
            .expect("course should remain");
    assert_eq!(primary_after_clear, Some(fixture.primary_instructor_id));
    let remaining_primary_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM classroom_course_instructors
         WHERE classroom_course_id = $1 AND role = 'primary'",
    )
    .bind(fixture.course_id)
    .fetch_one(&pool)
    .await
    .expect("course team should remain queryable");
    assert_eq!(remaining_primary_count, 1);
    let cleared_timetable_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM timetable_entry_instructors
         WHERE entry_id = $1 AND instructor_id = $2",
    )
    .bind(entry_id)
    .bind(fixture.second_instructor_id)
    .fetch_one(&pool)
    .await
    .expect("timetable team should remain queryable");
    assert_eq!(cleared_timetable_count, 0);

    let retained_course_role: String = sqlx::query_scalar(
        "SELECT role FROM classroom_course_instructors
         WHERE classroom_course_id = $1 AND instructor_id = $2",
    )
    .bind(fixture.course_id)
    .bind(fixture.primary_instructor_id)
    .fetch_one(&pool)
    .await
    .expect("previous instructor should remain as a secondary team member");
    assert_eq!(retained_course_role, "primary");
    let retained_timetable_role: String = sqlx::query_scalar(
        "SELECT role FROM timetable_entry_instructors
         WHERE entry_id = $1 AND instructor_id = $2",
    )
    .bind(entry_id)
    .bind(fixture.primary_instructor_id)
    .fetch_one(&pool)
    .await
    .expect("previous instructor should remain on the timetable entry");
    assert_eq!(retained_timetable_role, retained_course_role);
}

#[tokio::test]
async fn course_instructor_mutations_validate_role_and_assignment_targets() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;

    assert!(matches!(
        course_planning_service::add_course_instructor(
            &pool,
            fixture.course_id,
            Uuid::new_v4(),
            "secondary",
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        course_planning_service::add_course_instructor(
            &pool,
            fixture.course_id,
            fixture.second_instructor_id,
            "assistant",
        )
        .await,
        Err(AppError::BadRequest(_))
    ));
    assert!(matches!(
        course_planning_service::remove_course_instructor(
            &pool,
            fixture.course_id,
            fixture.second_instructor_id,
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        course_planning_service::update_course_instructor_role(
            &pool,
            fixture.course_id,
            fixture.second_instructor_id,
            "primary",
        )
        .await,
        Err(AppError::NotFound(_))
    ));

    let primary_after: Option<Uuid> =
        sqlx::query_scalar("SELECT primary_instructor_id FROM classroom_courses WHERE id = $1")
            .bind(fixture.course_id)
            .fetch_one(&pool)
            .await
            .expect("course should still exist");
    assert_eq!(primary_after, Some(fixture.primary_instructor_id));
}

#[tokio::test]
async fn promoting_course_instructor_synchronizes_existing_timetable_team() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let entry_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO academic_timetable_entries
            (id, classroom_course_id, day_of_week, period_id, classroom_id,
             academic_semester_id)
         VALUES ($1, $2, 'MON', $3, $4, $5)",
    )
    .bind(entry_id)
    .bind(fixture.course_id)
    .bind(fixture.period_id)
    .bind(fixture.classroom_id)
    .bind(fixture.semester_id)
    .execute(&pool)
    .await
    .expect("timetable entry should insert");
    sqlx::query(
        "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
         VALUES ($1, $2, 'primary')",
    )
    .bind(entry_id)
    .bind(fixture.primary_instructor_id)
    .execute(&pool)
    .await
    .expect("primary timetable instructor should insert");

    course_planning_service::add_course_instructor(
        &pool,
        fixture.course_id,
        fixture.second_instructor_id,
        "secondary",
    )
    .await
    .expect("secondary instructor should be added");
    course_planning_service::update_course_instructor_role(
        &pool,
        fixture.course_id,
        fixture.second_instructor_id,
        "primary",
    )
    .await
    .expect("secondary instructor should become primary");

    let course_primary: Option<Uuid> =
        sqlx::query_scalar("SELECT primary_instructor_id FROM classroom_courses WHERE id = $1")
            .bind(fixture.course_id)
            .fetch_one(&pool)
            .await
            .expect("course should exist");
    assert_eq!(course_primary, Some(fixture.second_instructor_id));

    let timetable_roles: Vec<(Uuid, String)> = sqlx::query_as(
        "SELECT instructor_id, role FROM timetable_entry_instructors
         WHERE entry_id = $1 ORDER BY instructor_id",
    )
    .bind(entry_id)
    .fetch_all(&pool)
    .await
    .expect("timetable instructor roles should load");
    assert!(timetable_roles.contains(&(fixture.primary_instructor_id, "secondary".to_string())));
    assert!(timetable_roles.contains(&(fixture.second_instructor_id, "primary".to_string())));
}

#[tokio::test]
async fn removing_primary_synchronizes_the_database_promoted_timetable_role() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let entry_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO academic_timetable_entries
            (id, classroom_course_id, day_of_week, period_id, classroom_id,
             academic_semester_id)
         VALUES ($1, $2, 'WED', $3, $4, $5)",
    )
    .bind(entry_id)
    .bind(fixture.course_id)
    .bind(fixture.period_id)
    .bind(fixture.classroom_id)
    .bind(fixture.semester_id)
    .execute(&pool)
    .await
    .expect("timetable entry should insert");
    sqlx::query(
        "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
         VALUES ($1, $2, 'primary')",
    )
    .bind(entry_id)
    .bind(fixture.primary_instructor_id)
    .execute(&pool)
    .await
    .expect("primary timetable instructor should insert");

    course_planning_service::add_course_instructor(
        &pool,
        fixture.course_id,
        fixture.second_instructor_id,
        "secondary",
    )
    .await
    .expect("secondary instructor should be added");
    sqlx::query(
        "DELETE FROM timetable_entry_instructors
         WHERE entry_id = $1 AND instructor_id = $2",
    )
    .bind(entry_id)
    .bind(fixture.second_instructor_id)
    .execute(&pool)
    .await
    .expect("legacy partial timetable assignment should be simulated");
    course_planning_service::remove_course_instructor(
        &pool,
        fixture.course_id,
        fixture.primary_instructor_id,
    )
    .await
    .expect("current primary should be removed");

    let course_role: String = sqlx::query_scalar(
        "SELECT role FROM classroom_course_instructors
         WHERE classroom_course_id = $1 AND instructor_id = $2",
    )
    .bind(fixture.course_id)
    .bind(fixture.second_instructor_id)
    .fetch_one(&pool)
    .await
    .expect("remaining course instructor should exist");
    assert_eq!(course_role, "primary");

    let timetable_role: String = sqlx::query_scalar(
        "SELECT role FROM timetable_entry_instructors
         WHERE entry_id = $1 AND instructor_id = $2",
    )
    .bind(entry_id)
    .bind(fixture.second_instructor_id)
    .fetch_one(&pool)
    .await
    .expect("remaining timetable instructor should exist");
    assert_eq!(timetable_role, course_role);
}

#[tokio::test]
async fn clearing_primary_fills_a_missing_promoted_timetable_assignment() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let entry_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO academic_timetable_entries
            (id, classroom_course_id, day_of_week, period_id, classroom_id,
             academic_semester_id)
         VALUES ($1, $2, 'TUE', $3, $4, $5)",
    )
    .bind(entry_id)
    .bind(fixture.course_id)
    .bind(fixture.period_id)
    .bind(fixture.classroom_id)
    .bind(fixture.semester_id)
    .execute(&pool)
    .await
    .expect("timetable entry should insert");
    sqlx::query(
        "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
         VALUES ($1, $2, 'primary')",
    )
    .bind(entry_id)
    .bind(fixture.primary_instructor_id)
    .execute(&pool)
    .await
    .expect("current primary timetable assignment should insert");
    course_planning_service::add_course_instructor(
        &pool,
        fixture.course_id,
        fixture.second_instructor_id,
        "secondary",
    )
    .await
    .expect("secondary instructor should be added");
    sqlx::query(
        "DELETE FROM timetable_entry_instructors
         WHERE entry_id = $1 AND instructor_id = $2",
    )
    .bind(entry_id)
    .bind(fixture.second_instructor_id)
    .execute(&pool)
    .await
    .expect("legacy partial timetable assignment should be simulated");

    course_planning_service::update_course(
        &pool,
        fixture.course_id,
        UpdateCourseRequest {
            primary_instructor_id: OptionalUuidPatch::Null,
            settings: None,
        },
    )
    .await
    .expect("clearing the primary should attach the promoted replacement");

    let timetable_role: String = sqlx::query_scalar(
        "SELECT role FROM timetable_entry_instructors
         WHERE entry_id = $1 AND instructor_id = $2",
    )
    .bind(entry_id)
    .bind(fixture.second_instructor_id)
    .fetch_one(&pool)
    .await
    .expect("promoted replacement should be restored to the timetable entry");
    assert_eq!(timetable_role, "primary");
}

#[tokio::test]
async fn adding_the_first_secondary_uses_the_database_promoted_timetable_role() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let entry_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO academic_timetable_entries
            (id, classroom_course_id, day_of_week, period_id, classroom_id,
             academic_semester_id)
         VALUES ($1, $2, 'FRI', $3, $4, $5)",
    )
    .bind(entry_id)
    .bind(fixture.course_id)
    .bind(fixture.period_id)
    .bind(fixture.classroom_id)
    .bind(fixture.semester_id)
    .execute(&pool)
    .await
    .expect("timetable entry should insert");
    course_planning_service::update_course(
        &pool,
        fixture.course_id,
        UpdateCourseRequest {
            primary_instructor_id: OptionalUuidPatch::Null,
            settings: None,
        },
    )
    .await
    .expect("existing primary should clear before adding the first team member");

    course_planning_service::add_course_instructor(
        &pool,
        fixture.course_id,
        fixture.second_instructor_id,
        "secondary",
    )
    .await
    .expect("first secondary request should add the instructor");

    let course_role: String = sqlx::query_scalar(
        "SELECT role FROM classroom_course_instructors
         WHERE classroom_course_id = $1 AND instructor_id = $2",
    )
    .bind(fixture.course_id)
    .bind(fixture.second_instructor_id)
    .fetch_one(&pool)
    .await
    .expect("new course instructor should exist");
    assert_eq!(course_role, "primary");
    let timetable_role: String = sqlx::query_scalar(
        "SELECT role FROM timetable_entry_instructors
         WHERE entry_id = $1 AND instructor_id = $2",
    )
    .bind(entry_id)
    .bind(fixture.second_instructor_id)
    .fetch_one(&pool)
    .await
    .expect("new timetable instructor should exist");
    assert_eq!(timetable_role, course_role);
}

#[tokio::test]
async fn adding_an_instructor_with_a_timetable_conflict_rolls_back_the_team_change() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let course_entry_id = Uuid::new_v4();
    let conflicting_entry_id = Uuid::new_v4();

    for entry_id in [course_entry_id, conflicting_entry_id] {
        sqlx::query(
            "INSERT INTO academic_timetable_entries
                (id, classroom_course_id, day_of_week, period_id, classroom_id,
                 academic_semester_id)
             VALUES ($1, $2, 'THU', $3, $4, $5)",
        )
        .bind(entry_id)
        .bind(if entry_id == course_entry_id {
            Some(fixture.course_id)
        } else {
            None
        })
        .bind(fixture.period_id)
        .bind(if entry_id == course_entry_id {
            Some(fixture.classroom_id)
        } else {
            None
        })
        .bind(fixture.semester_id)
        .execute(&pool)
        .await
        .expect("timetable entry should insert");
    }
    sqlx::query(
        "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
         VALUES ($1, $2, 'primary')",
    )
    .bind(conflicting_entry_id)
    .bind(fixture.second_instructor_id)
    .execute(&pool)
    .await
    .expect("conflicting timetable assignment should insert");

    assert!(matches!(
        course_planning_service::add_course_instructor(
            &pool,
            fixture.course_id,
            fixture.second_instructor_id,
            "secondary",
        )
        .await,
        Err(AppError::Conflict(_))
    ));

    let course_assignment_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM classroom_course_instructors
         WHERE classroom_course_id = $1 AND instructor_id = $2",
    )
    .bind(fixture.course_id)
    .bind(fixture.second_instructor_id)
    .fetch_one(&pool)
    .await
    .expect("course assignment count should load");
    assert_eq!(course_assignment_count, 0);
}

#[tokio::test]
async fn updating_the_course_primary_maps_timetable_conflict_and_rolls_back() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let course_entry_id = Uuid::new_v4();
    let conflicting_entry_id = Uuid::new_v4();

    for entry_id in [course_entry_id, conflicting_entry_id] {
        sqlx::query(
            "INSERT INTO academic_timetable_entries
                (id, classroom_course_id, day_of_week, period_id, classroom_id,
                 academic_semester_id)
             VALUES ($1, $2, 'SAT', $3, $4, $5)",
        )
        .bind(entry_id)
        .bind(if entry_id == course_entry_id {
            Some(fixture.course_id)
        } else {
            None
        })
        .bind(fixture.period_id)
        .bind(if entry_id == course_entry_id {
            Some(fixture.classroom_id)
        } else {
            None
        })
        .bind(fixture.semester_id)
        .execute(&pool)
        .await
        .expect("timetable entry should insert");
    }
    sqlx::query(
        "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
         VALUES ($1, $2, 'primary')",
    )
    .bind(conflicting_entry_id)
    .bind(fixture.second_instructor_id)
    .execute(&pool)
    .await
    .expect("conflicting timetable assignment should insert");

    assert!(matches!(
        course_planning_service::update_course(
            &pool,
            fixture.course_id,
            UpdateCourseRequest {
                primary_instructor_id: OptionalUuidPatch::Value(fixture.second_instructor_id),
                settings: None,
            },
        )
        .await,
        Err(AppError::Conflict(_))
    ));

    let course_primary: Option<Uuid> =
        sqlx::query_scalar("SELECT primary_instructor_id FROM classroom_courses WHERE id = $1")
            .bind(fixture.course_id)
            .fetch_one(&pool)
            .await
            .expect("course primary should load");
    assert_eq!(course_primary, Some(fixture.primary_instructor_id));
}

#[tokio::test]
async fn promoting_an_instructor_fills_missing_timetable_assignments() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;

    course_planning_service::add_course_instructor(
        &pool,
        fixture.course_id,
        fixture.second_instructor_id,
        "secondary",
    )
    .await
    .expect("secondary instructor should be added before scheduling");

    let entry_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO academic_timetable_entries
            (id, classroom_course_id, day_of_week, period_id, classroom_id,
             academic_semester_id)
         VALUES ($1, $2, 'SUN', $3, $4, $5)",
    )
    .bind(entry_id)
    .bind(fixture.course_id)
    .bind(fixture.period_id)
    .bind(fixture.classroom_id)
    .bind(fixture.semester_id)
    .execute(&pool)
    .await
    .expect("timetable entry should insert");
    sqlx::query(
        "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
         VALUES ($1, $2, 'primary')",
    )
    .bind(entry_id)
    .bind(fixture.primary_instructor_id)
    .execute(&pool)
    .await
    .expect("current primary timetable assignment should insert");

    course_planning_service::update_course_instructor_role(
        &pool,
        fixture.course_id,
        fixture.second_instructor_id,
        "primary",
    )
    .await
    .expect("promotion should fill the instructor's missing timetable assignment");

    let promoted_role: String = sqlx::query_scalar(
        "SELECT role FROM timetable_entry_instructors
         WHERE entry_id = $1 AND instructor_id = $2",
    )
    .bind(entry_id)
    .bind(fixture.second_instructor_id)
    .fetch_one(&pool)
    .await
    .expect("promoted instructor should be added to the timetable entry");
    assert_eq!(promoted_role, "primary");
}

#[tokio::test]
async fn demoting_the_primary_promotes_another_instructor_and_synchronizes_timetable_roles() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let entry_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO academic_timetable_entries
            (id, classroom_course_id, day_of_week, period_id, classroom_id,
             academic_semester_id)
         VALUES ($1, $2, 'MON', $3, $4, $5)",
    )
    .bind(entry_id)
    .bind(fixture.course_id)
    .bind(fixture.period_id)
    .bind(fixture.classroom_id)
    .bind(fixture.semester_id)
    .execute(&pool)
    .await
    .expect("timetable entry should insert");
    sqlx::query(
        "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
         VALUES ($1, $2, 'primary')",
    )
    .bind(entry_id)
    .bind(fixture.primary_instructor_id)
    .execute(&pool)
    .await
    .expect("current primary timetable assignment should insert");
    course_planning_service::add_course_instructor(
        &pool,
        fixture.course_id,
        fixture.second_instructor_id,
        "secondary",
    )
    .await
    .expect("secondary instructor should be added");

    course_planning_service::update_course_instructor_role(
        &pool,
        fixture.course_id,
        fixture.primary_instructor_id,
        "secondary",
    )
    .await
    .expect("another instructor should replace the demoted primary");

    let course_primary: Option<Uuid> =
        sqlx::query_scalar("SELECT primary_instructor_id FROM classroom_courses WHERE id = $1")
            .bind(fixture.course_id)
            .fetch_one(&pool)
            .await
            .expect("course primary should load");
    assert_eq!(course_primary, Some(fixture.second_instructor_id));

    let roles: Vec<(Uuid, String, String)> = sqlx::query_as(
        "SELECT cci.instructor_id, cci.role, tei.role
         FROM classroom_course_instructors cci
         JOIN timetable_entry_instructors tei ON tei.instructor_id = cci.instructor_id
         WHERE cci.classroom_course_id = $1 AND tei.entry_id = $2
         ORDER BY cci.instructor_id",
    )
    .bind(fixture.course_id)
    .bind(entry_id)
    .fetch_all(&pool)
    .await
    .expect("course and timetable roles should load");
    assert!(roles.contains(&(
        fixture.primary_instructor_id,
        "secondary".to_string(),
        "secondary".to_string(),
    )));
    assert!(roles.contains(&(
        fixture.second_instructor_id,
        "primary".to_string(),
        "primary".to_string(),
    )));
}

#[tokio::test]
async fn demoting_the_only_instructor_returns_conflict_without_changing_the_team() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;

    assert!(matches!(
        course_planning_service::update_course_instructor_role(
            &pool,
            fixture.course_id,
            fixture.primary_instructor_id,
            "secondary",
        )
        .await,
        Err(AppError::Conflict(_))
    ));

    let role: String = sqlx::query_scalar(
        "SELECT role FROM classroom_course_instructors
         WHERE classroom_course_id = $1 AND instructor_id = $2",
    )
    .bind(fixture.course_id)
    .bind(fixture.primary_instructor_id)
    .fetch_one(&pool)
    .await
    .expect("sole instructor role should load");
    assert_eq!(role, "primary");
}

#[tokio::test]
async fn keeping_a_secondary_role_does_not_restore_an_intentionally_missing_timetable_row() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;

    course_planning_service::add_course_instructor(
        &pool,
        fixture.course_id,
        fixture.second_instructor_id,
        "secondary",
    )
    .await
    .expect("secondary instructor should join the course team");

    let course_entry_id = Uuid::new_v4();
    let conflicting_entry_id = Uuid::new_v4();
    for entry_id in [course_entry_id, conflicting_entry_id] {
        sqlx::query(
            "INSERT INTO academic_timetable_entries
                (id, classroom_course_id, day_of_week, period_id, classroom_id,
                 academic_semester_id)
             VALUES ($1, $2, 'WED', $3, $4, $5)",
        )
        .bind(entry_id)
        .bind(if entry_id == course_entry_id {
            Some(fixture.course_id)
        } else {
            None
        })
        .bind(fixture.period_id)
        .bind(if entry_id == course_entry_id {
            Some(fixture.classroom_id)
        } else {
            None
        })
        .bind(fixture.semester_id)
        .execute(&pool)
        .await
        .expect("timetable entry should insert");
    }
    sqlx::query(
        "INSERT INTO timetable_entry_instructors (entry_id, instructor_id, role)
         VALUES ($1, $2, 'primary'), ($3, $4, 'primary')",
    )
    .bind(course_entry_id)
    .bind(fixture.primary_instructor_id)
    .bind(conflicting_entry_id)
    .bind(fixture.second_instructor_id)
    .execute(&pool)
    .await
    .expect("actual timetable assignments should insert");

    course_planning_service::update_course_instructor_role(
        &pool,
        fixture.course_id,
        fixture.second_instructor_id,
        "secondary",
    )
    .await
    .expect("unchanged secondary role should not touch missing timetable rows");

    let restored_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM timetable_entry_instructors
         WHERE entry_id = $1 AND instructor_id = $2",
    )
    .bind(course_entry_id)
    .bind(fixture.second_instructor_id)
    .fetch_one(&pool)
    .await
    .expect("timetable assignment count should load");
    assert_eq!(restored_count, 0);
}

#[tokio::test]
async fn adding_a_secondary_does_not_restore_a_missing_primary_timetable_row() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let entry_id = Uuid::new_v4();

    sqlx::query(
        "INSERT INTO academic_timetable_entries
            (id, classroom_course_id, day_of_week, period_id, classroom_id,
             academic_semester_id)
         VALUES ($1, $2, 'FRI', $3, $4, $5)",
    )
    .bind(entry_id)
    .bind(fixture.course_id)
    .bind(fixture.period_id)
    .bind(fixture.classroom_id)
    .bind(fixture.semester_id)
    .execute(&pool)
    .await
    .expect("timetable entry should insert without an actual primary assignment");

    course_planning_service::add_course_instructor(
        &pool,
        fixture.course_id,
        fixture.second_instructor_id,
        "secondary",
    )
    .await
    .expect("secondary instructor should be added to current entries");

    let primary_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM timetable_entry_instructors
         WHERE entry_id = $1 AND instructor_id = $2",
    )
    .bind(entry_id)
    .bind(fixture.primary_instructor_id)
    .fetch_one(&pool)
    .await
    .expect("primary assignment count should load");
    assert_eq!(primary_count, 0);
}

#[tokio::test]
async fn removing_a_secondary_does_not_restore_a_missing_primary_timetable_row() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    course_planning_service::add_course_instructor(
        &pool,
        fixture.course_id,
        fixture.second_instructor_id,
        "secondary",
    )
    .await
    .expect("secondary instructor should join the course team");

    let entry_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO academic_timetable_entries
            (id, classroom_course_id, day_of_week, period_id, classroom_id,
             academic_semester_id)
         VALUES ($1, $2, 'SAT', $3, $4, $5)",
    )
    .bind(entry_id)
    .bind(fixture.course_id)
    .bind(fixture.period_id)
    .bind(fixture.classroom_id)
    .bind(fixture.semester_id)
    .execute(&pool)
    .await
    .expect("timetable entry should insert without an actual primary assignment");

    course_planning_service::remove_course_instructor(
        &pool,
        fixture.course_id,
        fixture.second_instructor_id,
    )
    .await
    .expect("secondary instructor should be removed from the course team");

    let primary_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM timetable_entry_instructors
         WHERE entry_id = $1 AND instructor_id = $2",
    )
    .bind(entry_id)
    .bind(fixture.primary_instructor_id)
    .fetch_one(&pool)
    .await
    .expect("primary assignment count should load");
    assert_eq!(primary_count, 0);
}

#[tokio::test]
async fn classroom_activity_endpoints_validate_parents_and_assignment_target() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;

    assert!(matches!(
        course_planning_service::list_classroom_activities(
            &pool,
            Uuid::new_v4(),
            fixture.semester_id,
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        course_planning_service::list_classroom_activities(
            &pool,
            fixture.classroom_id,
            Uuid::new_v4(),
        )
        .await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        course_planning_service::remove_classroom_from_slot(
            &pool,
            fixture.classroom_id,
            Uuid::new_v4(),
        )
        .await,
        Err(AppError::NotFound(_))
    ));

    let rows = course_planning_service::list_classroom_courses(
        &pool,
        &PlanQuery {
            classroom_id: Some(fixture.classroom_id),
            instructor_id: None,
            academic_semester_id: Some(fixture.semester_id),
            subject_id: Some(fixture.subject_id),
        },
    )
    .await
    .expect("fixture course should remain readable");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, fixture.course_id);
}
