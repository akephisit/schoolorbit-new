use std::sync::{
    atomic::{AtomicI32, Ordering},
    Arc,
};

use chrono::{NaiveDate, NaiveTime};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::{
    CreateExamRoundRequest, GenerateSeatsRequest, ImportExamItemsRequest, PlaceExamSessionRequest,
    UpdateExamInvigilatorsRequest, UpdateExamRoundRequest, UpsertDayRoomAssignmentRequest,
    UpsertExamDayRequest,
};
use crate::test_helpers::{create_test_pool, run_test_migrations};

use super::exam_schedule_service;

static NEXT_YEAR: AtomicI32 = AtomicI32::new(30_000);

struct ExamScheduleFixture {
    academic_year_id: Uuid,
    semester_id: Uuid,
    grade_level_id: Uuid,
    classroom_id: Uuid,
    second_classroom_id: Uuid,
    subject_id: Uuid,
    second_subject_id: Uuid,
    course_id: Uuid,
    second_course_id: Uuid,
    third_course_id: Uuid,
    assessment_plan_id: Uuid,
    second_assessment_plan_id: Uuid,
    assessment_category_id: Uuid,
    second_assessment_category_id: Uuid,
    room_id: Uuid,
    second_room_id: Uuid,
    student_user_id: Uuid,
    second_student_user_id: Uuid,
    staff_user_id: Uuid,
    parent_user_id: Uuid,
}

async fn migrated_pool() -> PgPool {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;
    pool
}

async fn insert_active_user(pool: &PgPool, user_type: &str, label: &str) -> Uuid {
    let user_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, password_hash, first_name, last_name, user_type, status)
         VALUES ($1, 'test-only', $2, 'Exam Fixture', $3, 'active')",
    )
    .bind(user_id)
    .bind(label)
    .bind(user_type)
    .execute(pool)
    .await
    .expect("exam-schedule fixture user should insert");
    user_id
}

