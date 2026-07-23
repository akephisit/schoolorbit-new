use std::sync::atomic::{AtomicI32, Ordering};

use sqlx::PgPool;
use uuid::Uuid;

use crate::modules::academic::models::timetable::{
    CreateBatchTimetableEntriesRequest, CreateTimetableEntryRequest, SwapTimetableEntriesRequest,
    UpdateTimetableEntryRequest, ValidateMovesRequest,
};
use crate::test_helpers::{create_test_pool, run_test_migrations};

use super::timetable_service::{
    self, CreateEntryOutcome, SwapOutcome, TimetableFilter, UpdateEntryOutcome,
};

static NEXT_YEAR: AtomicI32 = AtomicI32::new(40_000);

struct TimetableFixture {
    semester_id: Uuid,
    classroom_id: Uuid,
    course_id: Uuid,
    second_course_id: Uuid,
    period_id: Uuid,
    second_period_id: Uuid,
    room_id: Uuid,
    second_room_id: Uuid,
    student_id: Uuid,
    parent_id: Uuid,
    instructor_id: Uuid,
    second_instructor_id: Uuid,
}

async fn migrated_pool() -> PgPool {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await;
    pool
}

async fn insert_user(pool: &PgPool, user_type: &str, first_name: &str) -> Uuid {
    let id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO users (id, password_hash, first_name, last_name, user_type, status)
         VALUES ($1, 'test-only', $2, 'Timetable Fixture', $3, 'active')",
    )
    .bind(id)
    .bind(first_name)
    .bind(user_type)
    .execute(pool)
    .await
    .expect("timetable fixture user should insert");
    id
}

