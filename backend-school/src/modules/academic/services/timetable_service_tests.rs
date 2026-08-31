use chrono::NaiveDate;
use uuid::Uuid;

use super::{daily_teaching_service, timetable_service};
use crate::error::AppError;
use crate::modules::academic::cutover_test_support::{
    apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
    CutoverFixture,
};
use crate::modules::academic::models::timetable::{
    CreateBatchTimetableEntriesRequest, CreateTimetableEntryRequest, SwapTimetableEntriesRequest,
    TimetableConflictType, TimetablePlacementCandidate, TimetablePlacementMutationKind,
    TimetablePlacementPreviewRequest, TimetablePlacementSource, TimetablePlacementState,
    TimetableQuery, TimetableWorkspaceQuery, UpdateTimetableEntryRequest,
};
use crate::modules::academic::models::timetable_version::CloneTimetableVersionRequest;
use crate::modules::academic::models::timetable_version::TimetableVersion;
use crate::policies::resource_access_policy::AcademicResourceListFilter;
use crate::test_helpers::{create_named_test_pool, create_named_test_pool_with_max_connections};

async fn migrated_pool(test_name: &str) -> sqlx::PgPool {
    let pool = create_named_test_pool(test_name).await;
    prepare_migrated_pool(pool).await
}

async fn concurrent_migrated_pool(test_name: &str) -> sqlx::PgPool {
    let pool = create_named_test_pool_with_max_connections(test_name, 3).await;
    prepare_migrated_pool(pool).await
}

async fn prepare_migrated_pool(pool: sqlx::PgPool) -> sqlx::PgPool {
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, 54).await.unwrap();
    sqlx::query(
        r#"INSERT INTO bell_schedule_periods (
               id, bell_schedule_id, name,
               start_time, end_time, order_index, applicable_days
           )
           SELECT gen_random_uuid(), schedule.id,
                  'คาบทดสอบ 2', TIME '10:00', TIME '10:50', 2, 'MON-FRI'
           FROM bell_schedules schedule
           WHERE schedule.is_default
             AND NOT EXISTS (
                 SELECT 1 FROM bell_schedule_periods period
                 WHERE period.bell_schedule_id = schedule.id
                   AND period.order_index = 2
             )"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    pool
}

async fn clone_editable_version(pool: &sqlx::PgPool) -> TimetableVersion {
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let (source_id, source_row_version, term_start): (Uuid, i64, NaiveDate) = sqlx::query_as(
        r#"SELECT version.id, version.row_version, term.start_date
           FROM academic_timetable_versions version
           JOIN academic_terms term ON term.id = version.academic_term_id
           WHERE version.status = 'published' AND term.status = 'active'
           ORDER BY version.effective_from, version.id LIMIT 1"#,
    )
    .fetch_one(pool)
    .await
    .unwrap();
    super::timetable_version_service::clone_draft(
        pool,
        actor_id,
        source_id,
        CloneTimetableVersionRequest {
            effective_from: term_start.succ_opt().unwrap(),
            source_row_version,
        },
    )
    .await
    .unwrap()
}

struct ExactInstructorFixture {
    draft: TimetableVersion,
    actor_id: Uuid,
    term_id: Uuid,
    group_id: Uuid,
    period_id: Uuid,
    teacher_a: Uuid,
    teacher_b: Uuid,
}

async fn exact_instructor_fixture(pool: &sqlx::PgPool) -> ExactInstructorFixture {
    let draft = clone_editable_version(pool).await;
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let (offering_id, term_id, year_id, starts_on, period_id): (Uuid, Uuid, Uuid, NaiveDate, Uuid) =
        sqlx::query_as(
            r#"SELECT offering.id, offering.academic_term_id, offering.academic_year_id,
                  offering.starts_on, period.id
           FROM learning_offerings offering
           JOIN academic_terms term ON term.id = offering.academic_term_id
           JOIN bell_schedule_periods period
             ON period.bell_schedule_id = term.bell_schedule_id
           WHERE offering.academic_term_id = $1
             AND offering.kind = 'course'
             AND period.is_active
           ORDER BY offering.id, period.order_index, period.id
           LIMIT 1"#,
        )
        .bind(draft.academic_term_id)
        .fetch_one(pool)
        .await
        .expect("fixture must contain a course offering and an active bell period");
    let group_id = Uuid::new_v4();
    let teacher_a = Uuid::new_v4();
    let teacher_b = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO users (
               id, email, username, password_hash, first_name, last_name,
               user_type, status
           ) VALUES
               ($1, $3, $4, 'fixture-not-a-login', 'ครูเอ', 'ทดสอบ', 'staff', 'active'),
               ($2, $5, $6, 'fixture-not-a-login', 'ครูบี', 'ทดสอบ', 'staff', 'active')"#,
    )
    .bind(teacher_a)
    .bind(teacher_b)
    .bind(format!("{teacher_a}@example.invalid"))
    .bind(format!("teacher-{teacher_a}"))
    .bind(format!("{teacher_b}@example.invalid"))
    .bind(format!("teacher-{teacher_b}"))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO learning_groups (
               id, learning_offering_id, academic_term_id, academic_year_id,
               code, name, status, roster_status
           ) VALUES ($1, $2, $3, $4, $5, 'กลุ่มทดสอบครูรายคาบ', 'draft', 'draft')"#,
    )
    .bind(group_id)
    .bind(offering_id)
    .bind(term_id)
    .bind(year_id)
    .bind(format!("EXACT-{}", &group_id.to_string()[..8]))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO learning_group_teachers (
               id, learning_group_id, academic_term_id, academic_year_id,
               teacher_id, role, starts_on, created_by, updated_by
           ) VALUES
               (gen_random_uuid(), $1, $2, $3, $4, 'primary', $6, $7, $7),
               (gen_random_uuid(), $1, $2, $3, $5, 'secondary', $6, $7, $7)"#,
    )
    .bind(group_id)
    .bind(term_id)
    .bind(year_id)
    .bind(teacher_a)
    .bind(teacher_b)
    .bind(starts_on)
    .bind(actor_id)
    .execute(pool)
    .await
    .unwrap();

    ExactInstructorFixture {
        draft,
        actor_id,
        term_id,
        group_id,
        period_id,
        teacher_a,
        teacher_b,
    }
}

fn instructor_ids(
    entry: &crate::modules::academic::models::timetable::TimetableEntry,
) -> Vec<Uuid> {
    let mut ids = entry
        .instructors
        .iter()
        .map(|instructor| instructor.user_id)
        .collect::<Vec<_>>();
    ids.sort_unstable();
    ids
}

fn instructor_roles_json(
    entry: &crate::modules::academic::models::timetable::TimetableEntry,
) -> serde_json::Value {
    let mut instructors = entry
        .instructors
        .iter()
        .map(|instructor| {
            (
                instructor.user_id,
                serde_json::json!({
                    "instructorId": instructor.user_id,
                    "role": instructor.role
                }),
            )
        })
        .collect::<Vec<_>>();
    instructors.sort_by_key(|(instructor_id, _)| *instructor_id);
    serde_json::Value::Array(
        instructors
            .into_iter()
            .map(|(_, instructor)| instructor)
            .collect(),
    )
}