async fn insert_fixture(pool: &PgPool) -> ExamScheduleFixture {
    let year = NEXT_YEAR.fetch_add(1, Ordering::Relaxed);
    let academic_year_id = Uuid::new_v4();
    let semester_id = Uuid::new_v4();
    let study_plan_id = Uuid::new_v4();
    let study_plan_version_id = Uuid::new_v4();
    let classroom_id = Uuid::new_v4();
    let second_classroom_id = Uuid::new_v4();
    let subject_id = Uuid::new_v4();
    let second_subject_id = Uuid::new_v4();
    let course_id = Uuid::new_v4();
    let second_course_id = Uuid::new_v4();
    let third_course_id = Uuid::new_v4();
    let assessment_plan_id = Uuid::new_v4();
    let second_assessment_plan_id = Uuid::new_v4();
    let assessment_category_id = Uuid::new_v4();
    let second_assessment_category_id = Uuid::new_v4();
    let room_id = Uuid::new_v4();
    let second_room_id = Uuid::new_v4();
    let student_user_id = insert_active_user(pool, "student", "First Student").await;
    let second_student_user_id = insert_active_user(pool, "student", "Second Student").await;
    let staff_user_id = insert_active_user(pool, "staff", "Invigilator").await;
    let parent_user_id = insert_active_user(pool, "parent", "Parent").await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY created_at, id LIMIT 1")
            .fetch_one(pool)
            .await
            .expect("baseline grade level should exist");

    sqlx::query(
        "INSERT INTO academic_years (id, year, name, start_date, end_date)
         VALUES ($1, $2, $3, '9800-01-01', '9800-12-31')",
    )
    .bind(academic_year_id)
    .bind(year)
    .bind(format!("Exam Schedule {year}"))
    .execute(pool)
    .await
    .expect("academic year should insert");

    sqlx::query(
        "INSERT INTO academic_semesters
            (id, academic_year_id, term, name, start_date, end_date)
         VALUES ($1, $2, '1', 'Exam Semester', '9800-01-01', '9800-06-30')",
    )
    .bind(semester_id)
    .bind(academic_year_id)
    .execute(pool)
    .await
    .expect("academic semester should insert");

    sqlx::query(
        "INSERT INTO study_plans (id, code, name_th)
         VALUES ($1, $2, 'Exam Schedule Test Plan')",
    )
    .bind(study_plan_id)
    .bind(format!("EXAM-{study_plan_id}"))
    .execute(pool)
    .await
    .expect("study plan should insert");

    sqlx::query(
        "INSERT INTO study_plan_versions
            (id, study_plan_id, version_name, start_academic_year_id)
         VALUES ($1, $2, 'Exam Version', $3)",
    )
    .bind(study_plan_version_id)
    .bind(study_plan_id)
    .bind(academic_year_id)
    .execute(pool)
    .await
    .expect("study-plan version should insert");

    for (id, suffix) in [(classroom_id, "A"), (second_classroom_id, "B")] {
        sqlx::query(
            "INSERT INTO class_rooms
                (id, code, name, academic_year_id, grade_level_id, study_plan_version_id)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(format!("EXAM-{suffix}-{}", &id.to_string()[..8]))
        .bind(format!("Exam Classroom {suffix}"))
        .bind(academic_year_id)
        .bind(grade_level_id)
        .bind(study_plan_version_id)
        .execute(pool)
        .await
        .expect("classroom should insert");
    }

    for (id, suffix) in [(subject_id, "ONE"), (second_subject_id, "TWO")] {
        sqlx::query(
            "INSERT INTO subjects (id, code, name_th, type, start_academic_year_id)
             VALUES ($1, $2, $3, 'BASIC', $4)",
        )
        .bind(id)
        .bind(format!("EX-{suffix}-{}", &id.to_string()[..8]))
        .bind(format!("Exam Subject {suffix}"))
        .bind(academic_year_id)
        .execute(pool)
        .await
        .expect("subject should insert");
    }

    for (id, classroom, subject) in [
        (course_id, classroom_id, subject_id),
        (second_course_id, classroom_id, second_subject_id),
        (third_course_id, second_classroom_id, subject_id),
    ] {
        sqlx::query(
            "INSERT INTO classroom_courses
                (id, classroom_id, subject_id, academic_semester_id, primary_instructor_id)
             VALUES ($1, $2, $3, $4, $5)",
        )
        .bind(id)
        .bind(classroom)
        .bind(subject)
        .bind(semester_id)
        .bind(staff_user_id)
        .execute(pool)
        .await
        .expect("classroom course should insert");
    }

    for (plan_id, course_id, subject_id) in [
        (assessment_plan_id, course_id, subject_id),
        (
            second_assessment_plan_id,
            second_course_id,
            second_subject_id,
        ),
    ] {
        sqlx::query(
            "INSERT INTO academic_assessment_plans
                (id, classroom_course_id, academic_semester_id, subject_id, status)
             VALUES ($1, $2, $3, $4, 'draft')",
        )
        .bind(plan_id)
        .bind(course_id)
        .bind(semester_id)
        .bind(subject_id)
        .execute(pool)
        .await
        .expect("assessment plan should insert");
    }

    for (category_id, plan_id, suffix) in [
        (assessment_category_id, assessment_plan_id, "One"),
        (
            second_assessment_category_id,
            second_assessment_plan_id,
            "Two",
        ),
    ] {
        sqlx::query(
            "INSERT INTO academic_assessment_categories
                (id, plan_id, code, name, max_score, exam_mode, exam_duration_minutes, created_by)
             VALUES ($1, $2, 'midterm', $3, 50, 'in_timetable', 60, $4)",
        )
        .bind(category_id)
        .bind(plan_id)
        .bind(format!("Midterm {suffix}"))
        .bind(staff_user_id)
        .execute(pool)
        .await
        .expect("assessment category should insert");
    }

    for (student_id, classroom_id, class_number) in [
        (student_user_id, classroom_id, 1),
        (second_student_user_id, second_classroom_id, 1),
    ] {
        sqlx::query(
            "INSERT INTO student_class_enrollments
                (student_id, class_room_id, status, class_number)
             VALUES ($1, $2, 'active', $3)",
        )
        .bind(student_id)
        .bind(classroom_id)
        .bind(class_number)
        .execute(pool)
        .await
        .expect("student enrollment should insert");
    }

    sqlx::query(
        "INSERT INTO student_parents (student_user_id, parent_user_id, relationship)
         VALUES ($1, $2, 'guardian')",
    )
    .bind(student_user_id)
    .bind(parent_user_id)
    .execute(pool)
    .await
    .expect("parent-child link should insert");

    for (id, suffix) in [(room_id, "A"), (second_room_id, "B")] {
        sqlx::query(
            "INSERT INTO rooms (id, name_th, code, room_type, capacity, status)
             VALUES ($1, $2, $3, 'GENERAL', 40, 'ACTIVE')",
        )
        .bind(id)
        .bind(format!("Exam Room {suffix}"))
        .bind(format!("ER-{suffix}-{}", &id.to_string()[..8]))
        .execute(pool)
        .await
        .expect("facility room should insert");
    }

    ExamScheduleFixture {
        academic_year_id,
        semester_id,
        grade_level_id,
        classroom_id,
        second_classroom_id,
        subject_id,
        second_subject_id,
        course_id,
        second_course_id,
        third_course_id,
        assessment_plan_id,
        second_assessment_plan_id,
        assessment_category_id,
        second_assessment_category_id,
        room_id,
        second_room_id,
        student_user_id,
        second_student_user_id,
        staff_user_id,
        parent_user_id,
    }
}

async fn create_round_with_day(pool: &PgPool, fixture: &ExamScheduleFixture) -> (Uuid, Uuid) {
    let round = exam_schedule_service::create_round(
        pool,
        CreateExamRoundRequest {
            academic_semester_id: fixture.semester_id,
            name: format!("Midterm {}", fixture.semester_id),
            description: Some("Database characterization fixture".to_string()),
            exam_kind: Some("midterm".to_string()),
        },
        fixture.staff_user_id,
    )
    .await
    .expect("exam round should be created");

    let day = exam_schedule_service::upsert_exam_day(
        pool,
        round.id,
        UpsertExamDayRequest {
            exam_date: NaiveDate::from_ymd_opt(9800, 3, 1).expect("fixture date should be valid"),
            label: Some("Exam Day 1".to_string()),
            start_time: NaiveTime::from_hms_opt(8, 0, 0).expect("fixture time should be valid"),
            end_time: NaiveTime::from_hms_opt(16, 0, 0).expect("fixture time should be valid"),
            grade_level_ids: vec![fixture.grade_level_id],
            blocked_windows: Vec::new(),
        },
    )
    .await
    .expect("exam day should be created");

    (round.id, day.id)
}

async fn import_items(
    pool: &PgPool,
    round_id: Uuid,
    fixture: &ExamScheduleFixture,
) -> Vec<(Uuid, Uuid, Uuid)> {
    let result = exam_schedule_service::import_exam_items(
        pool,
        round_id,
        ImportExamItemsRequest {
            grade_level_ids: Some(vec![fixture.grade_level_id]),
        },
        fixture.staff_user_id,
    )
    .await
    .expect("exam items should import");
    assert_eq!(result.inserted_count, 3);

    sqlx::query_as(
        "SELECT id, classroom_id, subject_id
         FROM academic_exam_schedule_items
         WHERE exam_round_id = $1
         ORDER BY classroom_id, subject_id, id",
    )
    .bind(round_id)
    .fetch_all(pool)
    .await
    .expect("imported exam items should load")
}

async fn assign_room(pool: &PgPool, exam_day_id: Uuid, fixture: &ExamScheduleFixture) -> Uuid {
    assign_room_for(
        pool,
        exam_day_id,
        fixture.classroom_id,
        fixture.room_id,
        fixture.staff_user_id,
    )
    .await
}

async fn assign_room_for(
    pool: &PgPool,
    exam_day_id: Uuid,
    classroom_id: Uuid,
    room_id: Uuid,
    actor_user_id: Uuid,
) -> Uuid {
    exam_schedule_service::upsert_day_room_assignment(
        pool,
        exam_day_id,
        UpsertDayRoomAssignmentRequest {
            classroom_id,
            room_id,
            capacity_override: None,
            invigilator_staff_ids: None,
        },
        actor_user_id,
    )
    .await
    .expect("day room assignment should be created")
    .id
}

fn imported_item_id(items: &[(Uuid, Uuid, Uuid)], classroom_id: Uuid, subject_id: Uuid) -> Uuid {
    items
        .iter()
        .find_map(|(item_id, item_classroom_id, item_subject_id)| {
            (*item_classroom_id == classroom_id && *item_subject_id == subject_id)
                .then_some(*item_id)
        })
        .expect("expected imported exam item should exist")
}

async fn place_session(
    pool: &PgPool,
    item_id: Uuid,
    exam_day_id: Uuid,
    starts_at: NaiveTime,
    actor_user_id: Uuid,
) -> Uuid {
    exam_schedule_service::place_exam_session(
        pool,
        PlaceExamSessionRequest {
            exam_schedule_item_id: item_id,
            exam_day_id,
            starts_at,
        },
        actor_user_id,
    )
    .await
    .expect("exam session should be placed")
    .id
}

#[tokio::test]
async fn fixture_builds_all_prerequisites() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;

    let course_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT
         FROM classroom_courses
         WHERE id = ANY($1::uuid[])
           AND academic_semester_id = $2",
    )
    .bind(vec![
        fixture.course_id,
        fixture.second_course_id,
        fixture.third_course_id,
    ])
    .bind(fixture.semester_id)
    .fetch_one(&pool)
    .await
    .expect("fixture courses should be queryable");
    assert_eq!(course_count, 3);

    let enrollment_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT
         FROM student_class_enrollments
         WHERE student_id = ANY($1::uuid[])
           AND class_room_id = ANY($2::uuid[])
           AND status = 'active'",
    )
    .bind(vec![
        fixture.student_user_id,
        fixture.second_student_user_id,
    ])
    .bind(vec![fixture.classroom_id, fixture.second_classroom_id])
    .fetch_one(&pool)
    .await
    .expect("fixture enrollments should be queryable");
    assert_eq!(enrollment_count, 2);

    let category_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT
         FROM academic_assessment_categories category
         JOIN academic_assessment_plans plan ON plan.id = category.plan_id
         WHERE category.id = ANY($1::uuid[])
           AND plan.id = ANY($2::uuid[])
           AND category.code = 'midterm'
           AND category.exam_mode = 'in_timetable'
           AND category.exam_duration_minutes = 60",
    )
    .bind(vec![
        fixture.assessment_category_id,
        fixture.second_assessment_category_id,
    ])
    .bind(vec![
        fixture.assessment_plan_id,
        fixture.second_assessment_plan_id,
    ])
    .fetch_one(&pool)
    .await
    .expect("fixture assessment categories should be queryable");
    assert_eq!(category_count, 2);

    let parent_link_exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(
            SELECT 1 FROM student_parents
            WHERE student_user_id = $1 AND parent_user_id = $2
         )",
    )
    .bind(fixture.student_user_id)
    .bind(fixture.parent_user_id)
    .fetch_one(&pool)
    .await
    .expect("fixture parent link should be queryable");
    assert!(parent_link_exists);

    let room_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM rooms
         WHERE id = ANY($1::uuid[]) AND status = 'ACTIVE'",
    )
    .bind(vec![fixture.room_id, fixture.second_room_id])
    .fetch_one(&pool)
    .await
    .expect("fixture rooms should be queryable");
    assert_eq!(room_count, 2);

    let academic_year_exists: bool =
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_years WHERE id = $1)")
            .bind(fixture.academic_year_id)
            .fetch_one(&pool)
            .await
            .expect("fixture academic year should be queryable");
    assert!(academic_year_exists);

    let subject_count: i64 =
        sqlx::query_scalar("SELECT COUNT(*)::BIGINT FROM subjects WHERE id = ANY($1::uuid[])")
            .bind(vec![fixture.subject_id, fixture.second_subject_id])
            .fetch_one(&pool)
            .await
            .expect("fixture subjects should be queryable");
    assert_eq!(subject_count, 2);

    let (round_id, day_id) = create_round_with_day(&pool, &fixture).await;
    assert_eq!(import_items(&pool, round_id, &fixture).await.len(), 3);
    assert_ne!(assign_room(&pool, day_id, &fixture).await, Uuid::nil());
}