async fn insert_fixture(pool: &PgPool) -> TimetableFixture {
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
    let period_id = Uuid::new_v4();
    let second_period_id = Uuid::new_v4();
    let room_id = Uuid::new_v4();
    let second_room_id = Uuid::new_v4();
    let student_id = insert_user(pool, "student", "Timetable Student").await;
    let parent_id = insert_user(pool, "parent", "Timetable Parent").await;
    let instructor_id = insert_user(pool, "staff", "Primary Teacher").await;
    let second_instructor_id = insert_user(pool, "staff", "Secondary Teacher").await;
    let grade_level_id: Uuid =
        sqlx::query_scalar("SELECT id FROM grade_levels ORDER BY created_at, id LIMIT 1")
            .fetch_one(pool)
            .await
            .expect("baseline grade level should exist");

    sqlx::query(
        "INSERT INTO academic_years (id, year, name, start_date, end_date)
         VALUES ($1, $2, $3, '9700-01-01', '9700-12-31')",
    )
    .bind(academic_year_id)
    .bind(year)
    .bind(format!("Timetable {year}"))
    .execute(pool)
    .await
    .expect("academic year should insert");

    sqlx::query(
        "INSERT INTO academic_semesters
            (id, academic_year_id, term, name, start_date, end_date)
         VALUES ($1, $2, '1', 'Timetable Semester', '9700-01-01', '9700-06-30')",
    )
    .bind(semester_id)
    .bind(academic_year_id)
    .execute(pool)
    .await
    .expect("semester should insert");

    sqlx::query(
        "INSERT INTO study_plans (id, code, name_th)
         VALUES ($1, $2, 'Timetable Characterization Plan')",
    )
    .bind(study_plan_id)
    .bind(format!("TT-{study_plan_id}"))
    .execute(pool)
    .await
    .expect("study plan should insert");

    sqlx::query(
        "INSERT INTO study_plan_versions
            (id, study_plan_id, version_name, start_academic_year_id)
         VALUES ($1, $2, 'Timetable Version', $3)",
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
        .bind(format!("TT-{suffix}-{}", &id.to_string()[..8]))
        .bind(format!("Timetable Classroom {suffix}"))
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
        .bind(format!("TT-{suffix}-{}", &id.to_string()[..8]))
        .bind(format!("Timetable Subject {suffix}"))
        .bind(academic_year_id)
        .execute(pool)
        .await
        .expect("subject should insert");
    }

    for (id, classroom, subject, instructor) in [
        (course_id, classroom_id, subject_id, instructor_id),
        (
            second_course_id,
            second_classroom_id,
            second_subject_id,
            second_instructor_id,
        ),
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
        .bind(instructor)
        .execute(pool)
        .await
        .expect("classroom course should insert");
    }

    for (id, name, start, end, order_index) in [
        (period_id, "Period 1", "08:00", "08:50", 1),
        (second_period_id, "Period 2", "09:00", "09:50", 2),
    ] {
        sqlx::query(
            "INSERT INTO academic_periods
                (id, academic_year_id, name, start_time, end_time, order_index)
             VALUES ($1, $2, $3, $4::time, $5::time, $6)",
        )
        .bind(id)
        .bind(academic_year_id)
        .bind(name)
        .bind(start)
        .bind(end)
        .bind(order_index)
        .execute(pool)
        .await
        .expect("period should insert");
    }

    for (id, suffix) in [(room_id, "A"), (second_room_id, "B")] {
        sqlx::query(
            "INSERT INTO rooms (id, name_th, code, room_type, capacity, status)
             VALUES ($1, $2, $3, 'GENERAL', 40, 'ACTIVE')",
        )
        .bind(id)
        .bind(format!("Timetable Room {suffix}"))
        .bind(format!("TR-{suffix}-{}", &id.to_string()[..8]))
        .execute(pool)
        .await
        .expect("room should insert");
    }

    sqlx::query(
        "INSERT INTO student_class_enrollments
            (student_id, class_room_id, status, class_number)
         VALUES ($1, $2, 'active', 1)",
    )
    .bind(student_id)
    .bind(classroom_id)
    .execute(pool)
    .await
    .expect("student enrollment should insert");

    sqlx::query(
        "INSERT INTO student_parents (student_user_id, parent_user_id, relationship)
         VALUES ($1, $2, 'guardian')",
    )
    .bind(student_id)
    .bind(parent_id)
    .execute(pool)
    .await
    .expect("parent-child link should insert");

    TimetableFixture {
        semester_id,
        classroom_id,
        course_id,
        second_course_id,
        period_id,
        second_period_id,
        room_id,
        second_room_id,
        student_id,
        parent_id,
        instructor_id,
        second_instructor_id,
    }
}

fn course_request(
    course_id: Uuid,
    day: &str,
    period_id: Uuid,
    room_id: Uuid,
) -> CreateTimetableEntryRequest {
    CreateTimetableEntryRequest {
        classroom_course_id: Some(course_id),
        day_of_week: day.to_string(),
        period_id,
        room_id: Some(room_id),
        note: Some("characterization".to_string()),
        activity_slot_id: None,
        entry_type: None,
        title: None,
        classroom_id: None,
        academic_semester_id: None,
        client_temp_id: None,
    }
}

async fn create_course_entry(
    pool: &PgPool,
    actor_id: Uuid,
    course_id: Uuid,
    day: &str,
    period_id: Uuid,
    room_id: Uuid,
) -> crate::modules::academic::models::timetable::TimetableEntry {
    match timetable_service::create_entry(
        pool,
        Some(actor_id),
        course_request(course_id, day, period_id, room_id),
    )
    .await
    .expect("course entry creation should complete")
    {
        CreateEntryOutcome::Created(entry) => *entry,
        CreateEntryOutcome::Conflict(_) => panic!("fixture entry should not conflict"),
    }
}

fn semester_filter(semester_id: Uuid) -> TimetableFilter {
    TimetableFilter {
        academic_semester_id: Some(semester_id),
        ..TimetableFilter::default()
    }
}