#[tokio::test]
async fn workspace_loads_one_bounded_exact_instructor_board() {
    let pool = migrated_pool("timetable_workspace_board").await;
    let fixture = exact_instructor_fixture(&pool).await;
    let offering_id: Uuid =
        sqlx::query_scalar("SELECT learning_offering_id FROM learning_groups WHERE id = $1")
            .bind(fixture.group_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let year_id: Uuid =
        sqlx::query_scalar("SELECT academic_year_id FROM learning_groups WHERE id = $1")
            .bind(fixture.group_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let homeroom_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT id
           FROM homerooms
           WHERE academic_year_id = $1 AND is_active
           ORDER BY room_number, id
           LIMIT 2"#,
    )
    .bind(year_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(homeroom_ids.len(), 2);
    let period_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT period.id
           FROM bell_schedule_periods period
           WHERE period.bell_schedule_id = $1 AND period.is_active
           ORDER BY period.order_index, period.id
           LIMIT 2"#,
    )
    .bind(fixture.draft.bell_schedule_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(period_ids.len(), 2);
    let referenced_inactive_room_id = Uuid::new_v4();
    let unreferenced_inactive_room_id = Uuid::new_v4();
    let unreferenced_inactive_staff_id = Uuid::new_v4();
    let first_entry_id = Uuid::new_v4();
    let second_entry_id = Uuid::new_v4();

    sqlx::query("DELETE FROM academic_timetable_entries WHERE timetable_version_id = $1")
        .bind(fixture.draft.id)
        .execute(&pool)
        .await
        .unwrap();
    for homeroom_id in &homeroom_ids {
        sqlx::query(
            r#"INSERT INTO learning_group_homerooms (
                   id, learning_group_id, academic_term_id, academic_year_id,
                   homeroom_id, coverage_source
               ) VALUES (gen_random_uuid(), $1, $2, $3, $4, 'workspace_test')"#,
        )
        .bind(fixture.group_id)
        .bind(fixture.term_id)
        .bind(year_id)
        .bind(homeroom_id)
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        r#"INSERT INTO rooms (
               id, name_th, code, room_type, capacity, status
           ) VALUES
               ($1, 'ห้องอ้างอิงที่ปิดแล้ว', 'WORKSPACE-REF', 'GENERAL', 30, 'INACTIVE'),
               ($2, 'ห้องที่ปิดและไม่ถูกอ้างอิง', 'WORKSPACE-NOREF', 'GENERAL', 30, 'INACTIVE')"#,
    )
    .bind(referenced_inactive_room_id)
    .bind(unreferenced_inactive_room_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO users (
               id, email, username, password_hash, first_name, last_name,
               user_type, status
           ) VALUES ($1, $2, $3, 'fixture-not-a-login',
                     'ครูที่ไม่ถูกอ้างอิง', 'ปิดใช้งาน', 'staff', 'inactive')"#,
    )
    .bind(unreferenced_inactive_staff_id)
    .bind(format!("{unreferenced_inactive_staff_id}@example.invalid"))
    .bind(format!("inactive-{unreferenced_inactive_staff_id}"))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE users SET status = 'inactive' WHERE id = $1")
        .bind(fixture.teacher_b)
        .execute(&pool)
        .await
        .unwrap();
    let updated_target = sqlx::query(
        r#"UPDATE academic_timetable_version_targets
           SET weekly_period_target = 3
           WHERE timetable_version_id = $1 AND learning_offering_id = $2"#,
    )
    .bind(fixture.draft.id)
    .bind(offering_id)
    .execute(&pool)
    .await
    .unwrap();
    assert_eq!(updated_target.rows_affected(), 1);
    sqlx::query(
        r#"INSERT INTO academic_timetable_entries (
               id, day_of_week, bell_schedule_period_id,
               room_id, note, is_active, created_by, updated_by, entry_type, title,
               homeroom_id, academic_term_id, batch_id,
               academic_year_id, learning_offering_id, learning_group_id,
               bell_schedule_id, migration_provenance, row_version,
               timetable_version_id
           ) VALUES
               ($1, 'MON', $3, $5, NULL, true, $6, $6, 'COURSE', NULL,
                NULL, $7, NULL, $8, $9, $10, $11, '{}'::jsonb, 1, $12),
               ($2, 'TUE', $4, NULL, NULL, true, $6, $6, 'COURSE', NULL,
                NULL, $7, NULL, $8, $9, $10, $11, '{}'::jsonb, 1, $12)"#,
    )
    .bind(first_entry_id)
    .bind(second_entry_id)
    .bind(period_ids[0])
    .bind(period_ids[1])
    .bind(referenced_inactive_room_id)
    .bind(fixture.actor_id)
    .bind(fixture.term_id)
    .bind(year_id)
    .bind(offering_id)
    .bind(fixture.group_id)
    .bind(fixture.draft.bell_schedule_id)
    .bind(fixture.draft.id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO timetable_entry_instructors (
               id, entry_id, instructor_id, role
           ) VALUES
               (gen_random_uuid(), $1, $3, 'primary'),
               (gen_random_uuid(), $1, $4, 'secondary'),
               (gen_random_uuid(), $2, $3, 'primary')"#,
    )
    .bind(first_entry_id)
    .bind(second_entry_id)
    .bind(fixture.teacher_a)
    .bind(fixture.teacher_b)
    .execute(&pool)
    .await
    .unwrap();

    let workspace = timetable_service::get_workspace(
        &pool,
        TimetableWorkspaceQuery {
            academic_year_id: year_id,
            academic_term_id: fixture.term_id,
            timetable_version_id: fixture.draft.id,
        },
        &school_access(),
    )
    .await
    .unwrap();

    assert_eq!(workspace.entries.len(), 2);
    assert_eq!(workspace.entries[0].instructors.len(), 2);
    let group = workspace
        .learning_groups
        .iter()
        .find(|group| group.id == fixture.group_id)
        .unwrap();
    assert_eq!(group.homeroom_ids, homeroom_ids);
    let demand = workspace
        .unscheduled_demands
        .iter()
        .find(|demand| demand.learning_group_id == fixture.group_id)
        .unwrap();
    assert_eq!(demand.required_periods, 3);
    assert_eq!(demand.scheduled_periods, 2);
    assert_eq!(demand.remaining_periods, 1);
    assert!(workspace
        .rooms
        .iter()
        .any(|room| room.id == referenced_inactive_room_id));
    assert!(!workspace
        .rooms
        .iter()
        .any(|room| room.id == unreferenced_inactive_room_id));
    assert!(workspace
        .staff
        .iter()
        .any(|staff| staff.id == fixture.teacher_b));
    assert!(!workspace
        .staff
        .iter()
        .any(|staff| staff.id == unreferenced_inactive_staff_id));
}

#[tokio::test]
async fn workspace_rejects_a_version_outside_the_requested_academic_context() {
    let pool = migrated_pool("timetable_workspace_context_guard").await;
    let fixture = exact_instructor_fixture(&pool).await;

    let result = timetable_service::get_workspace(
        &pool,
        TimetableWorkspaceQuery {
            academic_year_id: Uuid::new_v4(),
            academic_term_id: fixture.term_id,
            timetable_version_id: fixture.draft.id,
        },
        &school_access(),
    )
    .await;

    assert!(matches!(result, Err(AppError::ValidationError(_))));
}