#[tokio::test]
async fn round_and_day_lifecycle_preserves_identity_and_draft_rules() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let (round_id, day_id) = create_round_with_day(&pool, &fixture).await;

    let updated_day = exam_schedule_service::update_exam_day(
        &pool,
        day_id,
        UpsertExamDayRequest {
            exam_date: NaiveDate::from_ymd_opt(9800, 3, 2).expect("fixture date should be valid"),
            label: Some("Updated Exam Day".to_string()),
            start_time: NaiveTime::from_hms_opt(9, 0, 0).expect("fixture time should be valid"),
            end_time: NaiveTime::from_hms_opt(15, 0, 0).expect("fixture time should be valid"),
            grade_level_ids: vec![fixture.grade_level_id],
            blocked_windows: Vec::new(),
        },
    )
    .await
    .expect("exam day should update");
    assert_eq!(updated_day.id, day_id);
    assert_eq!(updated_day.label.as_deref(), Some("Updated Exam Day"));

    let updated_round = exam_schedule_service::update_round(
        &pool,
        round_id,
        UpdateExamRoundRequest {
            name: Some("Final Characterization".to_string()),
            description: Some("Updated description".to_string()),
            exam_kind: Some("final".to_string()),
        },
        fixture.staff_user_id,
    )
    .await
    .expect("exam round should update");
    assert_eq!(updated_round.id, round_id);
    assert_eq!(updated_round.status, "draft");
    assert_eq!(updated_round.exam_kind, "final");

    let listed_round = exam_schedule_service::list_rounds(&pool, Some(fixture.semester_id))
        .await
        .expect("exam rounds should list")
        .into_iter()
        .find(|round| round.id == round_id)
        .expect("updated round should be listed");
    assert_eq!(listed_round.name, "Final Characterization");

    let workspace = exam_schedule_service::get_workspace(&pool, round_id)
        .await
        .expect("workspace should load");
    assert_eq!(workspace.days.len(), 1);
    assert_eq!(workspace.days[0].id, day_id);
    assert_eq!(workspace.days[0].exam_date, updated_day.exam_date);

    exam_schedule_service::delete_exam_day(&pool, day_id)
        .await
        .expect("exam day should delete");
    let workspace = exam_schedule_service::get_workspace(&pool, round_id)
        .await
        .expect("workspace should reload");
    assert!(workspace.days.is_empty());
}