#[tokio::test]
async fn create_update_delete_and_filtered_lists_preserve_joined_entry_shape() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let created = create_course_entry(
        &pool,
        fixture.instructor_id,
        fixture.course_id,
        "MON",
        fixture.period_id,
        fixture.room_id,
    )
    .await;

    let joined = timetable_service::fetch_entry_by_id(&pool, created.id)
        .await
        .expect("created entry should be fetchable");
    assert_eq!(joined.classroom_id, Some(fixture.classroom_id));
    assert_eq!(
        joined.subject_name_th.as_deref(),
        Some("Timetable Subject ONE")
    );
    assert_eq!(
        joined.classroom_name.as_deref(),
        Some("Timetable Classroom A")
    );
    assert_eq!(joined.period_name.as_deref(), Some("Period 1"));
    assert!(joined
        .instructor_ids
        .as_ref()
        .is_some_and(|ids| ids == &[fixture.instructor_id]));

    let updated = timetable_service::update_entry(
        &pool,
        Some(fixture.instructor_id),
        created.id,
        UpdateTimetableEntryRequest {
            day_of_week: Some("TUE".to_string()),
            period_id: Some(fixture.second_period_id),
            room_id: Some(fixture.second_room_id),
            note: Some("updated characterization".to_string()),
            classroom_course_id: None,
            activity_slot_id: None,
            classroom_id: None,
        },
    )
    .await
    .expect("entry update should complete");
    match updated {
        UpdateEntryOutcome::Updated { updated, existing } => {
            assert_eq!(existing.day_of_week, "MON");
            assert_eq!(updated.day_of_week, "TUE");
            assert_eq!(updated.period_id, fixture.second_period_id);
            assert_eq!(updated.room_id, Some(fixture.second_room_id));
        }
        UpdateEntryOutcome::Conflict { .. } => panic!("fixture update should not conflict"),
    }

    let listed = timetable_service::list_entries(
        &pool,
        TimetableFilter {
            classroom_id: Some(fixture.classroom_id),
            academic_semester_id: Some(fixture.semester_id),
            day_of_week: Some("TUE".to_string()),
            ..TimetableFilter::default()
        },
    )
    .await
    .expect("filtered timetable should list");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].id, created.id);
    assert!(listed[0].room_code.is_some());

    assert_eq!(
        timetable_service::delete_entry(&pool, created.id)
            .await
            .expect("entry deletion should complete"),
        Some(fixture.semester_id)
    );
    assert!(timetable_service::fetch_entry_by_id(&pool, created.id)
        .await
        .is_none());
}

#[tokio::test]
async fn admin_self_and_parent_filters_resolve_the_same_persisted_entry() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let created = create_course_entry(
        &pool,
        fixture.instructor_id,
        fixture.course_id,
        "MON",
        fixture.period_id,
        fixture.room_id,
    )
    .await;

    let admin = timetable_service::list_entries(&pool, semester_filter(fixture.semester_id))
        .await
        .expect("admin timetable should list");
    let student = timetable_service::list_entries(
        &pool,
        TimetableFilter {
            student_id: Some(fixture.student_id),
            academic_semester_id: Some(fixture.semester_id),
            ..TimetableFilter::default()
        },
    )
    .await
    .expect("student timetable should list");
    let teacher = timetable_service::list_entries(
        &pool,
        TimetableFilter {
            instructor_id: Some(fixture.instructor_id),
            academic_semester_id: Some(fixture.semester_id),
            ..TimetableFilter::default()
        },
    )
    .await
    .expect("teacher timetable should list");

    let linked_student: Uuid =
        sqlx::query_scalar("SELECT student_user_id FROM student_parents WHERE parent_user_id = $1")
            .bind(fixture.parent_id)
            .fetch_one(&pool)
            .await
            .expect("fixture parent should resolve child");
    let parent = timetable_service::list_entries(
        &pool,
        TimetableFilter {
            student_id: Some(linked_student),
            academic_semester_id: Some(fixture.semester_id),
            ..TimetableFilter::default()
        },
    )
    .await
    .expect("parent timetable should list");

    for view in [&admin, &student, &teacher, &parent] {
        assert_eq!(view.len(), 1);
        assert_eq!(view[0].id, created.id);
    }
}