#[tokio::test]
async fn placement_preview_distinguishes_create_move_update_swap_and_blocked() {
    let pool = migrated_pool("timetable_placement_preview_states").await;
    let fixture = exact_instructor_fixture(&pool).await;
    let offering_id: Uuid =
        sqlx::query_scalar("SELECT learning_offering_id FROM learning_groups WHERE id = $1")
            .bind(fixture.group_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    let period_ids: Vec<Uuid> = sqlx::query_scalar(
        r#"SELECT id FROM bell_schedule_periods
           WHERE bell_schedule_id = $1 AND is_active
           ORDER BY order_index, id LIMIT 2"#,
    )
    .bind(fixture.draft.bell_schedule_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(period_ids.len(), 2);
    sqlx::query("DELETE FROM academic_timetable_entries WHERE timetable_version_id = $1")
        .bind(fixture.draft.id)
        .execute(&pool)
        .await
        .unwrap();

    let candidate = |instructor_ids: Vec<Uuid>| TimetablePlacementCandidate {
        entry_type: "COURSE".to_string(),
        learning_group_id: Some(fixture.group_id),
        learning_offering_id: Some(offering_id),
        homeroom_id: None,
        room_id: None,
        instructor_ids,
    };
    let unscheduled_source = TimetablePlacementSource::UnscheduledDemand {
        learning_group_id: fixture.group_id,
        learning_offering_id: offering_id,
    };
    let preview_create = timetable_service::preview_placement(
        &pool,
        &TimetablePlacementPreviewRequest {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            source: unscheduled_source.clone(),
            candidate: candidate(vec![fixture.teacher_a]),
            target_day_of_week: "WED".to_string(),
            target_bell_schedule_period_id: period_ids[0],
            expected_target_entry_id: None,
            expected_target_row_version: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(preview_create.state, TimetablePlacementState::Move);
    assert_eq!(
        preview_create.mutation,
        Some(TimetablePlacementMutationKind::Create)
    );

    let source = timetable_service::create_entry(
        &pool,
        fixture.actor_id,
        CreateTimetableEntryRequest {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            learning_group_id: Some(fixture.group_id),
            homeroom_id: None,
            day_of_week: "MON".to_string(),
            bell_schedule_period_id: period_ids[0],
            room_id: None,
            note: None,
            entry_type: "COURSE".to_string(),
            title: None,
            instructor_ids: vec![fixture.teacher_a],
        },
    )
    .await
    .unwrap();
    let existing_source = TimetablePlacementSource::ExistingEntry {
        entry_id: source.id,
        row_version: source.row_version,
    };
    let preview_move = timetable_service::preview_placement(
        &pool,
        &TimetablePlacementPreviewRequest {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            source: existing_source.clone(),
            candidate: candidate(vec![fixture.teacher_a]),
            target_day_of_week: "THU".to_string(),
            target_bell_schedule_period_id: period_ids[1],
            expected_target_entry_id: None,
            expected_target_row_version: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(preview_move.state, TimetablePlacementState::Move);
    assert_eq!(
        preview_move.mutation,
        Some(TimetablePlacementMutationKind::Move)
    );

    let preview_update = timetable_service::preview_placement(
        &pool,
        &TimetablePlacementPreviewRequest {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            source: existing_source.clone(),
            candidate: candidate(vec![fixture.teacher_a, fixture.teacher_b]),
            target_day_of_week: source.day_of_week.clone(),
            target_bell_schedule_period_id: source.bell_schedule_period_id,
            expected_target_entry_id: None,
            expected_target_row_version: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(preview_update.state, TimetablePlacementState::Move);
    assert_eq!(
        preview_update.mutation,
        Some(TimetablePlacementMutationKind::Update)
    );

    let target = timetable_service::create_entry(
        &pool,
        fixture.actor_id,
        CreateTimetableEntryRequest {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            learning_group_id: Some(fixture.group_id),
            homeroom_id: None,
            day_of_week: "TUE".to_string(),
            bell_schedule_period_id: period_ids[1],
            room_id: None,
            note: None,
            entry_type: "COURSE".to_string(),
            title: None,
            instructor_ids: vec![fixture.teacher_b],
        },
    )
    .await
    .unwrap();
    let preview_swap = timetable_service::preview_placement(
        &pool,
        &TimetablePlacementPreviewRequest {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            source: existing_source,
            candidate: candidate(vec![fixture.teacher_a]),
            target_day_of_week: target.day_of_week.clone(),
            target_bell_schedule_period_id: target.bell_schedule_period_id,
            expected_target_entry_id: Some(target.id),
            expected_target_row_version: Some(target.row_version),
        },
    )
    .await
    .unwrap();
    assert_eq!(preview_swap.state, TimetablePlacementState::Swap);
    assert_eq!(
        preview_swap.mutation,
        Some(TimetablePlacementMutationKind::Swap)
    );

    let preview_blocked = timetable_service::preview_placement(
        &pool,
        &TimetablePlacementPreviewRequest {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            source: unscheduled_source,
            candidate: candidate(vec![fixture.teacher_a]),
            target_day_of_week: target.day_of_week,
            target_bell_schedule_period_id: target.bell_schedule_period_id,
            expected_target_entry_id: Some(target.id),
            expected_target_row_version: Some(target.row_version),
        },
    )
    .await
    .unwrap();
    assert_eq!(preview_blocked.state, TimetablePlacementState::Blocked);
    assert_eq!(preview_blocked.mutation, None);
    assert!(!preview_blocked.conflicts.is_empty());

    let swapped = timetable_service::swap_entries(
        &pool,
        fixture.actor_id,
        SwapTimetableEntriesRequest {
            timetable_version_id: fixture.draft.id,
            entry_a_id: source.id,
            entry_a_row_version: source.row_version,
            entry_b_id: target.id,
            entry_b_row_version: target.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(swapped.entry_a.day_of_week, preview_swap.target_day_of_week);
    assert_eq!(
        swapped.entry_a.bell_schedule_period_id,
        preview_swap.target_bell_schedule_period_id
    );
    assert_eq!(swapped.entry_b.day_of_week, source.day_of_week);
    assert_eq!(
        swapped.entry_b.bell_schedule_period_id,
        source.bell_schedule_period_id
    );
}

#[tokio::test]
async fn placement_preview_reports_stale_exact_instructor_and_published_version_blocks() {
    let pool = migrated_pool("timetable_placement_preview_blocks").await;
    let fixture = exact_instructor_fixture(&pool).await;
    let offering_id: Uuid =
        sqlx::query_scalar("SELECT learning_offering_id FROM learning_groups WHERE id = $1")
            .bind(fixture.group_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    sqlx::query(
        r#"UPDATE academic_timetable_version_targets
           SET weekly_period_target = 3
           WHERE timetable_version_id = $1 AND learning_offering_id = $2"#,
    )
    .bind(fixture.draft.id)
    .bind(offering_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("DELETE FROM academic_timetable_entries WHERE timetable_version_id = $1")
        .bind(fixture.draft.id)
        .execute(&pool)
        .await
        .unwrap();
    let existing = timetable_service::create_entry(
        &pool,
        fixture.actor_id,
        CreateTimetableEntryRequest {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            learning_group_id: Some(fixture.group_id),
            homeroom_id: None,
            day_of_week: "FRI".to_string(),
            bell_schedule_period_id: fixture.period_id,
            room_id: None,
            note: None,
            entry_type: "COURSE".to_string(),
            title: None,
            instructor_ids: vec![fixture.teacher_a],
        },
    )
    .await
    .unwrap();
    let request = TimetablePlacementPreviewRequest {
        timetable_version_id: fixture.draft.id,
        academic_term_id: fixture.term_id,
        source: TimetablePlacementSource::ExistingEntry {
            entry_id: existing.id,
            row_version: existing.row_version + 1,
        },
        candidate: TimetablePlacementCandidate {
            entry_type: "COURSE".to_string(),
            learning_group_id: Some(fixture.group_id),
            learning_offering_id: Some(offering_id),
            homeroom_id: None,
            room_id: None,
            instructor_ids: vec![fixture.teacher_a],
        },
        target_day_of_week: "THU".to_string(),
        target_bell_schedule_period_id: fixture.period_id,
        expected_target_entry_id: None,
        expected_target_row_version: None,
    };
    let stale = timetable_service::preview_placement(&pool, &request)
        .await
        .unwrap();
    assert_eq!(stale.state, TimetablePlacementState::Blocked);
    assert!(stale
        .conflicts
        .iter()
        .any(|conflict| { conflict.conflict_type == TimetableConflictType::StaleEntry }));

    timetable_service::create_entry(
        &pool,
        fixture.actor_id,
        CreateTimetableEntryRequest {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            learning_group_id: None,
            homeroom_id: None,
            day_of_week: "THU".to_string(),
            bell_schedule_period_id: fixture.period_id,
            room_id: None,
            note: None,
            entry_type: "ACADEMIC".to_string(),
            title: Some("งานครูเอ".to_string()),
            instructor_ids: vec![fixture.teacher_a],
        },
    )
    .await
    .unwrap();
    let instructor_block = timetable_service::preview_placement(
        &pool,
        &TimetablePlacementPreviewRequest {
            source: TimetablePlacementSource::UnscheduledDemand {
                learning_group_id: fixture.group_id,
                learning_offering_id: offering_id,
            },
            candidate: request.candidate.clone(),
            target_day_of_week: "THU".to_string(),
            expected_target_entry_id: None,
            expected_target_row_version: None,
            ..request.clone()
        },
    )
    .await
    .unwrap();
    assert_eq!(instructor_block.state, TimetablePlacementState::Blocked);
    assert!(instructor_block
        .conflicts
        .iter()
        .any(|conflict| conflict.conflict_type == TimetableConflictType::Instructor));

    sqlx::query(
        r#"UPDATE academic_timetable_versions
           SET status = 'published', published_by = $2, published_at = now()
           WHERE id = $1"#,
    )
    .bind(fixture.draft.id)
    .bind(fixture.actor_id)
    .execute(&pool)
    .await
    .unwrap();
    let published = timetable_service::preview_placement(
        &pool,
        &TimetablePlacementPreviewRequest {
            source: TimetablePlacementSource::ExistingEntry {
                entry_id: existing.id,
                row_version: existing.row_version,
            },
            ..request
        },
    )
    .await
    .unwrap();
    assert_eq!(published.state, TimetablePlacementState::Blocked);
    assert!(published
        .conflicts
        .iter()
        .any(|conflict| { conflict.conflict_type == TimetableConflictType::Version }));
}

#[tokio::test]
async fn legacy_teacher_conflict_diagnostic_names_every_entry_in_the_slot() {
    let pool = migrated_pool("timetable_legacy_teacher_conflict_diagnostic").await;
    let fixture = exact_instructor_fixture(&pool).await;
    let second_group_id = Uuid::new_v4();
    let first_entry_id = Uuid::new_v4();
    let second_entry_id = Uuid::new_v4();

    sqlx::query(
        r#"INSERT INTO learning_groups (
               id, learning_offering_id, academic_term_id, academic_year_id,
               code, name, status, roster_status
           )
           SELECT $1, learning_offering_id, academic_term_id, academic_year_id,
                  E'LEGACY-DIAG-2\nforged=true', 'กลุ่มวินิจฉัยที่สอง', 'draft', 'draft'
           FROM learning_groups
           WHERE id = $2"#,
    )
    .bind(second_group_id)
    .bind(fixture.group_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO learning_group_teachers (
               id, learning_group_id, academic_term_id, academic_year_id,
               teacher_id, role, starts_on, created_by, updated_by
           )
           SELECT gen_random_uuid(), $1, learning_group.academic_term_id,
                  learning_group.academic_year_id, $3, 'primary',
                  offering.starts_on, $4, $4
           FROM learning_groups learning_group
           JOIN learning_offerings offering
             ON offering.id = learning_group.learning_offering_id
           WHERE learning_group.id = $2"#,
    )
    .bind(second_group_id)
    .bind(second_group_id)
    .bind(fixture.teacher_a)
    .bind(fixture.actor_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO academic_timetable_entries (
               id, day_of_week, bell_schedule_period_id,
               room_id, note, is_active, created_by, updated_by, entry_type, title,
               homeroom_id, academic_term_id, batch_id,
               academic_year_id, learning_offering_id, learning_group_id,
               bell_schedule_id, migration_provenance, row_version,
               timetable_version_id
           )
           SELECT requested.entry_id, 'TUE', $3,
                  NULL, NULL, true, $4, $4, 'COURSE', NULL,
                  NULL, learning_group.academic_term_id, NULL,
                  learning_group.academic_year_id, learning_group.learning_offering_id,
                  learning_group.id, term.bell_schedule_id, '{}'::jsonb, 1, $5
           FROM (
               VALUES ($1::uuid, $6::uuid), ($2::uuid, $7::uuid)
           ) AS requested(entry_id, learning_group_id)
           JOIN learning_groups learning_group
             ON learning_group.id = requested.learning_group_id
           JOIN academic_terms term ON term.id = learning_group.academic_term_id"#,
    )
    .bind(first_entry_id)
    .bind(second_entry_id)
    .bind(fixture.period_id)
    .bind(fixture.actor_id)
    .bind(fixture.draft.id)
    .bind(fixture.group_id)
    .bind(second_group_id)
    .execute(&pool)
    .await
    .unwrap();

    let conflicts = timetable_service::list_legacy_current_teacher_conflicts(&pool)
        .await
        .unwrap();

    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].teacher_id, fixture.teacher_a);
    assert_eq!(conflicts[0].timetable_version_id, fixture.draft.id);
    assert_eq!(conflicts[0].day_of_week, "TUE");
    assert_eq!(conflicts[0].bell_schedule_period_id, fixture.period_id);
    assert_eq!(conflicts[0].entry_count, 2);
    assert_eq!(conflicts[0].group_code_count, 2);
    let mut expected_entry_ids = vec![first_entry_id, second_entry_id];
    expected_entry_ids.sort_unstable();
    assert_eq!(conflicts[0].entry_ids, expected_entry_ids);
    assert_eq!(
        conflicts[0].group_codes,
        vec![
            format!("EXACT-{}", &fixture.group_id.to_string()[..8]),
            "LEGACY-DIAG-2 forged=true".to_string(),
        ]
    );
}

#[tokio::test]
async fn legacy_teacher_conflict_diagnostic_caps_each_conflict_bucket() {
    let pool = migrated_pool("timetable_legacy_teacher_conflict_bounds").await;
    let fixture = exact_instructor_fixture(&pool).await;

    sqlx::query(
        r#"WITH source AS (
               SELECT learning_group.learning_offering_id,
                      learning_group.academic_term_id,
                      learning_group.academic_year_id,
                      offering.starts_on,
                      term.bell_schedule_id
               FROM learning_groups learning_group
               JOIN learning_offerings offering
                 ON offering.id = learning_group.learning_offering_id
               JOIN academic_terms term
                 ON term.id = learning_group.academic_term_id
               WHERE learning_group.id = $1
           ), requested AS (
               SELECT sequence,
                      gen_random_uuid() AS group_id,
                      gen_random_uuid() AS entry_id
               FROM generate_series(1, 25) AS sequence
           ), inserted_groups AS (
               INSERT INTO learning_groups (
                   id, learning_offering_id, academic_term_id, academic_year_id,
                   code, name, status, roster_status
               )
               SELECT requested.group_id, source.learning_offering_id,
                      source.academic_term_id, source.academic_year_id,
                      CASE WHEN requested.sequence = 25
                           THEN repeat('X', 120)
                           ELSE format('BOUND-%s', requested.sequence)
                      END,
                      format('กลุ่มวินิจฉัยขอบเขต %s', requested.sequence),
                      'draft', 'draft'
               FROM requested CROSS JOIN source
               RETURNING id
           ), inserted_teachers AS (
               INSERT INTO learning_group_teachers (
                   id, learning_group_id, academic_term_id, academic_year_id,
                   teacher_id, role, starts_on, created_by, updated_by
               )
               SELECT gen_random_uuid(), requested.group_id,
                      source.academic_term_id, source.academic_year_id,
                      $2, 'primary', source.starts_on, $3, $3
               FROM requested CROSS JOIN source
               RETURNING learning_group_id
           )
           INSERT INTO academic_timetable_entries (
               id, day_of_week, bell_schedule_period_id,
               room_id, note, is_active, created_by, updated_by, entry_type, title,
               homeroom_id, academic_term_id, batch_id,
               academic_year_id, learning_offering_id, learning_group_id,
               bell_schedule_id, migration_provenance, row_version,
               timetable_version_id
           )
           SELECT requested.entry_id, 'WED', $4,
                  NULL, NULL, true, $3, $3, 'COURSE', NULL,
                  NULL, source.academic_term_id, NULL,
                  source.academic_year_id, source.learning_offering_id,
                  requested.group_id, source.bell_schedule_id, '{}'::jsonb, 1, $5
           FROM requested CROSS JOIN source
           JOIN inserted_groups ON inserted_groups.id = requested.group_id
           JOIN inserted_teachers
             ON inserted_teachers.learning_group_id = requested.group_id"#,
    )
    .bind(fixture.group_id)
    .bind(fixture.teacher_a)
    .bind(fixture.actor_id)
    .bind(fixture.period_id)
    .bind(fixture.draft.id)
    .execute(&pool)
    .await
    .unwrap();

    let conflicts = timetable_service::list_legacy_current_teacher_conflicts(&pool)
        .await
        .unwrap();
    let conflict = conflicts
        .iter()
        .find(|conflict| conflict.day_of_week == "WED")
        .unwrap();

    assert_eq!(conflict.entry_count, 25);
    assert_eq!(conflict.group_code_count, 25);
    assert_eq!(conflict.entry_ids.len(), 20);
    assert_eq!(conflict.group_codes.len(), 20);
    assert!(conflict
        .group_codes
        .iter()
        .all(|code| code.chars().count() <= 80 && !code.chars().any(char::is_control)));
}

#[tokio::test]
async fn timetable_entries_split_and_coteach_with_exact_instructors() {
    let pool = migrated_pool("timetable_exact_instructor_split_coteach").await;
    let fixture = exact_instructor_fixture(&pool).await;
    let mut created = Vec::new();
    for (day, selected) in [
        ("MON", vec![fixture.teacher_a]),
        ("WED", vec![fixture.teacher_b]),
        ("FRI", vec![fixture.teacher_a, fixture.teacher_b]),
    ] {
        created.push(
            timetable_service::create_entry(
                &pool,
                fixture.actor_id,
                CreateTimetableEntryRequest {
                    timetable_version_id: fixture.draft.id,
                    academic_term_id: fixture.term_id,
                    learning_group_id: Some(fixture.group_id),
                    homeroom_id: None,
                    day_of_week: day.to_string(),
                    bell_schedule_period_id: fixture.period_id,
                    room_id: None,
                    note: None,
                    entry_type: "COURSE".to_string(),
                    title: None,
                    instructor_ids: selected,
                },
            )
            .await
            .unwrap(),
        );
    }

    assert_eq!(instructor_ids(&created[0]), vec![fixture.teacher_a]);
    assert_eq!(instructor_ids(&created[1]), vec![fixture.teacher_b]);
    let mut expected_coteachers = vec![fixture.teacher_a, fixture.teacher_b];
    expected_coteachers.sort_unstable();
    assert_eq!(instructor_ids(&created[2]), expected_coteachers);

    let teacher_a_entries = timetable_service::list_entries(
        &pool,
        &TimetableQuery {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            learning_group_id: Some(fixture.group_id),
            homeroom_id: None,
            instructor_id: Some(fixture.teacher_a),
            room_id: None,
            day_of_week: None,
            entry_type: None,
        },
        &school_access(),
    )
    .await
    .unwrap();
    assert_eq!(
        teacher_a_entries
            .iter()
            .map(|entry| entry.day_of_week.as_str())
            .collect::<Vec<_>>(),
        vec!["FRI", "MON"]
    );
    let teacher_b_entries = timetable_service::list_entries(
        &pool,
        &TimetableQuery {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            learning_group_id: Some(fixture.group_id),
            homeroom_id: None,
            instructor_id: Some(fixture.teacher_b),
            room_id: None,
            day_of_week: None,
            entry_type: None,
        },
        &school_access(),
    )
    .await
    .unwrap();
    assert_eq!(
        teacher_b_entries
            .iter()
            .map(|entry| entry.day_of_week.as_str())
            .collect::<Vec<_>>(),
        vec!["FRI", "WED"]
    );

    let candidate_for_unscheduled_group_teacher = CreateTimetableEntryRequest {
        timetable_version_id: fixture.draft.id,
        academic_term_id: fixture.term_id,
        learning_group_id: None,
        homeroom_id: None,
        day_of_week: "WED".to_string(),
        bell_schedule_period_id: fixture.period_id,
        room_id: None,
        note: None,
        entry_type: "ACADEMIC".to_string(),
        title: Some("งานครูเอต่างหาก".to_string()),
        instructor_ids: vec![fixture.teacher_a],
    };
    assert!(
        timetable_service::validate_candidate(&pool, &candidate_for_unscheduled_group_teacher)
            .await
            .unwrap()
            .is_valid,
        "a teacher assigned to the group but not to its Wednesday entry remains available"
    );
    let teacher_b_conflict = timetable_service::validate_candidate(
        &pool,
        &CreateTimetableEntryRequest {
            instructor_ids: vec![fixture.teacher_b],
            title: Some("งานครูบีชนคาบ".to_string()),
            ..candidate_for_unscheduled_group_teacher.clone()
        },
    )
    .await
    .unwrap();
    assert!(!teacher_b_conflict.is_valid);
    assert!(teacher_b_conflict
        .conflicts
        .iter()
        .any(|conflict| conflict.conflict_type == TimetableConflictType::Instructor));

    let occupancy = timetable_service::occupancy(&pool, fixture.draft.id, fixture.term_id)
        .await
        .unwrap();
    for entry in &created {
        let cell = occupancy
            .iter()
            .find(|cell| cell.entry_id == entry.id)
            .unwrap();
        assert_eq!(cell.instructor_ids, instructor_ids(entry));
    }
}

#[tokio::test]
async fn daily_teaching_returns_only_periods_the_staff_member_exactly_teaches() {
    let pool = migrated_pool("daily_teaching_exact_entry_instructors").await;
    let fixture = exact_instructor_fixture(&pool).await;
    let created = timetable_service::create_entry(
        &pool,
        fixture.actor_id,
        CreateTimetableEntryRequest {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            learning_group_id: Some(fixture.group_id),
            homeroom_id: None,
            day_of_week: "WED".to_string(),
            bell_schedule_period_id: fixture.period_id,
            room_id: None,
            note: None,
            entry_type: "COURSE".to_string(),
            title: None,
            instructor_ids: vec![fixture.teacher_b],
        },
    )
    .await
    .unwrap();
    sqlx::query(
        r#"UPDATE academic_timetable_versions
           SET status = 'published', published_by = $2, published_at = now()
           WHERE id = $1"#,
    )
    .bind(fixture.draft.id)
    .bind(fixture.actor_id)
    .execute(&pool)
    .await
    .unwrap();
    let mut observed_date = fixture.draft.effective_from;
    while daily_teaching_service::day_code_from_date(observed_date) != "WED" {
        observed_date = observed_date.succ_opt().unwrap();
    }

    let overview = daily_teaching_service::get_daily_teaching_overview(
        &pool,
        daily_teaching_service::DailyTeachingQuery {
            academic_term_id: fixture.term_id,
            date: Some(observed_date),
            include_empty_teachers: Some(false),
        },
    )
    .await
    .unwrap();
    assert!(!overview
        .teachers
        .iter()
        .any(|teacher| teacher.id == fixture.teacher_a));
    let teacher_b = overview
        .teachers
        .iter()
        .find(|teacher| teacher.id == fixture.teacher_b)
        .expect("the exact Wednesday teacher must be present");
    assert!(teacher_b
        .periods
        .iter()
        .flat_map(|period| &period.entries)
        .any(|entry| entry.entry_id == created.id));
}

#[tokio::test]
async fn template_entry_creation_preserves_validated_selected_instructor_order() {
    let pool = migrated_pool("template_entry_preserves_selected_instructor_order").await;
    let fixture = exact_instructor_fixture(&pool).await;
    let request = CreateTimetableEntryRequest {
        timetable_version_id: fixture.draft.id,
        academic_term_id: fixture.term_id,
        learning_group_id: Some(fixture.group_id),
        homeroom_id: None,
        day_of_week: "THU".to_string(),
        bell_schedule_period_id: fixture.period_id,
        room_id: None,
        note: None,
        entry_type: "COURSE".to_string(),
        title: None,
        instructor_ids: vec![fixture.teacher_b, fixture.teacher_a],
    };
    let mut transaction = pool.begin().await.unwrap();
    let entry_id = timetable_service::create_entry_in_tx_preserving_instructor_order(
        &mut transaction,
        fixture.actor_id,
        None,
        &request,
    )
    .await
    .unwrap();
    transaction.commit().await.unwrap();

    let created = timetable_service::get_entry(&pool, entry_id).await.unwrap();
    assert_eq!(created.instructors.len(), 2);
    assert_eq!(created.instructors[0].user_id, fixture.teacher_b);
    assert_eq!(created.instructors[0].role, "primary");
    assert_eq!(created.instructors[1].user_id, fixture.teacher_a);
    assert_eq!(created.instructors[1].role, "secondary");
}

#[tokio::test]
async fn timetable_create_maps_a_concurrent_database_guard_to_conflict() {
    let pool = concurrent_migrated_pool("timetable_concurrent_guard_mapping").await;
    let fixture = exact_instructor_fixture(&pool).await;
    let trigger_lock_key = format!("{}:{}:{}", fixture.draft.id, "MON", fixture.period_id);
    let mut guard_transaction = pool.begin().await.unwrap();
    sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1, 0))")
        .bind(&trigger_lock_key)
        .execute(&mut *guard_transaction)
        .await
        .unwrap();

    let service_pool = pool.clone();
    let timetable_version_id = fixture.draft.id;
    let academic_term_id = fixture.term_id;
    let learning_group_id = fixture.group_id;
    let bell_schedule_period_id = fixture.period_id;
    let actor_user_id = fixture.actor_id;
    let create_task = tokio::spawn(async move {
        timetable_service::create_entry(
            &service_pool,
            actor_user_id,
            CreateTimetableEntryRequest {
                timetable_version_id,
                academic_term_id,
                learning_group_id: Some(learning_group_id),
                homeroom_id: None,
                day_of_week: "MON".to_string(),
                bell_schedule_period_id,
                room_id: None,
                note: None,
                entry_type: "COURSE".to_string(),
                title: None,
                instructor_ids: vec![fixture.teacher_a],
            },
        )
        .await
    });

    let mut service_is_waiting_on_guard = false;
    for _ in 0..100 {
        service_is_waiting_on_guard = sqlx::query_scalar(
            r#"SELECT EXISTS (
                   SELECT 1
                   FROM pg_stat_activity
                   WHERE pid <> pg_backend_pid()
                     AND wait_event_type = 'Lock'
                     AND wait_event = 'advisory'
               )"#,
        )
        .fetch_one(&mut *guard_transaction)
        .await
        .unwrap();
        if service_is_waiting_on_guard {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
    }
    assert!(
        service_is_waiting_on_guard,
        "service must reach the database guard after its preflight"
    );

    sqlx::query(
        r#"INSERT INTO academic_timetable_entries (
               id, day_of_week, bell_schedule_period_id,
               room_id, note, is_active, created_by, updated_by, entry_type, title,
               homeroom_id, academic_term_id, batch_id,
               academic_year_id, learning_offering_id, learning_group_id,
               bell_schedule_id, migration_provenance, row_version,
               timetable_version_id
           )
           SELECT gen_random_uuid(), 'MON', $2,
                  NULL, NULL, true, $3, $3, 'COURSE', 'concurrent guard fixture',
                  NULL, learning_group.academic_term_id, NULL,
                  learning_group.academic_year_id, learning_group.learning_offering_id,
                  learning_group.id, term.bell_schedule_id, '{}'::jsonb, 1, $4
           FROM learning_groups learning_group
           JOIN academic_terms term ON term.id = learning_group.academic_term_id
           WHERE learning_group.id = $1"#,
    )
    .bind(fixture.group_id)
    .bind(fixture.period_id)
    .bind(fixture.actor_id)
    .bind(fixture.draft.id)
    .execute(&mut *guard_transaction)
    .await
    .unwrap();
    guard_transaction.commit().await.unwrap();

    let result = create_task.await.unwrap();
    assert!(matches!(result, Err(AppError::Conflict(_))));
}

#[tokio::test]
async fn timetable_update_replaces_the_complete_instructor_set_atomically() {
    let pool = migrated_pool("timetable_exact_instructor_update").await;
    let fixture = exact_instructor_fixture(&pool).await;
    let created = timetable_service::create_entry(
        &pool,
        fixture.actor_id,
        CreateTimetableEntryRequest {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            learning_group_id: Some(fixture.group_id),
            homeroom_id: None,
            day_of_week: "MON".to_string(),
            bell_schedule_period_id: fixture.period_id,
            room_id: None,
            note: None,
            entry_type: "COURSE".to_string(),
            title: None,
            instructor_ids: vec![fixture.teacher_a],
        },
    )
    .await
    .unwrap();
    let created_payload: serde_json::Value = sqlx::query_scalar(
        r#"SELECT payload
           FROM academic_audit_events
           WHERE event_code = 'academic_timetable_entry.created'
             AND entity_id = $1
           ORDER BY created_at DESC, id DESC
           LIMIT 1"#,
    )
    .bind(created.id)
    .fetch_one(&pool)
    .await
    .expect("entry creation must append one atomic audit event");
    assert_eq!(created_payload["actorUserId"], fixture.actor_id.to_string());
    assert_eq!(
        created_payload["academicTermId"],
        fixture.term_id.to_string()
    );
    assert_eq!(created_payload["oldRowVersion"], 0);
    assert_eq!(created_payload["newRowVersion"], created.row_version);
    assert_eq!(created_payload["before"]["isActive"], false);
    assert_eq!(created_payload["after"]["isActive"], true);
    assert_eq!(
        created_payload["after"]["instructors"],
        serde_json::json!([{
            "instructorId": fixture.teacher_a,
            "role": "primary"
        }])
    );
    let updated = timetable_service::update_entry(
        &pool,
        created.id,
        fixture.actor_id,
        UpdateTimetableEntryRequest {
            timetable_version_id: fixture.draft.id,
            row_version: created.row_version,
            day_of_week: None,
            bell_schedule_period_id: None,
            room_id: None,
            clear_room: None,
            note: None,
            clear_note: None,
            title: None,
            instructor_ids: Some(vec![fixture.teacher_a, fixture.teacher_b]),
        },
    )
    .await
    .unwrap();

    let mut expected = vec![fixture.teacher_a, fixture.teacher_b];
    expected.sort_unstable();
    assert_eq!(instructor_ids(&updated), expected);
    assert_eq!(updated.row_version, created.row_version + 1);

    let cleared = timetable_service::update_entry(
        &pool,
        created.id,
        fixture.actor_id,
        UpdateTimetableEntryRequest {
            timetable_version_id: fixture.draft.id,
            row_version: updated.row_version,
            day_of_week: None,
            bell_schedule_period_id: None,
            room_id: None,
            clear_room: None,
            note: None,
            clear_note: None,
            title: None,
            instructor_ids: Some(Vec::new()),
        },
    )
    .await
    .unwrap();
    assert!(cleared.instructors.is_empty());
    assert_eq!(cleared.row_version, updated.row_version + 1);
}

#[tokio::test]
async fn timetable_update_moves_slot_and_hands_off_to_an_available_instructor_atomically() {
    let pool = migrated_pool("timetable_exact_instructor_move_handoff").await;
    let fixture = exact_instructor_fixture(&pool).await;
    let blocker = timetable_service::create_entry(
        &pool,
        fixture.actor_id,
        CreateTimetableEntryRequest {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            learning_group_id: None,
            homeroom_id: None,
            day_of_week: "WED".to_string(),
            bell_schedule_period_id: fixture.period_id,
            room_id: None,
            note: None,
            entry_type: "ACADEMIC".to_string(),
            title: Some("งานครูเอ".to_string()),
            instructor_ids: vec![fixture.teacher_a],
        },
    )
    .await
    .unwrap();
    let movable = timetable_service::create_entry(
        &pool,
        fixture.actor_id,
        CreateTimetableEntryRequest {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            learning_group_id: Some(fixture.group_id),
            homeroom_id: None,
            day_of_week: "MON".to_string(),
            bell_schedule_period_id: fixture.period_id,
            room_id: None,
            note: None,
            entry_type: "COURSE".to_string(),
            title: None,
            instructor_ids: vec![fixture.teacher_a],
        },
    )
    .await
    .unwrap();

    let moved = timetable_service::update_entry(
        &pool,
        movable.id,
        fixture.actor_id,
        UpdateTimetableEntryRequest {
            timetable_version_id: fixture.draft.id,
            row_version: movable.row_version,
            day_of_week: Some("WED".to_string()),
            bell_schedule_period_id: None,
            room_id: None,
            clear_room: None,
            note: None,
            clear_note: None,
            title: None,
            instructor_ids: Some(vec![fixture.teacher_b]),
        },
    )
    .await
    .expect("moving and replacing the complete teacher set must be one atomic change");

    assert_eq!(moved.day_of_week, "WED");
    assert_eq!(instructor_ids(&moved), vec![fixture.teacher_b]);
    assert_eq!(instructor_ids(&blocker), vec![fixture.teacher_a]);
}

#[tokio::test]
async fn timetable_update_rejects_empty_structural_scope_without_homeroom() {
    let pool = migrated_pool("timetable_structural_scope_update_guard").await;
    let fixture = exact_instructor_fixture(&pool).await;
    let created = timetable_service::create_entry(
        &pool,
        fixture.actor_id,
        CreateTimetableEntryRequest {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            learning_group_id: None,
            homeroom_id: None,
            day_of_week: "MON".to_string(),
            bell_schedule_period_id: fixture.period_id,
            room_id: None,
            note: None,
            entry_type: "ACADEMIC".to_string(),
            title: Some("งานครู".to_string()),
            instructor_ids: vec![fixture.teacher_a],
        },
    )
    .await
    .unwrap();

    let result = timetable_service::update_entry(
        &pool,
        created.id,
        fixture.actor_id,
        UpdateTimetableEntryRequest {
            timetable_version_id: fixture.draft.id,
            row_version: created.row_version,
            day_of_week: None,
            bell_schedule_period_id: None,
            room_id: None,
            clear_room: None,
            note: None,
            clear_note: None,
            title: None,
            instructor_ids: Some(Vec::new()),
        },
    )
    .await;

    assert!(matches!(result, Err(AppError::ValidationError(_))));
}

#[tokio::test]
async fn timetable_rejects_instructor_outside_group_effective_date() {
    let pool = migrated_pool("timetable_exact_instructor_effective_date").await;
    let fixture = exact_instructor_fixture(&pool).await;
    let starts_after_version = fixture
        .draft
        .effective_from
        .checked_add_signed(chrono::Duration::days(1))
        .unwrap();
    sqlx::query(
        "UPDATE learning_group_teachers SET starts_on = $1 WHERE learning_group_id = $2 AND teacher_id = $3",
    )
    .bind(starts_after_version)
    .bind(fixture.group_id)
    .bind(fixture.teacher_b)
    .execute(&pool)
    .await
    .unwrap();

    let result = timetable_service::create_entry(
        &pool,
        fixture.actor_id,
        CreateTimetableEntryRequest {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            learning_group_id: Some(fixture.group_id),
            homeroom_id: None,
            day_of_week: "MON".to_string(),
            bell_schedule_period_id: fixture.period_id,
            room_id: None,
            note: None,
            entry_type: "COURSE".to_string(),
            title: None,
            instructor_ids: vec![fixture.teacher_b],
        },
    )
    .await;
    assert!(matches!(result, Err(AppError::ValidationError(_))));
}

#[tokio::test]
async fn timetable_teacher_set_change_audits_exact_before_and_after_sets() {
    let pool = migrated_pool("timetable_exact_instructor_audit").await;
    let fixture = exact_instructor_fixture(&pool).await;
    let created = timetable_service::create_entry(
        &pool,
        fixture.actor_id,
        CreateTimetableEntryRequest {
            timetable_version_id: fixture.draft.id,
            academic_term_id: fixture.term_id,
            learning_group_id: Some(fixture.group_id),
            homeroom_id: None,
            day_of_week: "MON".to_string(),
            bell_schedule_period_id: fixture.period_id,
            room_id: None,
            note: None,
            entry_type: "COURSE".to_string(),
            title: None,
            instructor_ids: vec![fixture.teacher_a],
        },
    )
    .await
    .unwrap();
    let updated = timetable_service::update_entry(
        &pool,
        created.id,
        fixture.actor_id,
        UpdateTimetableEntryRequest {
            timetable_version_id: fixture.draft.id,
            row_version: created.row_version,
            day_of_week: None,
            bell_schedule_period_id: None,
            room_id: None,
            clear_room: None,
            note: None,
            clear_note: None,
            title: None,
            instructor_ids: Some(vec![fixture.teacher_b, fixture.teacher_a]),
        },
    )
    .await
    .unwrap();
    let (actor_user_id, payload): (Uuid, serde_json::Value) = sqlx::query_as(
        r#"SELECT actor_user_id, payload
           FROM academic_audit_events
           WHERE event_code = 'academic_timetable_entry.updated'
             AND entity_id = $1
           ORDER BY created_at DESC, id DESC
           LIMIT 1"#,
    )
    .bind(created.id)
    .fetch_one(&pool)
    .await
    .expect("entry update must append one atomic audit event");

    let mut expected_after = vec![fixture.teacher_a.to_string(), fixture.teacher_b.to_string()];
    expected_after.sort_unstable();
    let mut expected_after_instructors = vec![
        (fixture.teacher_a, "primary"),
        (fixture.teacher_b, "secondary"),
    ];
    expected_after_instructors.sort_by_key(|(instructor_id, _)| *instructor_id);
    let expected_after_instructors = expected_after_instructors
        .iter()
        .map(|(instructor_id, role)| {
            serde_json::json!({
                "instructorId": instructor_id,
                "role": role
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(actor_user_id, fixture.actor_id);
    assert_eq!(payload["entryId"], created.id.to_string());
    assert_eq!(payload["timetableVersionId"], fixture.draft.id.to_string());
    assert_eq!(payload["actorUserId"], fixture.actor_id.to_string());
    assert_eq!(payload["oldRowVersion"], created.row_version);
    assert_eq!(payload["newRowVersion"], updated.row_version);
    assert_eq!(
        payload["before"]["instructorIds"],
        serde_json::json!([fixture.teacher_a])
    );
    assert_eq!(
        payload["after"]["instructorIds"],
        serde_json::json!(expected_after)
    );
    assert_eq!(
        payload["before"]["instructors"],
        serde_json::json!([{
            "instructorId": fixture.teacher_a,
            "role": "primary"
        }])
    );
    assert_eq!(
        payload["after"]["instructors"],
        serde_json::json!(expected_after_instructors)
    );

    let deactivated = timetable_service::deactivate_entry(
        &pool,
        updated.id,
        fixture.draft.id,
        updated.row_version,
        fixture.actor_id,
    )
    .await
    .unwrap();
    let deactivated_payload: serde_json::Value = sqlx::query_scalar(
        r#"SELECT payload
           FROM academic_audit_events
           WHERE event_code = 'academic_timetable_entry.deactivated'
             AND entity_id = $1
           ORDER BY created_at DESC, id DESC
           LIMIT 1"#,
    )
    .bind(created.id)
    .fetch_one(&pool)
    .await
    .expect("entry deactivation must append one atomic audit event");
    assert_eq!(
        deactivated_payload["actorUserId"],
        fixture.actor_id.to_string()
    );
    assert_eq!(deactivated_payload["oldRowVersion"], updated.row_version);
    assert_eq!(
        deactivated_payload["newRowVersion"],
        deactivated.row_version
    );
    assert_eq!(deactivated_payload["before"]["isActive"], true);
    assert_eq!(deactivated_payload["after"]["isActive"], false);
    assert_eq!(
        deactivated_payload["before"]["instructors"],
        deactivated_payload["after"]["instructors"]
    );
}

#[tokio::test]
async fn timetable_mutations_and_reads_are_isolated_by_version() {
    let pool = migrated_pool("timetable_version_isolation").await;
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let draft = clone_editable_version(&pool).await;
    let source_version_id = draft.source_version_id.unwrap();
    let (term_id, group_id, day, period_id): (Uuid, Uuid, String, Uuid) = sqlx::query_as(
        r#"SELECT entry.academic_term_id, entry.learning_group_id,
                  entry.day_of_week, entry.bell_schedule_period_id
           FROM academic_timetable_entries entry
           WHERE entry.timetable_version_id = $1
             AND entry.learning_group_id IS NOT NULL
             AND entry.is_active
           ORDER BY entry.id LIMIT 1"#,
    )
    .bind(draft.id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let draft_entry_id: Uuid = sqlx::query_scalar(
        r#"SELECT id FROM academic_timetable_entries
           WHERE timetable_version_id = $1
             AND learning_group_id = $2
             AND day_of_week = $3
             AND bell_schedule_period_id = $4
             AND is_active
           ORDER BY id LIMIT 1"#,
    )
    .bind(draft.id)
    .bind(group_id)
    .bind(&day)
    .bind(period_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE academic_timetable_entries SET is_active = false WHERE id = $1")
        .bind(draft_entry_id)
        .execute(&pool)
        .await
        .unwrap();

    let request = CreateTimetableEntryRequest {
        timetable_version_id: draft.id,
        academic_term_id: term_id,
        learning_group_id: Some(group_id),
        homeroom_id: None,
        day_of_week: day.clone(),
        bell_schedule_period_id: period_id,
        room_id: None,
        note: Some("version isolated".to_string()),
        entry_type: "course".to_string(),
        title: None,
        instructor_ids: Vec::new(),
    };
    let created = timetable_service::create_entry(&pool, actor_id, request.clone())
        .await
        .unwrap();
    assert_eq!(created.timetable_version_id, draft.id);

    let conflict = timetable_service::create_entry(&pool, actor_id, request).await;
    assert!(matches!(conflict, Err(AppError::Conflict(_))));

    let published_mutation = timetable_service::create_entry(
        &pool,
        actor_id,
        CreateTimetableEntryRequest {
            timetable_version_id: source_version_id,
            academic_term_id: term_id,
            learning_group_id: Some(group_id),
            homeroom_id: None,
            day_of_week: "SUN".to_string(),
            bell_schedule_period_id: period_id,
            room_id: None,
            note: None,
            entry_type: "course".to_string(),
            title: None,
            instructor_ids: Vec::new(),
        },
    )
    .await;
    assert!(matches!(published_mutation, Err(AppError::Conflict(_))));

    let listed = timetable_service::list_entries(
        &pool,
        &TimetableQuery {
            timetable_version_id: draft.id,
            academic_term_id: term_id,
            learning_group_id: Some(group_id),
            homeroom_id: None,
            instructor_id: None,
            room_id: None,
            day_of_week: None,
            entry_type: None,
        },
        &school_access(),
    )
    .await
    .unwrap();
    assert!(listed
        .iter()
        .all(|entry| entry.timetable_version_id == draft.id));
    let occupancy = timetable_service::occupancy(&pool, draft.id, term_id)
        .await
        .unwrap();
    assert!(occupancy.iter().any(|cell| cell.entry_id == created.id));
    assert!(!occupancy.iter().any(|cell| {
        listed.iter().all(|entry| entry.id != cell.entry_id) && cell.entry_id == draft_entry_id
    }));
}

fn school_access() -> AcademicResourceListFilter {
    AcademicResourceListFilter {
        includes_school_owned: true,
        ..Default::default()
    }
}

#[tokio::test]
async fn course_and_activity_groups_share_homeroom_conflict_detection() {
    let pool = migrated_pool("timetable_group_conflict").await;
    let draft = clone_editable_version(&pool).await;
    let (term_id, day, period_id, homeroom_id): (Uuid, String, Uuid, Uuid) = sqlx::query_as(
        r#"SELECT entry.academic_term_id, entry.day_of_week,
                  entry.bell_schedule_period_id,
                  coalesce(entry.homeroom_id, coverage.homeroom_id)
           FROM academic_timetable_entries entry
           LEFT JOIN learning_group_homerooms coverage
             ON coverage.learning_group_id = entry.learning_group_id
           WHERE entry.timetable_version_id = $1
             AND entry.entry_type = 'COURSE' AND entry.is_active
           ORDER BY entry.id, coverage.homeroom_id
           LIMIT 1"#,
    )
    .bind(draft.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let activity_group_id: Uuid = sqlx::query_scalar(
        r#"SELECT learning_group.id
           FROM learning_groups learning_group
           JOIN learning_offerings offering ON offering.id = learning_group.learning_offering_id
           JOIN learning_group_homerooms coverage ON coverage.learning_group_id = learning_group.id
           WHERE offering.kind = 'activity'
             AND learning_group.academic_term_id = $1
             AND coverage.homeroom_id = $2
           ORDER BY learning_group.id
           LIMIT 1"#,
    )
    .bind(term_id)
    .bind(homeroom_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    let result = timetable_service::create_entry(
        &pool,
        Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap(),
        CreateTimetableEntryRequest {
            timetable_version_id: draft.id,
            academic_term_id: term_id,
            learning_group_id: Some(activity_group_id),
            homeroom_id: None,
            day_of_week: day,
            bell_schedule_period_id: period_id,
            room_id: None,
            note: None,
            entry_type: "activity".to_string(),
            title: None,
            instructor_ids: Vec::new(),
        },
    )
    .await;
    assert!(
        matches!(result, Err(AppError::Conflict(message)) if message.contains("ห้องประจำชั้น") || message.contains("ครู"))
    );
}

#[tokio::test]
async fn student_timetable_uses_the_membership_interval_for_the_requested_date() {
    let pool = migrated_pool("timetable_student_membership_interval").await;
    let (version_id, academic_term_id, learning_group_id, student_id, membership_id, joined_at): (
        Uuid,
        Uuid,
        Uuid,
        Uuid,
        Uuid,
        NaiveDate,
    ) = sqlx::query_as(
        r#"SELECT version.id, entry.academic_term_id, entry.learning_group_id,
                  membership.student_id, membership.id, membership.joined_at
           FROM academic_timetable_versions version
           JOIN academic_timetable_entries entry
             ON entry.timetable_version_id = version.id
            AND entry.learning_group_id IS NOT NULL
            AND entry.is_active
           JOIN learning_group_students membership
             ON membership.learning_group_id = entry.learning_group_id
            AND membership.membership_status = 'active'
           JOIN learning_groups learning_group ON learning_group.id = entry.learning_group_id
           WHERE version.status = 'published'
             AND learning_group.roster_status IN ('published', 'closed')
           ORDER BY version.effective_from, entry.id, membership.id
           LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .expect("fixture must contain a scheduled group with a published membership");
    let left_at = joined_at
        .checked_add_signed(chrono::Duration::days(1))
        .unwrap();
    sqlx::query(
        "UPDATE learning_group_students \
         SET membership_status = 'ended', left_at = $1, row_version = row_version + 1 \
         WHERE id = $2",
    )
    .bind(left_at)
    .bind(membership_id)
    .execute(&pool)
    .await
    .unwrap();

    let on_inclusive_end = timetable_service::list_student_entries(
        &pool,
        version_id,
        academic_term_id,
        student_id,
        left_at,
    )
    .await
    .unwrap();
    assert!(on_inclusive_end
        .iter()
        .any(|entry| entry.learning_group_id == Some(learning_group_id)));

    let after_end = timetable_service::list_student_entries(
        &pool,
        version_id,
        academic_term_id,
        student_id,
        left_at
            .checked_add_signed(chrono::Duration::days(1))
            .unwrap(),
    )
    .await
    .unwrap();
    assert!(!after_end
        .iter()
        .any(|entry| entry.learning_group_id == Some(learning_group_id)));
}

#[tokio::test]
async fn timetable_entry_accepts_a_room_with_active_status() {
    let pool = migrated_pool("timetable_active_room_status").await;
    let draft = clone_editable_version(&pool).await;
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let (entry_id, row_version): (Uuid, i64) = sqlx::query_as(
        r#"SELECT id, row_version
           FROM academic_timetable_entries
           WHERE timetable_version_id = $1 AND is_active
           ORDER BY id LIMIT 1"#,
    )
    .bind(draft.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let room_id: Uuid =
        sqlx::query_scalar("SELECT id FROM rooms WHERE status = 'ACTIVE' ORDER BY id LIMIT 1")
            .fetch_one(&pool)
            .await
            .expect("fixture must contain an active room");

    let updated = timetable_service::update_entry(
        &pool,
        entry_id,
        actor_id,
        UpdateTimetableEntryRequest {
            timetable_version_id: draft.id,
            row_version,
            day_of_week: None,
            bell_schedule_period_id: None,
            room_id: Some(room_id),
            clear_room: None,
            note: None,
            clear_note: None,
            title: None,
            instructor_ids: None,
        },
    )
    .await
    .expect("a room with ACTIVE status must be accepted by timetable mutations");
    assert_eq!(updated.room_id, Some(room_id));
}

#[tokio::test]
async fn listing_occupancy_and_swaps_are_explicitly_term_and_group_scoped() {
    let pool = migrated_pool("timetable_term_group_swap").await;
    let draft = clone_editable_version(&pool).await;
    let actor_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let (entry_id, term_id, group_id, first_day, first_period_id, row_version): (
        Uuid,
        Uuid,
        Uuid,
        String,
        Uuid,
        i64,
    ) = sqlx::query_as(
        r#"SELECT id, academic_term_id, learning_group_id, day_of_week,
                  bell_schedule_period_id, row_version
           FROM academic_timetable_entries
           WHERE timetable_version_id = $1
             AND learning_group_id IS NOT NULL AND entry_type = 'COURSE'
             AND EXISTS (
                 SELECT 1 FROM timetable_entry_instructors instructor
                 WHERE instructor.entry_id = academic_timetable_entries.id
             )
           ORDER BY id LIMIT 1"#,
    )
    .bind(draft.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let original_entry = timetable_service::get_entry(&pool, entry_id).await.unwrap();
    assert!(!original_entry.instructors.is_empty());
    let second_period_id: Uuid = sqlx::query_scalar(
        r#"SELECT period.id
           FROM academic_terms term
           JOIN bell_schedule_periods period ON period.bell_schedule_id = term.bell_schedule_id
           WHERE term.id = $1 AND period.id <> $2 AND period.is_active
           ORDER BY period.order_index, period.id LIMIT 1"#,
    )
    .bind(term_id)
    .bind(first_period_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let created = timetable_service::create_entry(
        &pool,
        actor_id,
        CreateTimetableEntryRequest {
            timetable_version_id: draft.id,
            academic_term_id: term_id,
            learning_group_id: Some(group_id),
            homeroom_id: None,
            day_of_week: "TUE".to_string(),
            bell_schedule_period_id: second_period_id,
            room_id: None,
            note: Some("canonical".to_string()),
            entry_type: "course".to_string(),
            title: None,
            instructor_ids: instructor_ids(&original_entry),
        },
    )
    .await
    .unwrap();

    let listed = timetable_service::list_entries(
        &pool,
        &TimetableQuery {
            timetable_version_id: draft.id,
            academic_term_id: term_id,
            learning_group_id: Some(group_id),
            homeroom_id: None,
            instructor_id: None,
            room_id: None,
            day_of_week: None,
            entry_type: None,
        },
        &school_access(),
    )
    .await
    .unwrap();
    assert!(listed.iter().all(|entry| {
        entry.academic_term_id == term_id && entry.learning_group_id == Some(group_id)
    }));
    let listed_created = listed
        .iter()
        .find(|entry| entry.id == created.id)
        .expect("batch-hydrated list must retain the created entry");
    assert_eq!(
        listed_created
            .instructors
            .iter()
            .map(|instructor| instructor.user_id)
            .collect::<Vec<_>>(),
        created
            .instructors
            .iter()
            .map(|instructor| instructor.user_id)
            .collect::<Vec<_>>()
    );

    let occupancy = timetable_service::occupancy(&pool, draft.id, term_id)
        .await
        .unwrap();
    let created_cell = occupancy
        .iter()
        .find(|cell| cell.entry_id == created.id)
        .unwrap();
    assert!(!created_cell.homeroom_ids.is_empty());

    let swapped = timetable_service::swap_entries(
        &pool,
        actor_id,
        SwapTimetableEntriesRequest {
            timetable_version_id: draft.id,
            entry_a_id: entry_id,
            entry_a_row_version: row_version,
            entry_b_id: created.id,
            entry_b_row_version: created.row_version,
        },
    )
    .await
    .unwrap();
    assert_eq!(swapped.entry_a.academic_term_id, term_id);
    assert_eq!(swapped.entry_b.academic_term_id, term_id);
    assert!(swapped.entry_a.row_version > row_version);
    assert!(swapped.entry_b.row_version > created.row_version);

    let swapped_ids = vec![entry_id, created.id];
    let swap_audits: Vec<(Uuid, serde_json::Value)> = sqlx::query_as(
        r#"SELECT entity_id, payload
           FROM academic_audit_events
           WHERE event_code = 'academic_timetable_entry.updated'
             AND entity_id = ANY($1)
           ORDER BY entity_id, created_at DESC, id DESC"#,
    )
    .bind(&swapped_ids)
    .fetch_all(&pool)
    .await
    .unwrap();
    let entry_a_audit = swap_audits
        .iter()
        .find(|(audit_entry_id, payload)| {
            *audit_entry_id == entry_id && payload["newRowVersion"] == swapped.entry_a.row_version
        })
        .map(|(_, payload)| payload)
        .expect("swap must audit the first entry atomically");
    let entry_b_audit = swap_audits
        .iter()
        .find(|(audit_entry_id, payload)| {
            *audit_entry_id == created.id && payload["newRowVersion"] == swapped.entry_b.row_version
        })
        .map(|(_, payload)| payload)
        .expect("swap must audit the second entry atomically");
    let expected_entry_a_instructors = instructor_roles_json(&original_entry);
    let expected_entry_b_instructors = instructor_roles_json(&created);
    assert_ne!(expected_entry_a_instructors, serde_json::json!([]));
    assert_ne!(expected_entry_b_instructors, serde_json::json!([]));
    for (payload, expected_instructors) in [
        (entry_a_audit, &expected_entry_a_instructors),
        (entry_b_audit, &expected_entry_b_instructors),
    ] {
        assert_eq!(payload["actorUserId"], actor_id.to_string());
        assert_eq!(payload["timetableVersionId"], draft.id.to_string());
        assert_eq!(payload["academicTermId"], term_id.to_string());
        assert_eq!(payload["before"]["instructors"], *expected_instructors);
        assert_eq!(payload["after"]["instructors"], *expected_instructors);
    }
    assert_eq!(entry_a_audit["oldRowVersion"], row_version);
    assert_eq!(entry_a_audit["before"]["dayOfWeek"], first_day);
    assert_eq!(
        entry_a_audit["before"]["bellSchedulePeriodId"],
        first_period_id.to_string()
    );
    assert_eq!(entry_a_audit["after"]["dayOfWeek"], "TUE");
    assert_eq!(
        entry_a_audit["after"]["bellSchedulePeriodId"],
        second_period_id.to_string()
    );
    assert_eq!(entry_b_audit["oldRowVersion"], created.row_version);
    assert_eq!(entry_b_audit["before"]["dayOfWeek"], "TUE");
    assert_eq!(
        entry_b_audit["after"]["bellSchedulePeriodId"],
        first_period_id.to_string()
    );
}

#[tokio::test]
async fn batch_and_conflict_reads_preserve_results() {
    let pool = migrated_pool("timetable_batch_conflict_bulk_reads").await;
    let fixture = exact_instructor_fixture(&pool).await;
    let draft = fixture.draft;
    let actor_id = fixture.actor_id;
    let term_id = fixture.term_id;
    let group_id = fixture.group_id;
    let period_id = fixture.period_id;
    let teacher_a = fixture.teacher_a;
    let teacher_b = fixture.teacher_b;

    let created = timetable_service::create_batch(
        &pool,
        actor_id,
        CreateBatchTimetableEntriesRequest {
            timetable_version_id: draft.id,
            academic_term_id: term_id,
            learning_group_ids: vec![group_id],
            homeroom_ids: Vec::new(),
            days_of_week: vec!["TUE".to_string(), "WED".to_string()],
            bell_schedule_period_ids: vec![period_id],
            entry_type: "course".to_string(),
            title: Some("bulk".to_string()),
            room_id: None,
            note: None,
            instructor_ids: vec![teacher_a, teacher_b],
        },
    )
    .await
    .unwrap();
    assert_eq!(created.entries.len(), 2);
    assert!(created
        .entries
        .iter()
        .all(|entry| entry.learning_group_id == Some(group_id)));
    let expected_instructors = instructor_roles_json(&created.entries[0]);
    assert_ne!(expected_instructors, serde_json::json!([]));
    assert!(created
        .entries
        .iter()
        .all(|entry| instructor_roles_json(entry) == expected_instructors));

    let moves = timetable_service::validate_moves(&pool, draft.id, term_id, created.entries[0].id)
        .await
        .unwrap();
    assert_eq!(
        moves
            .iter()
            .filter(|cell| cell.state == TimetablePlacementState::Source)
            .count(),
        1
    );
    assert!(moves.len() >= 14);

    let deactivated =
        timetable_service::deactivate_batch(&pool, created.batch_id, draft.id, actor_id)
            .await
            .unwrap();
    let mut expected_deactivated_ids = created
        .entries
        .iter()
        .map(|entry| entry.id)
        .collect::<Vec<_>>();
    expected_deactivated_ids.sort_unstable();
    assert_eq!(
        deactivated.iter().map(|entry| entry.id).collect::<Vec<_>>(),
        expected_deactivated_ids
    );
    assert!(deactivated.iter().all(|entry| !entry.is_active));

    let audit_rows: Vec<(String, Uuid, Uuid, serde_json::Value)> = sqlx::query_as(
        r#"SELECT event_code, entity_id, actor_user_id, payload
           FROM academic_audit_events
           WHERE entity_id = ANY($1)
             AND event_code IN (
                 'academic_timetable_entry.created',
                 'academic_timetable_entry.deactivated'
             )
           ORDER BY event_code, entity_id"#,
    )
    .bind(&expected_deactivated_ids)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(audit_rows.len(), 4);
    assert_eq!(
        audit_rows
            .iter()
            .filter(|(event_code, _, _, _)| { event_code == "academic_timetable_entry.created" })
            .count(),
        2
    );
    assert_eq!(
        audit_rows
            .iter()
            .filter(|(event_code, _, _, _)| {
                event_code == "academic_timetable_entry.deactivated"
            })
            .count(),
        2
    );
    for (event_code, entity_id, audit_actor_id, payload) in audit_rows {
        assert_eq!(audit_actor_id, actor_id);
        assert_eq!(payload["entryId"], entity_id.to_string());
        assert_eq!(payload["timetableVersionId"], draft.id.to_string());
        assert_eq!(payload["academicTermId"], term_id.to_string());
        assert_eq!(payload["actorUserId"], actor_id.to_string());
        if event_code == "academic_timetable_entry.created" {
            assert_eq!(payload["before"]["instructors"], serde_json::json!([]));
            assert_eq!(payload["after"]["instructors"], expected_instructors);
            assert_eq!(payload["oldRowVersion"], 0);
            assert_eq!(payload["newRowVersion"], 1);
        } else {
            assert_eq!(payload["before"]["instructors"], expected_instructors);
            assert_eq!(payload["after"]["instructors"], expected_instructors);
            assert_eq!(payload["oldRowVersion"], 1);
            assert_eq!(payload["newRowVersion"], 2);
        }
    }
}

#[tokio::test]
async fn daily_teaching_uses_group_and_offering_snapshots_in_requested_term() {
    let pool = migrated_pool("daily_teaching_group_snapshot").await;
    let term_id: Uuid = sqlx::query_scalar(
        "SELECT academic_term_id FROM academic_timetable_entries ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let overview = daily_teaching_service::get_daily_teaching_overview(
        &pool,
        daily_teaching_service::DailyTeachingQuery {
            academic_term_id: term_id,
            date: Some(NaiveDate::from_ymd_opt(2026, 8, 24).unwrap()),
            include_empty_teachers: Some(false),
        },
    )
    .await
    .unwrap();
    assert_eq!(overview.academic_term_id, term_id);
    assert_eq!(overview.day_of_week, "MON");
    assert!(overview
        .teachers
        .iter()
        .flat_map(|teacher| &teacher.periods)
        .flat_map(|period| &period.entries)
        .any(|entry| {
            entry.learning_group_id.is_some()
                && entry.offering_id.is_some()
                && (entry.subject_id.is_some() || entry.activity_id.is_some())
        }));
}

#[test]
fn timetable_request_rejects_legacy_identity_fields() {
    let payload = serde_json::json!({
        "timetableVersionId": Uuid::new_v4(),
        "academicTermId": Uuid::new_v4(),
        "classroomCourseId": Uuid::new_v4(),
        "dayOfWeek": "MON",
        "bellSchedulePeriodId": Uuid::new_v4(),
        "entryType": "course"
    });
    assert!(serde_json::from_value::<CreateTimetableEntryRequest>(payload).is_err());
}