#[tokio::test]
async fn room_assignment_and_seat_generation_preserve_capacity_rules() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let extra_student_id = insert_active_user(&pool, "student", "Capacity Student").await;
    sqlx::query(
        "INSERT INTO student_class_enrollments
            (student_id, class_room_id, status, class_number)
         VALUES ($1, $2, 'active', 2)",
    )
    .bind(extra_student_id)
    .bind(fixture.classroom_id)
    .execute(&pool)
    .await
    .expect("capacity-test enrollment should insert");

    let (round_id, day_id) = create_round_with_day(&pool, &fixture).await;
    assert_eq!(import_items(&pool, round_id, &fixture).await.len(), 3);
    let assignment_id = assign_room(&pool, day_id, &fixture).await;

    let seats = exam_schedule_service::generate_seats_for_assignment(
        &pool,
        assignment_id,
        GenerateSeatsRequest { regenerate: false },
        fixture.staff_user_id,
    )
    .await
    .expect("seats should generate");
    assert_eq!(seats.len(), 2);
    assert_eq!(seats[0].student_id, fixture.student_user_id);
    assert_eq!(seats[0].seat_number, "01");
    assert_eq!(seats[1].student_id, extra_student_id);
    assert_eq!(seats[1].seat_number, "02");

    let capacity_error = exam_schedule_service::upsert_day_room_assignment(
        &pool,
        day_id,
        UpsertDayRoomAssignmentRequest {
            classroom_id: fixture.classroom_id,
            room_id: fixture.room_id,
            capacity_override: Some(1),
            invigilator_staff_ids: None,
        },
        fixture.staff_user_id,
    )
    .await
    .expect_err("capacity below active enrollment should be rejected");
    assert!(matches!(
        capacity_error,
        AppError::BadRequest(message)
            if message == "Classroom has 2 active student(s), which exceeds the room capacity of 1"
    ));

    let assignments = exam_schedule_service::list_day_room_assignments(&pool, day_id)
        .await
        .expect("room assignments should list");
    assert_eq!(assignments.len(), 1);
    assert_eq!(assignments[0].id, assignment_id);
    assert_eq!(assignments[0].capacity_override, None);

    let persisted_seats = exam_schedule_service::generate_seats_for_assignment(
        &pool,
        assignment_id,
        GenerateSeatsRequest { regenerate: false },
        fixture.staff_user_id,
    )
    .await
    .expect("existing seats should load");
    assert_eq!(
        persisted_seats
            .iter()
            .map(|seat| (seat.id, seat.student_id, seat.seat_number.as_str()))
            .collect::<Vec<_>>(),
        seats
            .iter()
            .map(|seat| (seat.id, seat.student_id, seat.seat_number.as_str()))
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn invigilator_assignment_rejects_overlapping_live_sessions() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let (round_id, day_id) = create_round_with_day(&pool, &fixture).await;
    let items = import_items(&pool, round_id, &fixture).await;
    let first_assignment_id = assign_room(&pool, day_id, &fixture).await;
    let second_assignment_id = assign_room_for(
        &pool,
        day_id,
        fixture.second_classroom_id,
        fixture.second_room_id,
        fixture.staff_user_id,
    )
    .await;

    exam_schedule_service::assign_invigilator_to_assignment(
        &pool,
        first_assignment_id,
        fixture.staff_user_id,
        fixture.staff_user_id,
    )
    .await
    .expect("first invigilator assignment should succeed");

    place_session(
        &pool,
        imported_item_id(&items, fixture.classroom_id, fixture.subject_id),
        day_id,
        NaiveTime::from_hms_opt(8, 0, 0).expect("fixture time should be valid"),
        fixture.staff_user_id,
    )
    .await;
    place_session(
        &pool,
        imported_item_id(&items, fixture.second_classroom_id, fixture.subject_id),
        day_id,
        NaiveTime::from_hms_opt(8, 30, 0).expect("fixture time should be valid"),
        fixture.staff_user_id,
    )
    .await;

    let conflict = exam_schedule_service::update_assignment_invigilators(
        &pool,
        second_assignment_id,
        UpdateExamInvigilatorsRequest {
            invigilator_staff_ids: vec![fixture.staff_user_id],
        },
        fixture.staff_user_id,
    )
    .await
    .expect_err("overlapping invigilation should be rejected");
    assert!(matches!(
        conflict,
        AppError::BadRequest(message)
            if message == "Invigilator has an overlapping exam supervision assignment"
    ));

    let workspace = exam_schedule_service::get_invigilator_workspace(&pool, round_id)
        .await
        .expect("invigilator workspace should load");
    let first_assignment = workspace
        .assignments
        .iter()
        .find(|assignment| assignment.assignment_id == first_assignment_id)
        .expect("first assignment should remain");
    let second_assignment = workspace
        .assignments
        .iter()
        .find(|assignment| assignment.assignment_id == second_assignment_id)
        .expect("second assignment should remain");
    assert_eq!(first_assignment.invigilators.len(), 1);
    assert_eq!(
        first_assignment.invigilators[0].staff_id,
        fixture.staff_user_id
    );
    assert!(second_assignment.invigilators.is_empty());
}