#[tokio::test]
async fn room_classroom_and_instructor_conflicts_preserve_outcomes_and_rollback() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    create_course_entry(
        &pool,
        fixture.instructor_id,
        fixture.course_id,
        "MON",
        fixture.period_id,
        fixture.room_id,
    )
    .await;

    let classroom_conflict = timetable_service::create_entry(
        &pool,
        Some(fixture.instructor_id),
        course_request(
            fixture.course_id,
            "MON",
            fixture.period_id,
            fixture.second_room_id,
        ),
    )
    .await
    .expect("classroom conflict should be reported");
    assert!(matches!(
        classroom_conflict,
        CreateEntryOutcome::Conflict(ref conflicts)
            if conflicts.iter().any(|c| c.conflict_type == "CLASSROOM_CONFLICT")
    ));

    let room_conflict = timetable_service::create_entry(
        &pool,
        Some(fixture.second_instructor_id),
        course_request(
            fixture.second_course_id,
            "MON",
            fixture.period_id,
            fixture.room_id,
        ),
    )
    .await
    .expect("room conflict should be reported");
    assert!(matches!(
        room_conflict,
        CreateEntryOutcome::Conflict(ref conflicts)
            if conflicts.iter().any(|c| c.conflict_type == "ROOM_CONFLICT")
    ));

    sqlx::query(
        "INSERT INTO classroom_course_instructors
            (classroom_course_id, instructor_id, role)
         VALUES ($1, $2, 'secondary')",
    )
    .bind(fixture.second_course_id)
    .bind(fixture.instructor_id)
    .execute(&pool)
    .await
    .expect("shared instructor should insert");
    let instructor_conflict = timetable_service::create_entry(
        &pool,
        Some(fixture.second_instructor_id),
        course_request(
            fixture.second_course_id,
            "MON",
            fixture.period_id,
            fixture.second_room_id,
        ),
    )
    .await
    .expect("instructor conflict should be reported");
    assert!(matches!(
        instructor_conflict,
        CreateEntryOutcome::Conflict(ref conflicts)
            if conflicts.iter().any(|c| c.conflict_type == "INSTRUCTOR_CONFLICT")
    ));

    let persisted: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM academic_timetable_entries
         WHERE academic_semester_id = $1",
    )
    .bind(fixture.semester_id)
    .fetch_one(&pool)
    .await
    .expect("persisted entry count should query");
    assert_eq!(persisted, 1);
}

#[tokio::test]
async fn add_remove_instructors_preserve_team_and_entry_state() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let created = create_course_entry(
        &pool,
        fixture.instructor_id,
        fixture.course_id,
        "MON",
        fixture.period_id,
        fixture.room_id,
    )
    .await;

    let added = timetable_service::add_entry_instructor(
        &pool,
        created.id,
        fixture.second_instructor_id,
        "secondary",
    )
    .await
    .expect("secondary instructor should add");
    assert_eq!(added.semester_id, Some(fixture.semester_id));
    assert!(added.instructor_name.contains("Secondary Teacher"));

    let occupancy = timetable_service::get_occupancy(&pool, fixture.semester_id)
        .await
        .expect("occupancy should list");
    let row = occupancy
        .iter()
        .find(|row| row.id == created.id)
        .expect("created entry should be occupied");
    assert!(row.instructor_ids.contains(&fixture.instructor_id));
    assert!(row.instructor_ids.contains(&fixture.second_instructor_id));

    let removed =
        timetable_service::remove_entry_instructor(&pool, created.id, fixture.second_instructor_id)
            .await
            .expect("secondary instructor should remove");
    assert_eq!(removed.semester_id, Some(fixture.semester_id));
    assert!(!removed.entry_deleted);
    assert!(timetable_service::fetch_entry_by_id(&pool, created.id)
        .await
        .is_some());

    let removed_last =
        timetable_service::remove_entry_instructor(&pool, created.id, fixture.instructor_id)
            .await
            .expect("last course instructor should remove");
    assert!(removed_last.entry_deleted);
    assert!(timetable_service::fetch_entry_by_id(&pool, created.id)
        .await
        .is_none());
}