#[tokio::test]
async fn concurrent_overlapping_placements_allow_only_one_commit() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let (round_id, day_id) = create_round_with_day(&pool, &fixture).await;
    let items = import_items(&pool, round_id, &fixture).await;
    assign_room(&pool, day_id, &fixture).await;

    let first_item_id = imported_item_id(&items, fixture.classroom_id, fixture.subject_id);
    let second_item_id = imported_item_id(&items, fixture.classroom_id, fixture.second_subject_id);
    let actor_user_id = fixture.staff_user_id;
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let first_pool = pool.clone();
    let second_pool = pool.clone();
    let first_barrier = Arc::clone(&barrier);
    let second_barrier = Arc::clone(&barrier);
    let starts_at = NaiveTime::from_hms_opt(8, 0, 0).expect("fixture time should be valid");

    let (first_result, second_result) = tokio::join!(
        async move {
            first_barrier.wait().await;
            exam_schedule_service::place_exam_session(
                &first_pool,
                PlaceExamSessionRequest {
                    exam_schedule_item_id: first_item_id,
                    exam_day_id: day_id,
                    starts_at,
                },
                actor_user_id,
            )
            .await
        },
        async move {
            second_barrier.wait().await;
            exam_schedule_service::place_exam_session(
                &second_pool,
                PlaceExamSessionRequest {
                    exam_schedule_item_id: second_item_id,
                    exam_day_id: day_id,
                    starts_at,
                },
                actor_user_id,
            )
            .await
        }
    );

    assert_eq!(
        usize::from(first_result.is_ok()) + usize::from(second_result.is_ok()),
        1
    );
    let errors = [first_result.err(), second_result.err()]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(errors.len(), 1);
    assert!(matches!(
        &errors[0],
        AppError::BadRequest(message)
            if message == "Classroom already has an exam session during this time"
    ));

    let persisted_count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*)::BIGINT FROM academic_exam_sessions
         WHERE exam_schedule_item_id = ANY($1::uuid[])",
    )
    .bind(vec![first_item_id, second_item_id])
    .fetch_one(&pool)
    .await
    .expect("concurrent session rows should be countable");
    assert_eq!(persisted_count, 1);
}

#[tokio::test]
async fn publish_exposes_the_same_session_to_student_staff_and_linked_parent() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let (round_id, day_id) = create_round_with_day(&pool, &fixture).await;
    let items = import_items(&pool, round_id, &fixture).await;
    let first_assignment_id = assign_room(&pool, day_id, &fixture).await;
    let second_assignment_id = assign_room_for(
        &pool,
        day_id,
        fixture.second_classroom_id,
        fixture.second_room_id,
        fixture.staff_user_id,
    )
    .await;

    for assignment_id in [first_assignment_id, second_assignment_id] {
        exam_schedule_service::generate_seats_for_assignment(
            &pool,
            assignment_id,
            GenerateSeatsRequest { regenerate: false },
            fixture.staff_user_id,
        )
        .await
        .expect("publish fixture seats should generate");
    }
    exam_schedule_service::assign_invigilator_to_assignment(
        &pool,
        first_assignment_id,
        fixture.staff_user_id,
        fixture.staff_user_id,
    )
    .await
    .expect("publish fixture invigilator should assign");

    place_session(
        &pool,
        imported_item_id(&items, fixture.classroom_id, fixture.subject_id),
        day_id,
        NaiveTime::from_hms_opt(8, 0, 0).expect("fixture time should be valid"),
        fixture.staff_user_id,
    )
    .await;
    place_session(
        &pool,
        imported_item_id(&items, fixture.classroom_id, fixture.second_subject_id),
        day_id,
        NaiveTime::from_hms_opt(9, 0, 0).expect("fixture time should be valid"),
        fixture.staff_user_id,
    )
    .await;
    place_session(
        &pool,
        imported_item_id(&items, fixture.second_classroom_id, fixture.subject_id),
        day_id,
        NaiveTime::from_hms_opt(8, 0, 0).expect("fixture time should be valid"),
        fixture.staff_user_id,
    )
    .await;

    let published = exam_schedule_service::publish_round(&pool, round_id, fixture.staff_user_id)
        .await
        .expect("ready exam round should publish");
    assert_eq!(published.status, "published");

    let student_rounds = exam_schedule_service::list_my_published_exam_schedule(
        &pool,
        fixture.student_user_id,
        Some(fixture.semester_id),
    )
    .await
    .expect("student published schedule should load");
    let staff_rounds = exam_schedule_service::list_staff_published_exam_schedule(
        &pool,
        fixture.staff_user_id,
        Some(fixture.semester_id),
    )
    .await
    .expect("staff published schedule should load");
    let parent_rounds = exam_schedule_service::list_child_published_exam_schedule(
        &pool,
        fixture.parent_user_id,
        fixture.student_user_id,
        Some(fixture.semester_id),
    )
    .await
    .expect("parent published schedule should load");

    assert_eq!(student_rounds.len(), 1);
    assert_eq!(staff_rounds.len(), 1);
    assert_eq!(parent_rounds.len(), 1);
    assert_eq!(student_rounds[0].round_id, round_id);
    assert_eq!(staff_rounds[0].round_id, round_id);
    assert_eq!(parent_rounds[0].round_id, round_id);
    assert_eq!(student_rounds[0].sessions.len(), 2);
    assert_eq!(staff_rounds[0].sessions.len(), 3);
    assert_eq!(parent_rounds[0].sessions.len(), 2);

    let student_sessions = student_rounds[0]
        .sessions
        .iter()
        .map(|session| {
            (
                session.exam_date,
                session.starts_at,
                session.ends_at,
                session.subject_name.as_str(),
                session.room_name.as_str(),
                session.seat_number.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    let parent_sessions = parent_rounds[0]
        .sessions
        .iter()
        .map(|session| {
            (
                session.exam_date,
                session.starts_at,
                session.ends_at,
                session.subject_name.as_str(),
                session.room_name.as_str(),
                session.seat_number.as_deref(),
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(student_sessions, parent_sessions);
    assert!(student_sessions
        .iter()
        .all(|session| session.5 == Some("01")));

    let unrelated_parent_id = insert_active_user(&pool, "parent", "Unrelated Parent").await;
    let unrelated_error = exam_schedule_service::list_child_published_exam_schedule(
        &pool,
        unrelated_parent_id,
        fixture.student_user_id,
        Some(fixture.semester_id),
    )
    .await
    .expect_err("unlinked parent should not read the student schedule");
    assert!(matches!(
        unrelated_error,
        AppError::Forbidden(message) if message == "คุณไม่มีสิทธิ์เข้าถึงข้อมูลนักเรียนคนนี้"
    ));
}