#[tokio::test]
async fn swap_validate_occupancy_and_batch_mutations_preserve_persisted_rows() {
    let pool = migrated_pool().await;
    let fixture = insert_fixture(&pool).await;
    let first = create_course_entry(
        &pool,
        fixture.instructor_id,
        fixture.course_id,
        "MON",
        fixture.period_id,
        fixture.room_id,
    )
    .await;
    let second = create_course_entry(
        &pool,
        fixture.second_instructor_id,
        fixture.second_course_id,
        "TUE",
        fixture.second_period_id,
        fixture.second_room_id,
    )
    .await;

    let cells =
        timetable_service::validate_moves(&pool, ValidateMovesRequest { entry_id: first.id })
            .await
            .expect("move validation should complete");
    assert!(cells.iter().any(|cell| {
        cell.day_of_week == "MON" && cell.period_id == fixture.period_id && cell.state == "source"
    }));
    assert!(cells.iter().any(|cell| {
        cell.day_of_week == "TUE"
            && cell.period_id == fixture.second_period_id
            && cell.state == "occupied"
            && cell.target_entry_id == Some(second.id)
    }));

    match timetable_service::swap_entries(
        &pool,
        SwapTimetableEntriesRequest {
            entry_a_id: first.id,
            entry_b_id: second.id,
        },
    )
    .await
    .expect("entry swap should complete")
    {
        SwapOutcome::Swapped { semester_id } => assert_eq!(semester_id, fixture.semester_id),
        SwapOutcome::Conflict(_) => panic!("fixture swap should not conflict"),
    }
    let swapped_first = timetable_service::fetch_entry_by_id(&pool, first.id)
        .await
        .expect("swapped first entry should remain");
    assert_eq!(swapped_first.day_of_week, "TUE");
    assert_eq!(swapped_first.period_id, fixture.second_period_id);

    let batch = timetable_service::create_batch_entries(
        &pool,
        Some(fixture.instructor_id),
        CreateBatchTimetableEntriesRequest {
            classroom_ids: vec![fixture.classroom_id],
            days_of_week: vec!["WED".to_string()],
            period_ids: vec![fixture.period_id, fixture.second_period_id],
            academic_semester_id: fixture.semester_id,
            entry_type: "BREAK".to_string(),
            title: "Characterization Break".to_string(),
            room_id: None,
            note: Some("batch characterization".to_string()),
            subject_id: None,
            force: Some(false),
            activity_slot_id: None,
            instructor_ids: Vec::new(),
        },
    )
    .await
    .expect("batch creation should complete");
    assert_eq!(batch.inserted_count, 2);
    assert!(batch.skipped.is_empty());
    assert!(batch.blocked.is_empty());
    assert_eq!(batch.semester_id, fixture.semester_id);

    let batch_rows: Vec<(Uuid, Uuid)> = sqlx::query_as(
        "SELECT id, batch_id FROM academic_timetable_entries
         WHERE academic_semester_id = $1 AND title = 'Characterization Break'
         ORDER BY period_id",
    )
    .bind(fixture.semester_id)
    .fetch_all(&pool)
    .await
    .expect("batch rows should query");
    assert_eq!(batch_rows.len() as i64, batch.inserted_count);
    assert!(batch_rows.iter().all(|row| row.1 == batch_rows[0].1));

    let (deleted, semester_id) = timetable_service::delete_batch_group(&pool, batch_rows[0].1)
        .await
        .expect("batch deletion should complete");
    assert_eq!(deleted, 2);
    assert_eq!(semester_id, Some(fixture.semester_id));
    assert!(timetable_service::fetch_entry_by_id(&pool, first.id)
        .await
        .is_some());
    assert!(timetable_service::fetch_entry_by_id(&pool, second.id)
        .await
        .is_some());
}
