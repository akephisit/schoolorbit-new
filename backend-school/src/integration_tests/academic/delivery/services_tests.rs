use super::{
    models::*,
    services::{activities, current, groups, offerings, roster_memberships, workspaces},
};
use crate::integration_tests::academic::{
    cutover_test_preflight::run_academic_core_preflight,
    cutover_test_support::{
        CutoverFixture, apply_migrations_through, apply_phase_b_runtime_migrations,
        seed_academic_cutover_fixture,
    },
};
use chrono::{Duration, NaiveDate, Utc};
use school_authorization::AcademicResourceListFilter;
use school_errors::AppError;
const ACADEMIC_CORE_NAMESPACE: Uuid = Uuid::from_u128(0x5c33_b984_10df_58db_bf80_62db_c4a0_3d1b);
use school_test_db::{create_named_test_pool, create_named_test_pool_with_max_connections};
use sqlx::{PgPool, Row};
use uuid::Uuid;

fn stable_uuid(name: &str) -> Uuid {
    Uuid::new_v5(&ACADEMIC_CORE_NAMESPACE, name.as_bytes())
}

async fn add_unmapped_independent_homeroom(pool: &PgPool) {
    sqlx::raw_sql(
        r#"
        UPDATE activity_groups
        SET allowed_classroom_ids = '["40000000-0000-0000-0000-000000000025"]'::jsonb
        WHERE id = '73000000-0000-0000-0000-000000000002';

        INSERT INTO users (
            id, email, username, password_hash, first_name, last_name, user_type, status
        )
        VALUES (
            '50000000-0000-0000-0000-000000000004',
            'fixture-slot-teacher@example.invalid', 'fixture-slot-teacher',
            'fixture-not-a-login', 'ครูกิจกรรม', 'ทดสอบ', 'staff', 'active'
        );

        INSERT INTO activity_slot_instructors (id, slot_id, user_id)
        VALUES (
            '72500000-0000-0000-0000-000000000004',
            '70000000-0000-0000-0000-000000000001',
            '50000000-0000-0000-0000-000000000004'
        );

        INSERT INTO activity_slot_classrooms (id, slot_id, classroom_id)
        VALUES (
            '71000000-0000-0000-0000-000000000003',
            '70000000-0000-0000-0000-000000000002',
            '40000000-0000-0000-0000-000000000125'
        );

        INSERT INTO activity_slot_classroom_assignments (
            id, slot_id, classroom_id, instructor_id
        )
        VALUES (
            '72000000-0000-0000-0000-000000000003',
            '70000000-0000-0000-0000-000000000002',
            '40000000-0000-0000-0000-000000000125',
            '50000000-0000-0000-0000-000000000002'
        );
        "#,
    )
    .execute(pool)
    .await
    .expect("independent activity fixture must extend the legacy schema");
}

async fn prepare_delivery_fixture(
    name: &str,
    include_unmapped_independent_homeroom: bool,
) -> PgPool {
    let pool = create_named_test_pool(name).await;
    apply_migrations_through(&pool, 40)
        .await
        .expect("legacy migrations must apply");
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .expect("passing cutover fixture must seed");

    if include_unmapped_independent_homeroom {
        add_unmapped_independent_homeroom(&pool).await;
    }

    apply_migrations_through(&pool, 41)
        .await
        .expect("migration 041 must prepare the core schema");

    pool
}

#[tokio::test]
async fn preflight_counts_generated_independent_activity_groups() {
    let pool = create_named_test_pool("academic_delivery_preflight_generated_group").await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    add_unmapped_independent_homeroom(&pool).await;

    let report = run_academic_core_preflight(
        &pool,
        "schoolorbit_test_academic_delivery_preflight_generated_group",
        NaiveDate::from_ymd_opt(2025, 8, 23).unwrap(),
    )
    .await
    .expect("delivery preflight must run");

    assert!(report.can_cut_over);
    assert_eq!(report.expected_target_counts["learningGroups"], 6);
}

#[tokio::test]
async fn migration_042_maps_delivery_fixture() {
    let pool = prepare_delivery_fixture("academic_delivery_042", true).await;

    apply_migrations_through(&pool, 42)
        .await
        .expect("migration 042 must map the passing delivery fixture");

    let target_relations: Vec<String> = sqlx::query_scalar(
        r#"SELECT relname::text
           FROM pg_class
           WHERE relnamespace = current_schema()::regnamespace
             AND relname = ANY($1)
           ORDER BY relname"#,
    )
    .bind(vec![
        "activity_offering_details",
        "activity_result_details",
        "academic_core_entity_map",
        "course_offering_details",
        "homeroom_placements",
        "homerooms",
        "learning_group_homerooms",
        "learning_group_students",
        "learning_group_teachers",
        "learning_groups",
        "learning_offering_targets",
        "learning_offerings",
        "learning_results",
        "student_academic_years",
    ])
    .fetch_all(&pool)
    .await
    .expect("delivery relations must be queryable");
    assert_eq!(target_relations.len(), 14);

    let homeroom_program: Uuid = sqlx::query_scalar(
        r#"SELECT study_program_id
           FROM homerooms
           WHERE id = '40000000-0000-0000-0000-000000000025'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("homeroom must resolve its exact default program");
    assert_eq!(
        homeroom_program,
        stable_uuid("program:31000000-0000-0000-0000-000000000025")
    );

    let student_years: Vec<(Uuid, i32, String)> = sqlx::query_as(
        r#"SELECT student_year.id, year.year, student_year.status
           FROM student_academic_years student_year
           JOIN academic_years year ON year.id = student_year.academic_year_id
           ORDER BY year.year"#,
    )
    .fetch_all(&pool)
    .await
    .expect("student-year rows must be queryable");
    assert_eq!(
        student_years,
        vec![
            (
                stable_uuid(
                    "student-year:50000000-0000-0000-0000-000000000001:10000000-0000-0000-0000-000000000024",
                ),
                2024,
                "completed".to_string(),
            ),
            (
                stable_uuid(
                    "student-year:50000000-0000-0000-0000-000000000001:10000000-0000-0000-0000-000000000025",
                ),
                2025,
                "active".to_string(),
            ),
            (
                stable_uuid(
                    "student-year:50000000-0000-0000-0000-000000000001:10000000-0000-0000-0000-000000000026",
                ),
                2026,
                "planned".to_string(),
            ),
        ]
    );

    let placements: Vec<(Uuid, String, Option<chrono::NaiveDate>)> = sqlx::query_as(
        r#"SELECT id, status, end_date
           FROM homeroom_placements
           ORDER BY id"#,
    )
    .fetch_all(&pool)
    .await
    .expect("placement history must be queryable");
    assert_eq!(placements.len(), 3);
    assert_eq!(
        placements.iter().map(|row| row.0).collect::<Vec<_>>(),
        vec![
            Uuid::parse_str("51000000-0000-0000-0000-000000000024").unwrap(),
            Uuid::parse_str("51000000-0000-0000-0000-000000000025").unwrap(),
            Uuid::parse_str("51000000-0000-0000-0000-000000000026").unwrap(),
        ]
    );
    assert_eq!(placements[0].1, "ended");
    assert!(placements[0].2.is_some());
    assert_eq!(placements[1].1, "current");
    assert_eq!(placements[1].2, None);
    assert_eq!(placements[2].1, "current");
    assert_eq!(placements[2].2, None);

    let offering_counts: Vec<(String, i64)> = sqlx::query_as(
        r#"SELECT kind, COUNT(*)::bigint
           FROM learning_offerings
           GROUP BY kind
           ORDER BY kind"#,
    )
    .fetch_all(&pool)
    .await
    .expect("offerings must be queryable");
    assert_eq!(
        offering_counts,
        vec![("activity".to_string(), 2), ("course".to_string(), 2)]
    );

    let course_offering_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM learning_offerings WHERE kind = 'course' ORDER BY id")
            .fetch_all(&pool)
            .await
            .expect("course offerings must be queryable");
    let mut expected_course_offering_ids = vec![
        stable_uuid(
            "course-offering:11000000-0000-0000-0000-000000000241:20000000-0000-0000-0000-000000000024",
        ),
        stable_uuid(
            "course-offering:11000000-0000-0000-0000-000000000251:20000000-0000-0000-0000-000000000025",
        ),
    ];
    expected_course_offering_ids.sort();
    assert_eq!(course_offering_ids, expected_course_offering_ids);

    let activity_offering_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM learning_offerings WHERE kind = 'activity' ORDER BY id")
            .fetch_all(&pool)
            .await
            .expect("activity offerings must preserve slot IDs");
    assert_eq!(
        activity_offering_ids,
        vec![
            Uuid::parse_str("70000000-0000-0000-0000-000000000001").unwrap(),
            Uuid::parse_str("70000000-0000-0000-0000-000000000002").unwrap(),
        ]
    );

    let generated_group_id = stable_uuid(
        "activity-group:70000000-0000-0000-0000-000000000002:40000000-0000-0000-0000-000000000125",
    );
    let group_ids: Vec<Uuid> = sqlx::query_scalar("SELECT id FROM learning_groups ORDER BY id")
        .fetch_all(&pool)
        .await
        .expect("learning groups must be queryable");
    assert_eq!(group_ids.len(), 6);
    assert!(group_ids.contains(&generated_group_id));
    assert!(group_ids.contains(&Uuid::parse_str("60000000-0000-0000-0000-000000000025").unwrap()));
    assert!(group_ids.contains(&Uuid::parse_str("73000000-0000-0000-0000-000000000001").unwrap()));

    let synchronized_slot_teacher: (Uuid, String) = sqlx::query_as(
        r#"SELECT teacher_id, role
           FROM learning_group_teachers
           WHERE learning_group_id = '73000000-0000-0000-0000-000000000001'
             AND teacher_id = '50000000-0000-0000-0000-000000000004'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("a synchronized slot teacher must be assigned to every source group");
    assert_eq!(
        synchronized_slot_teacher,
        (
            Uuid::parse_str("50000000-0000-0000-0000-000000000004").unwrap(),
            "assistant".to_string(),
        )
    );

    let generated_group_homeroom: Uuid = sqlx::query_scalar(
        r#"SELECT homeroom_id
           FROM learning_group_homerooms
           WHERE learning_group_id = $1"#,
    )
    .bind(generated_group_id)
    .fetch_one(&pool)
    .await
    .expect("generated independent group must cover its source homeroom");
    assert_eq!(
        generated_group_homeroom,
        Uuid::parse_str("40000000-0000-0000-0000-000000000125").unwrap()
    );

    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM learning_group_students")
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
    );
    let activity_roster: (Uuid, String) = sqlx::query_as(
        r#"SELECT id, roster_source
           FROM learning_group_students
           WHERE learning_group_id = '73000000-0000-0000-0000-000000000001'
             AND student_id = '50000000-0000-0000-0000-000000000001'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("legacy activity member must become an authoritative roster row");
    assert_eq!(
        activity_roster,
        (
            Uuid::parse_str("74000000-0000-0000-0000-000000000001").unwrap(),
            "legacy_activity_member".to_string(),
        )
    );

    let activity_result: (String, String) = sqlx::query_as(
        r#"SELECT result.status, detail.outcome
           FROM learning_results result
           JOIN activity_result_details detail ON detail.learning_result_id = result.id"#,
    )
    .fetch_one(&pool)
    .await
    .expect("legacy activity outcome must be preserved");
    assert_eq!(
        activity_result,
        ("recorded".to_string(), "pass".to_string())
    );

    let mapped_placements: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM academic_core_entity_map
           WHERE source_table = 'student_class_enrollments'
             AND target_table = 'homeroom_placements'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("placement mappings must be queryable");
    assert_eq!(mapped_placements, 3);

    let mapped_slot_teacher: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM academic_core_entity_map
           WHERE source_table = 'activity_slot_instructors'
             AND source_id = '72500000-0000-0000-0000-000000000004'
             AND target_table = 'learning_group_teachers'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("slot teacher mappings must be queryable");
    assert_eq!(mapped_slot_teacher, 1);

    let audit = sqlx::query(
        r#"SELECT source_counts, target_counts
           FROM academic_core_cutover_audits
           WHERE migration_version = 42"#,
    )
    .fetch_one(&pool)
    .await
    .expect("migration 042 must write aggregate reconciliation evidence");
    let source_counts: serde_json::Value = audit.get("source_counts");
    let target_counts: serde_json::Value = audit.get("target_counts");
    assert_eq!(source_counts["enrollments"], 3);
    assert_eq!(target_counts["studentAcademicYears"], 3);
    assert_eq!(target_counts["learningGroups"], 6);
    assert_eq!(target_counts["groupStudents"], 3);
}

#[tokio::test]
async fn migration_042_enforces_delivery_context_and_subtype_invariants() {
    let pool = prepare_delivery_fixture("academic_delivery_042_invariants", false).await;
    apply_migrations_through(&pool, 42).await.unwrap();

    let duplicate_placement = sqlx::query(
        r#"INSERT INTO homeroom_placements (
               id, student_academic_year_id, academic_year_id, homeroom_id,
               start_date, status, enrollment_type, migration_provenance
           )
           SELECT '51000000-0000-0000-0000-000000000125', student_academic_year_id,
                  academic_year_id, homeroom_id, start_date, 'current', enrollment_type,
                  '{"fixture":"duplicate-current"}'::jsonb
           FROM homeroom_placements
           WHERE id = '51000000-0000-0000-0000-000000000025'"#,
    )
    .execute(&pool)
    .await
    .expect_err("a student-year cannot have two current placements");
    assert!(
        duplicate_placement
            .to_string()
            .contains("homeroom_placements_one_current_key")
    );

    let duplicate_student = sqlx::query(
        r#"INSERT INTO learning_group_students (
               id, learning_group_id, academic_term_id, academic_year_id,
               student_academic_year_id, student_id, membership_status,
               roster_source, joined_at, migration_provenance
           )
           SELECT '74000000-0000-0000-0000-000000000125', learning_group_id,
                  academic_term_id, academic_year_id, student_academic_year_id,
                  student_id, 'active', 'fixture_duplicate', joined_at,
                  '{"fixture":"duplicate-student"}'::jsonb
           FROM learning_group_students
           WHERE id = '74000000-0000-0000-0000-000000000001'"#,
    )
    .execute(&pool)
    .await
    .expect_err("a student cannot be active twice in one group");
    assert!(
        duplicate_student
            .to_string()
            .contains("learning_group_students_one_active_key")
    );

    let cross_year_homeroom = sqlx::query(
        r#"INSERT INTO learning_group_homerooms (
               id, learning_group_id, academic_term_id, academic_year_id,
               homeroom_id, coverage_source, migration_provenance
           )
           SELECT '75000000-0000-0000-0000-000000000125', learning_group_id,
                  academic_term_id, academic_year_id,
                  '40000000-0000-0000-0000-000000000024', 'fixture_cross_year',
                  '{"fixture":"cross-year"}'::jsonb
           FROM learning_group_homerooms
           WHERE learning_group_id = '60000000-0000-0000-0000-000000000025'"#,
    )
    .execute(&pool)
    .await
    .expect_err("group coverage cannot use a homeroom from another year");
    assert!(
        cross_year_homeroom
            .to_string()
            .contains("learning_group_homerooms_homeroom_context_fkey")
    );

    let offering_id = stable_uuid(
        "course-offering:11000000-0000-0000-0000-000000000251:20000000-0000-0000-0000-000000000025",
    );
    let mismatched_group = sqlx::query(
        r#"INSERT INTO learning_groups (
               id, learning_offering_id, academic_term_id, academic_year_id,
               code, name, status, roster_status, migration_provenance
           )
           VALUES (
               '76000000-0000-0000-0000-000000000125', $1,
               '11000000-0000-0000-0000-000000000241',
               '10000000-0000-0000-0000-000000000024',
               'MISMATCH', 'กลุ่มต่างภาคเรียน', 'published', 'published',
               '{"fixture":"term-mismatch"}'::jsonb
           )"#,
    )
    .bind(offering_id)
    .execute(&pool)
    .await
    .expect_err("a group cannot cross its offering term");
    assert!(
        mismatched_group
            .to_string()
            .contains("learning_groups_offering_context_fkey")
    );

    let mut transaction = pool.begin().await.unwrap();
    sqlx::query("UPDATE learning_offerings SET status = 'draft' WHERE id = $1")
        .bind(offering_id)
        .execute(&mut *transaction)
        .await
        .expect("the subtype test must operate on a draft offering");
    sqlx::query(
        r#"INSERT INTO activity_offering_details (
               learning_offering_id, academic_term_id, academic_year_id,
               activity_version_id, activity_id, registration_type,
               scheduling_mode, hours, migration_provenance
           )
           SELECT $1, academic_term_id, academic_year_id, activity_version_id,
                  activity_id, registration_type, scheduling_mode, hours,
                  '{"fixture":"wrong-subtype"}'::jsonb
           FROM activity_offering_details
           LIMIT 1"#,
    )
    .bind(offering_id)
    .execute(&mut *transaction)
    .await
    .expect("deferred subtype constraint permits the statement until checked");
    let subtype_error = sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
        .execute(&mut *transaction)
        .await
        .expect_err("a course offering cannot carry activity details");
    assert!(
        subtype_error
            .to_string()
            .contains("ACADEMIC_CORE_OFFERING_SUBTYPE_MISMATCH"),
        "unexpected subtype constraint error: {subtype_error}"
    );
    transaction.rollback().await.unwrap();

    let mut transaction = pool.begin().await.unwrap();
    sqlx::query("UPDATE learning_offerings SET status = 'draft' WHERE id = $1")
        .bind(offering_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
    sqlx::query("DELETE FROM course_offering_details WHERE learning_offering_id = $1")
        .bind(offering_id)
        .execute(&mut *transaction)
        .await
        .expect("deferred subtype constraint permits the delete until checked");
    let missing_subtype_error = sqlx::query("SET CONSTRAINTS ALL IMMEDIATE")
        .execute(&mut *transaction)
        .await
        .expect_err("an offering cannot remain without its matching subtype");
    assert!(
        missing_subtype_error
            .to_string()
            .contains("ACADEMIC_CORE_OFFERING_SUBTYPE_MISMATCH"),
        "unexpected missing subtype constraint error: {missing_subtype_error}"
    );
    transaction.rollback().await.unwrap();

    let published_snapshot_error = sqlx::query(
        r#"UPDATE learning_offerings
           SET name_snapshot = 'ชื่อที่ไม่ควรแก้ได้'
           WHERE id = $1"#,
    )
    .bind(offering_id)
    .execute(&pool)
    .await
    .expect_err("published offering snapshots must be immutable");
    assert!(
        published_snapshot_error
            .to_string()
            .contains("ACADEMIC_CORE_PUBLISHED_OFFERING_IMMUTABLE")
    );
}

async fn prepare_delivery_runtime_fixture(name: &str) -> PgPool {
    prepare_delivery_runtime_fixture_through(name, school_migrations::latest_migration_version())
        .await
}

async fn prepare_delivery_runtime_fixture_through(name: &str, version: i64) -> PgPool {
    let pool = create_named_test_pool(name).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, version).await.unwrap();
    pool
}

async fn prepare_concurrent_delivery_runtime_fixture(name: &str) -> PgPool {
    let pool = create_named_test_pool_with_max_connections(name, 5).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, school_migrations::latest_migration_version())
        .await
        .unwrap();
    pool
}

#[tokio::test]
async fn lifecycle_delivery_closure_checks_the_year_and_keeps_closing_editable() {
    let pool = prepare_delivery_runtime_fixture("delivery_lifecycle_states").await;
    let context = planning_runtime_context(&pool).await;
    sqlx::query(
        "UPDATE academic_years SET status='closed' WHERE id<>$1 AND status IN ('active','closing')",
    )
    .bind(context.year_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "UPDATE academic_terms SET status='closed' WHERE id<>$1 AND status IN ('active','closing')",
    )
    .bind(context.term_id)
    .execute(&pool)
    .await
    .unwrap();
    for (year_status, term_status, writable) in [
        ("closing", "closing", true),
        ("closed", "planning", false),
        ("archived", "planning", false),
        ("planning", "closed", false),
        ("planning", "cancelled", false),
    ] {
        sqlx::query("UPDATE academic_years SET status=$2 WHERE id=$1")
            .bind(context.year_id)
            .bind(year_status)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE academic_terms SET status=$2 WHERE id=$1")
            .bind(context.term_id)
            .bind(term_status)
            .execute(&pool)
            .await
            .unwrap();
        for exclusive in [false, true] {
            let mut tx = pool.begin().await.unwrap();
            let result = school_academic_delivery::services::ensure_writable_term(
                &mut tx,
                context.term_id,
                exclusive,
            )
            .await;
            assert_eq!(
                result.is_ok(),
                writable,
                "{year_status}/{term_status}, exclusive={exclusive}: {result:?}"
            );
            if !writable {
                assert!(matches!(result, Err(AppError::Conflict(_))));
            }
            tx.rollback().await.unwrap();
        }
    }
}

#[tokio::test]
async fn lifecycle_delivery_waits_before_locking_offering_or_group() {
    let pool = prepare_concurrent_delivery_runtime_fixture("delivery_lifecycle_order").await;
    let context = planning_runtime_context(&pool).await;
    let offering = offerings::create(&pool, context.teacher_id, course_request(&context))
        .await
        .unwrap();
    let group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            registration_open: false,
            name: "Lock ordering".into(),
            description: None,
            capacity: Some(40),
            preferred_room_ids: vec![],
        },
    )
    .await
    .unwrap();
    assert!(group.code.starts_with("GROUP-"));
    for operation in 0..5 {
        let offering = offerings::get(&pool, offering.id).await.unwrap();
        let group = groups::get(&pool, group.id).await.unwrap();
        let mut boundary = pool.begin().await.unwrap();
        let boundary_pid: i32 = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut *boundary)
            .await
            .unwrap();
        sqlx::query("SELECT id FROM academic_terms WHERE id=$1 FOR UPDATE")
            .bind(context.term_id)
            .execute(&mut *boundary)
            .await
            .unwrap();
        let worker_pool = pool.clone();
        let actor = context.teacher_id;
        let targets = course_request(&context).targets().to_vec();
        let worker = tokio::spawn(async move {
            match operation {
                0 => offerings::update(
                    &worker_pool,
                    actor,
                    offering.id,
                    UpdateLearningOfferingRequest {
                        weekly_period_target: 5,
                        row_version: offering.row_version,
                        targets,
                    },
                )
                .await
                .map(|_| ()),
                1 => groups::create(
                    &worker_pool,
                    actor,
                    offering.id,
                    CreateLearningGroupRequest {
                        registration_open: false,
                        name: "Second group".into(),
                        description: None,
                        capacity: Some(40),
                        preferred_room_ids: vec![],
                    },
                )
                .await
                .map(|_| ()),
                2 => groups::update(
                    &worker_pool,
                    actor,
                    group.id,
                    super::models::UpdateLearningGroupRequest {
                        registration_open: false,
                        row_version: group.row_version,
                        name: group.name,
                        description: None,
                        capacity: Some(40),
                        preferred_room_ids: vec![],
                    },
                )
                .await
                .map(|_| ()),
                3 => groups::replace_homerooms(
                    &worker_pool,
                    actor,
                    group.id,
                    ReplaceLearningGroupHomeroomsRequest {
                        row_version: group.row_version,
                        homeroom_ids: vec![],
                    },
                )
                .await
                .map(|_| ()),
                _ => offerings::update_weekly_period_target(
                    &worker_pool,
                    offering.id,
                    school_academic_delivery::models::current::UpdateWeeklyPeriodTargetRequest {
                        row_version: offering.row_version,
                        weekly_period_target: 1,
                    },
                )
                .await
                .map(|_| ()),
            }
        });
        let mut waiting = false;
        for _ in 0..200 {
            waiting = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM pg_stat_activity WHERE $1 = ANY(pg_blocking_pids(pid)))")
                .bind(boundary_pid).fetch_one(&pool).await.unwrap();
            if waiting {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        let mut probe = pool.begin().await.unwrap();
        let offering_available =
            sqlx::query("SELECT id FROM learning_offerings WHERE id=$1 FOR UPDATE NOWAIT")
                .bind(offering.id)
                .execute(&mut *probe)
                .await
                .is_ok();
        let group_available = if offering_available {
            sqlx::query("SELECT id FROM learning_groups WHERE id=$1 FOR UPDATE NOWAIT")
                .bind(group.id)
                .execute(&mut *probe)
                .await
                .is_ok()
        } else {
            false
        };
        probe.rollback().await.unwrap();
        boundary.commit().await.unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(10), worker)
            .await
            .unwrap()
            .unwrap();
        assert!(
            waiting,
            "operation {operation} must wait for the term boundary"
        );
        assert!(
            offering_available && group_available,
            "operation {operation} locked an entity before its lifecycle boundary"
        );
        if operation != 4 {
            assert!(result.is_ok(), "operation {operation}: {result:?}");
        }
    }
}

#[tokio::test]
async fn lifecycle_delivery_closed_context_retains_history_and_rejects_mutations() {
    let pool = prepare_delivery_runtime_fixture("delivery_lifecycle_mutations").await;
    let context = planning_runtime_context(&pool).await;
    let offering = offerings::create(&pool, context.teacher_id, course_request(&context))
        .await
        .unwrap();
    let group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            registration_open: false,
            name: "Historical group".into(),
            description: None,
            capacity: Some(40),
            preferred_room_ids: vec![],
        },
    )
    .await
    .unwrap();
    for (year_status, term_status) in [("closed", "planning"), ("planning", "closed")] {
        sqlx::query("UPDATE academic_years SET status=$2 WHERE id=$1")
            .bind(context.year_id)
            .bind(year_status)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE academic_terms SET status=$2 WHERE id=$1")
            .bind(context.term_id)
            .bind(term_status)
            .execute(&pool)
            .await
            .unwrap();
        let outcomes = [
            offerings::create(&pool, context.teacher_id, course_request(&context))
                .await
                .map(|_| ()),
            offerings::update(
                &pool,
                context.teacher_id,
                offering.id,
                UpdateLearningOfferingRequest {
                    weekly_period_target: 5,
                    row_version: offering.row_version,
                    targets: course_request(&context).targets().to_vec(),
                },
            )
            .await
            .map(|_| ()),
            groups::create(
                &pool,
                context.teacher_id,
                offering.id,
                CreateLearningGroupRequest {
                    registration_open: false,
                    name: "Must not exist".into(),
                    description: None,
                    capacity: None,
                    preferred_room_ids: vec![],
                },
            )
            .await
            .map(|_| ()),
            groups::update(
                &pool,
                context.teacher_id,
                group.id,
                super::models::UpdateLearningGroupRequest {
                    registration_open: false,
                    row_version: group.row_version,
                    name: "Changed".into(),
                    description: None,
                    capacity: None,
                    preferred_room_ids: vec![],
                },
            )
            .await
            .map(|_| ()),
            groups::replace_teachers(
                &pool,
                context.teacher_id,
                group.id,
                ReplaceLearningGroupTeachersRequest {
                    row_version: group.row_version,
                    teachers: vec![],
                },
            )
            .await
            .map(|_| ()),
            groups::replace_homerooms(
                &pool,
                context.teacher_id,
                group.id,
                ReplaceLearningGroupHomeroomsRequest {
                    row_version: group.row_version,
                    homeroom_ids: vec![],
                },
            )
            .await
            .map(|_| ()),
            offerings::update_weekly_period_target(
                &pool,
                offering.id,
                school_academic_delivery::models::current::UpdateWeeklyPeriodTargetRequest {
                    row_version: offering.row_version,
                    weekly_period_target: 1,
                },
            )
            .await
            .map(|_| ()),
        ];
        for (operation, outcome) in outcomes.into_iter().enumerate() {
            assert!(
                matches!(outcome, Err(AppError::Conflict(_))),
                "{year_status}/{term_status} operation {operation}: {outcome:?}"
            );
        }
        assert_eq!(
            serde_json::to_value(offerings::get(&pool, offering.id).await.unwrap()).unwrap(),
            serde_json::to_value(&offering).unwrap()
        );
        assert_eq!(
            serde_json::to_value(groups::get(&pool, group.id).await.unwrap()).unwrap(),
            serde_json::to_value(&group).unwrap()
        );
    }
}

fn runtime_operation_date() -> NaiveDate {
    Utc::now()
        .with_timezone(&chrono_tz::Asia::Bangkok)
        .date_naive()
}
#[derive(Debug)]
struct RuntimeContext {
    term_id: Uuid,
    year_id: Uuid,
    homeroom_id: Uuid,
    grade_level_id: Uuid,
    study_program_id: Uuid,
    subject_version_id: Uuid,
    owner_id: Uuid,
    teacher_id: Uuid,
}

async fn planning_runtime_context(pool: &PgPool) -> RuntimeContext {
    planning_runtime_context_at(pool, runtime_operation_date()).await
}

async fn planning_runtime_context_at(pool: &PgPool, catalog_date: NaiveDate) -> RuntimeContext {
    let (term_id, year_id): (Uuid, Uuid) = sqlx::query_as(
        r#"SELECT term.id, term.academic_year_id
           FROM academic_terms term
           WHERE term.status = 'planning'
             AND EXISTS (
                 SELECT 1
                 FROM homerooms homeroom
                 JOIN curriculum_course_requirements requirement
                   ON requirement.study_program_id = homeroom.study_program_id
                  AND requirement.grade_level_id = homeroom.grade_level_id
                 JOIN curriculum_term_slots slot ON slot.id = requirement.term_slot_id
                 WHERE homeroom.academic_year_id = term.academic_year_id
                   AND slot.term_type = term.term_type
                   AND slot.type_occurrence = (
                       SELECT count(*)::integer
                       FROM academic_terms occurrence
                       WHERE occurrence.academic_year_id = term.academic_year_id
                         AND occurrence.term_type = term.term_type
                         AND occurrence.sequence_no <= term.sequence_no
                   )
             )
           ORDER BY (SELECT year FROM academic_years WHERE id=term.academic_year_id), term.sequence_no, term.id
           LIMIT 1"#,
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let (homeroom_id, grade_level_id, study_program_id): (Uuid, Uuid, Uuid) = sqlx::query_as(
        "SELECT id, grade_level_id, study_program_id FROM homerooms WHERE academic_year_id = $1 ORDER BY id LIMIT 1",
    )
    .bind(year_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let subject_version_id: Uuid = sqlx::query_scalar(
        r#"SELECT version.id
           FROM subject_versions version
           JOIN academic_terms term ON term.id = $1
           WHERE version.status = 'published'
             AND version.effective_from <= $2::date
           ORDER BY version.id
           LIMIT 1"#,
    )
    .bind(term_id)
    .bind(catalog_date)
    .fetch_one(pool)
    .await
    .unwrap();
    let owner_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM organization_units WHERE is_active ORDER BY parent_unit_id NULLS FIRST, id LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    let teacher_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM users WHERE user_type = 'staff' AND status = 'active' ORDER BY id LIMIT 1",
    )
    .fetch_one(pool)
    .await
    .unwrap();

    RuntimeContext {
        term_id,
        year_id,
        homeroom_id,
        grade_level_id,
        study_program_id,
        subject_version_id,
        owner_id,
        teacher_id,
    }
}

async fn operational_change_runtime_context(pool: &PgPool) -> RuntimeContext {
    let (term_id,year_id,homeroom_id,grade_level_id,study_program_id,subject_version_id,owner_id,teacher_id):(Uuid,Uuid,Uuid,Uuid,Uuid,Uuid,Uuid,Uuid)=sqlx::query_as("SELECT offering.academic_term_id,offering.academic_year_id,room.id,room.grade_level_id,room.study_program_id,detail.subject_version_id,offering.owning_organization_unit_id,teacher.teacher_id FROM learning_offerings offering JOIN academic_terms term ON term.id=offering.academic_term_id JOIN course_offering_details detail ON detail.learning_offering_id=offering.id JOIN learning_groups g ON g.learning_offering_id=offering.id JOIN learning_group_homerooms coverage ON coverage.learning_group_id=g.id JOIN homerooms room ON room.id=coverage.homeroom_id JOIN learning_group_teachers teacher ON teacher.learning_group_id=g.id JOIN users account ON account.id=teacher.teacher_id WHERE term.status='active' AND account.status='active' ORDER BY offering.id,g.id,teacher.id LIMIT 1").fetch_one(pool).await.unwrap();
    RuntimeContext {
        term_id,
        year_id,
        homeroom_id,
        grade_level_id,
        study_program_id,
        subject_version_id,
        owner_id,
        teacher_id,
    }
}

#[tokio::test]
async fn dated_roster_remove_and_readd_preserve_inclusive_membership_history() {
    let pool = prepare_delivery_runtime_fixture("academic_dated_roster_history").await;
    let context = operational_change_runtime_context(&pool).await;
    let (group_id, membership_id, student_year_id, joined_at): (Uuid, Uuid, Uuid, NaiveDate) =
        sqlx::query_as(
            r#"SELECT learning_group.id, membership.id,
                      membership.student_academic_year_id, membership.joined_at
               FROM learning_groups learning_group
               JOIN learning_group_students membership
                 ON membership.learning_group_id = learning_group.id
                AND membership.membership_status = 'active'
               WHERE learning_group.academic_term_id = $1
               ORDER BY learning_group.id, membership.id
               LIMIT 1"#,
        )
        .bind(context.term_id)
        .fetch_one(&pool)
        .await
        .expect("fixture must contain a published roster membership");
    let group = groups::get(&pool, group_id).await.unwrap();
    let membership = roster_memberships::list_memberships(&pool, group_id)
        .await
        .unwrap()
        .into_iter()
        .find(|value| value.id == membership_id)
        .unwrap();
    let left_at = joined_at
        .checked_add_signed(chrono::Duration::days(5))
        .unwrap();

    let ended = roster_memberships::remove_membership(
        &pool,
        context.teacher_id,
        group_id,
        membership_id,
        RemoveDatedRosterMembershipRequest {
            group_row_version: group.row_version,
            membership_row_version: membership.row_version,
            left_at,
        },
    )
    .await
    .expect("a published roster membership must accept an inclusive end date");
    assert_eq!(ended.joined_at, joined_at);
    assert_eq!(ended.left_at, Some(left_at));
    assert_eq!(
        ended.membership_status,
        super::models::MembershipStatus::Ended
    );

    let refreshed_group = groups::get(&pool, group_id).await.unwrap();
    let same_day = roster_memberships::add_membership(
        &pool,
        context.teacher_id,
        group_id,
        AddDatedRosterMembershipRequest {
            group_row_version: refreshed_group.row_version,
            student_academic_year_id: student_year_id,
            joined_at: left_at,
        },
    )
    .await
    .expect_err("inclusive end means a same-day re-add overlaps");
    assert!(matches!(
        same_day,
        AppError::Conflict(_) | AppError::ValidationError(_)
    ));

    let rejoined_at = left_at
        .checked_add_signed(chrono::Duration::days(1))
        .unwrap();
    let rejoined = roster_memberships::add_membership(
        &pool,
        context.teacher_id,
        group_id,
        AddDatedRosterMembershipRequest {
            group_row_version: refreshed_group.row_version,
            student_academic_year_id: student_year_id,
            joined_at: rejoined_at,
        },
    )
    .await
    .expect("a strictly later re-add must create a new interval");
    assert_ne!(rejoined.id, membership_id);
    assert_eq!(rejoined.joined_at, rejoined_at);
    assert_eq!(rejoined.left_at, None);

    let history = roster_memberships::list_memberships(&pool, group_id)
        .await
        .unwrap();
    let earlier = history
        .iter()
        .find(|value| value.id == membership_id)
        .unwrap();
    assert_eq!(earlier.joined_at, joined_at);
    assert_eq!(earlier.left_at, Some(left_at));
    assert!(history.iter().any(|value| value.id == rejoined.id));

    let stale = roster_memberships::remove_membership(
        &pool,
        context.teacher_id,
        group_id,
        rejoined.id,
        RemoveDatedRosterMembershipRequest {
            group_row_version: refreshed_group.row_version,
            membership_row_version: rejoined.row_version,
            left_at: rejoined_at,
        },
    )
    .await
    .expect_err("the group revision changes after re-adding a student");
    assert!(matches!(stale, AppError::Conflict(_)));
}

#[tokio::test]
async fn dated_roster_rejects_closed_terms_and_dates_before_actual_join() {
    let pool = prepare_delivery_runtime_fixture("academic_dated_roster_guards").await;
    let context = operational_change_runtime_context(&pool).await;
    let (group_id, membership_id): (Uuid, Uuid) = sqlx::query_as(
        r#"SELECT learning_group.id, membership.id
           FROM learning_groups learning_group
           JOIN learning_group_students membership
             ON membership.learning_group_id = learning_group.id
            AND membership.membership_status = 'active'
           WHERE learning_group.academic_term_id = $1
           ORDER BY learning_group.id, membership.id
           LIMIT 1"#,
    )
    .bind(context.term_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let group = groups::get(&pool, group_id).await.unwrap();
    let membership = roster_memberships::list_memberships(&pool, group_id)
        .await
        .unwrap()
        .into_iter()
        .find(|value| value.id == membership_id)
        .unwrap();
    let before_offering = membership.joined_at - Duration::days(1);
    let invalid_date = roster_memberships::remove_membership(
        &pool,
        context.teacher_id,
        group_id,
        membership_id,
        RemoveDatedRosterMembershipRequest {
            group_row_version: group.row_version,
            membership_row_version: membership.row_version,
            left_at: before_offering,
        },
    )
    .await
    .expect_err("membership dates before offering availability must fail");
    assert!(matches!(invalid_date, AppError::ValidationError(_)));

    sqlx::query("UPDATE academic_terms SET status='closed' WHERE id=(SELECT academic_term_id FROM learning_groups WHERE id=$1)")
        .bind(group_id)
        .execute(&pool)
        .await
        .unwrap();
    let closed = roster_memberships::remove_membership(
        &pool,
        context.teacher_id,
        group_id,
        membership_id,
        RemoveDatedRosterMembershipRequest {
            group_row_version: group.row_version,
            membership_row_version: membership.row_version,
            left_at: membership.joined_at,
        },
    )
    .await
    .expect_err("closed groups must reject dated roster changes");
    assert!(matches!(closed, AppError::Conflict(_)));
}

fn course_request(context: &RuntimeContext) -> CreateLearningOfferingRequest {
    CreateLearningOfferingRequest::Course(CreateCourseOfferingRequest {
        weekly_period_target: 5,

        academic_term_id: context.term_id,
        subject_version_id: context.subject_version_id,
        curriculum_course_requirement_id: None,
        targets: vec![OfferingTargetInput {
            target_kind: OfferingTargetKind::Homeroom,
            homeroom_id: Some(context.homeroom_id),
            grade_level_id: context.grade_level_id,
            study_program_id: context.study_program_id,
        }],
        assessment_total_score: "100.00".to_string(),
    })
}

#[test]
fn course_offering_wire_contract_owns_only_assessment_total() {
    let base = serde_json::json!({
        "academicTermId": Uuid::new_v4(),
        "weeklyPeriodTarget": 5,
        "subjectVersionId": Uuid::new_v4(),
        "curriculumCourseRequirementId": null,
        "targets": [],
        "assessmentTotalScore": "120.50"
    });
    let parsed = serde_json::from_value::<CreateCourseOfferingRequest>(base.clone()).unwrap();
    assert_eq!(parsed.weekly_period_target, 5);
    let mut missing_date = base.clone();
    missing_date
        .as_object_mut()
        .unwrap()
        .remove("weeklyPeriodTarget");
    assert!(
        serde_json::from_value::<CreateCourseOfferingRequest>(missing_date)
            .unwrap_err()
            .to_string()
            .contains("missing field `weeklyPeriodTarget`")
    );
    let mut legacy = base;
    legacy["gradingPolicy"] = serde_json::json!({"policyCode": "school_default", "totalScore": "100.00", "passingScore": "50.00"});
    let error = serde_json::from_value::<CreateCourseOfferingRequest>(legacy)
        .unwrap_err()
        .to_string();
    assert!(error.contains("unknown field `gradingPolicy`"), "{error}");
}

#[tokio::test]
async fn course_offering_persists_exact_assessment_total() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_assessment_total").await;
    let context = planning_runtime_context(&pool).await;
    let CreateLearningOfferingRequest::Course(mut request) = course_request(&context) else {
        panic!("course fixture must create a course request");
    };
    request.assessment_total_score = "120.50".to_string();
    let offering = offerings::create(
        &pool,
        context.teacher_id,
        CreateLearningOfferingRequest::Course(request),
    )
    .await
    .unwrap();
    let saved = serde_json::to_value(&offering.snapshot).unwrap();
    assert_eq!(saved["assessmentTotalScore"], "120.50");
    assert!(saved.get("gradingPolicy").is_none());
    let stored: String = sqlx::query_scalar("SELECT assessment_total_score::text FROM course_offering_details WHERE learning_offering_id = $1")
        .bind(offering.id).fetch_one(&pool).await.unwrap();
    assert_eq!(stored, "120.50");
    let plan =
        school_academic_assessment::assessment::services::get_plan_detail(&pool, offering.id)
            .await
            .unwrap();
    assert_eq!(plan.assessment_total_score, "120.5");
    assert_eq!(plan.readiness.expected_total_score, "120.5");
}

#[tokio::test]
async fn course_offering_snapshot_keeps_the_catalog_standard_immutable() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_weekly_period_target").await;
    let context = planning_runtime_context(&pool).await;
    let catalog_standard: i32 =
        sqlx::query_scalar("SELECT periods_per_week FROM subject_versions WHERE id = $1")
            .bind(context.subject_version_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(catalog_standard, 3);

    let offering = offerings::create(&pool, context.teacher_id, course_request(&context))
        .await
        .unwrap();
    let LearningOfferingSnapshot::Course(initial_snapshot) = &offering.snapshot else {
        panic!("course creation must return a course snapshot");
    };
    assert_eq!(initial_snapshot.standard_periods_per_week, 3);

    let targets = offering
        .targets
        .iter()
        .map(|target| OfferingTargetInput {
            target_kind: target.target_kind,
            homeroom_id: target.homeroom_id,
            grade_level_id: target.grade_level_id,
            study_program_id: target.study_program_id,
        })
        .collect::<Vec<_>>();
    let updated = offerings::update(
        &pool,
        context.teacher_id,
        offering.id,
        UpdateLearningOfferingRequest {
            weekly_period_target: 5,
            row_version: offering.row_version,
            targets,
        },
    )
    .await
    .unwrap();
    let LearningOfferingSnapshot::Course(updated_snapshot) = &updated.snapshot else {
        panic!("course update must return a course snapshot");
    };
    assert_eq!(updated_snapshot.standard_periods_per_week, 3);
    let unchanged_catalog_standard: i32 =
        sqlx::query_scalar("SELECT periods_per_week FROM subject_versions WHERE id = $1")
            .bind(context.subject_version_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unchanged_catalog_standard, 3);

    let next_term_id: Uuid = sqlx::query_scalar(
        r#"SELECT id
           FROM academic_terms
           WHERE academic_year_id = $1
             AND id <> $2
             AND status = 'planning'
           ORDER BY sequence_no, id
           LIMIT 1"#,
    )
    .bind(context.year_id)
    .bind(context.term_id)
    .fetch_one(&pool)
    .await
    .expect("fixture must include another planning term in the same academic year");
    let mut next_term_request = course_request(&context);
    let CreateLearningOfferingRequest::Course(request) = &mut next_term_request else {
        unreachable!();
    };
    request.academic_term_id = next_term_id;
    let next_term_offering = offerings::create(&pool, context.teacher_id, next_term_request)
        .await
        .unwrap();
    let LearningOfferingSnapshot::Course(next_term_snapshot) = &next_term_offering.snapshot else {
        panic!("next-term course must return a course snapshot");
    };
    assert_eq!(next_term_snapshot.standard_periods_per_week, 3);
}

fn default_preparation_choices(
    preview: &CurriculumOfferingPreview,
) -> Vec<CurriculumPreparationChoice> {
    preview
        .proposals
        .iter()
        .map(|proposal| CurriculumPreparationChoice {
            proposal_id: proposal.proposal_id.clone(),
            action: if proposal.grouping_state == PreparationGroupingState::Proposed
                && proposal.conflicts.is_empty()
            {
                PreparationAction::Apply
            } else {
                PreparationAction::DeferGroups
            },
            groups: if proposal.grouping_state == PreparationGroupingState::Proposed
                && proposal.conflicts.is_empty()
            {
                proposal.default_groups.clone()
            } else {
                Vec::new()
            },
        })
        .collect()
}

#[test]
fn create_offering_wire_contract_is_strictly_tagged_by_kind() {
    let parsed = serde_json::from_value::<CreateLearningOfferingRequest>(serde_json::json!({
        "kind": "course",
        "academicTermId": Uuid::nil(),
        "weeklyPeriodTarget": 5,
        "subjectVersionId": Uuid::nil(),
        "targets": [],
        "assessmentTotalScore": "100.00"
    }))
    .unwrap();
    let CreateLearningOfferingRequest::Course(course) = parsed else {
        panic!("course tag must select the course DTO")
    };
    assert_eq!(course.weekly_period_target, 5);

    let missing_date = serde_json::from_value::<CreateLearningOfferingRequest>(serde_json::json!({
        "kind": "course", "academicTermId": Uuid::nil(), "subjectVersionId": Uuid::nil(),
        "targets": [], "assessmentTotalScore": "100.00"
    }))
    .unwrap_err()
    .to_string();
    assert!(
        missing_date.contains("missing field `weeklyPeriodTarget`"),
        "{missing_date}"
    );

    let wrong_subtype =
        serde_json::from_value::<CreateLearningOfferingRequest>(serde_json::json!({
            "kind": "course",
            "academicTermId": Uuid::nil(),
        "weeklyPeriodTarget": 5,
            "activityVersionId": Uuid::nil(),
            "targets": [],
            "assessmentTotalScore": "100.00"
        }));
    let error = wrong_subtype.unwrap_err().to_string();
    assert!(
        error.contains("unknown field `activityVersionId`"),
        "{error}"
    );
}

#[tokio::test]
async fn catalog_start_only_former_expiry_allows_explicit_offering_pin_and_preserves_facts() {
    let pool = prepare_delivery_runtime_fixture("catalog_start_only_explicit_old_pin").await;
    let context = planning_runtime_context(&pool).await;
    // The historical Passing fixture was imported before migration 138. Select
    // preserved finite-expiry evidence; no existing year, room or version is edited.
    let (old_id,stable_id,code,name,credit,hours,standard,source_before,former_expiry,offering_start):(Uuid,Uuid,String,String,String,Option<String>,i32,serde_json::Value,NaiveDate,NaiveDate)=sqlx::query_as(
        "SELECT version.id,version.subject_id,subject.code,version.name_th,version.credit::text,version.hours_per_semester::text,version.periods_per_week,to_jsonb(version),(version.migration_provenance->'historicalCatalogContext'->>'effectiveUntil')::date,$2::date FROM subject_versions version JOIN subjects subject ON subject.id=version.subject_id JOIN academic_terms term ON term.id=$1 WHERE version.status='published' AND version.effective_from<=$2::date AND (version.migration_provenance->'historicalCatalogContext'->>'effectiveUntil')::date<$2::date AND version.periods_per_week IS NOT NULL AND version.hours_per_semester IS NOT NULL ORDER BY version.effective_from DESC,version.version_no DESC LIMIT 1"
    ).bind(context.term_id).bind(runtime_operation_date())
    .fetch_one(&pool).await.unwrap();
    assert!(former_expiry < offering_start);
    let mut request = course_request(&context);
    let CreateLearningOfferingRequest::Course(course) = &mut request else {
        unreachable!()
    };
    course.subject_version_id = old_id;
    let offering = offerings::create(&pool, context.teacher_id, request)
        .await
        .unwrap();
    assert_eq!(offering.academic_year_id, context.year_id);
    assert_eq!(offering.academic_term_id, context.term_id);
    assert_eq!(offering.code_snapshot, code);
    assert_eq!(offering.name_snapshot, name);
    assert_eq!(offering.targets.len(), 1);
    assert_eq!(offering.targets[0].homeroom_id, Some(context.homeroom_id));
    let LearningOfferingSnapshot::Course(snapshot) = &offering.snapshot else {
        panic!("an explicit course pin must retain a course snapshot")
    };
    assert_eq!(snapshot.subject_version_id, old_id);
    assert_eq!(snapshot.subject_id, stable_id);
    assert_eq!(
        snapshot.credit.parse::<bigdecimal::BigDecimal>().unwrap(),
        credit.parse::<bigdecimal::BigDecimal>().unwrap()
    );
    assert_eq!(
        snapshot
            .hours
            .as_deref()
            .unwrap()
            .parse::<bigdecimal::BigDecimal>()
            .unwrap(),
        hours
            .as_deref()
            .unwrap()
            .parse::<bigdecimal::BigDecimal>()
            .unwrap()
    );
    assert_eq!(snapshot.standard_periods_per_week, standard);
    assert_eq!(snapshot.assessment_total_score, "100.00");
    let source_after: serde_json::Value =
        sqlx::query_scalar("SELECT to_jsonb(version) FROM subject_versions version WHERE id=$1")
            .bind(old_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(source_before, source_after);
    let reloaded = offerings::get(&pool, offering.id).await.unwrap();
    let LearningOfferingSnapshot::Course(reloaded) = reloaded.snapshot else {
        panic!("readback must retain the pinned course snapshot")
    };
    assert_eq!(reloaded.subject_version_id, old_id);
    assert_eq!(reloaded.subject_id, stable_id);
}

#[tokio::test]
async fn curriculum_preview_apply_is_hash_checked_and_closed_terms_reject_writes() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_runtime_curriculum").await;
    let context = planning_runtime_context(&pool).await;

    let earlier_edition_program_id: Uuid = sqlx::query_scalar(
        r#"SELECT program.id
           FROM study_programs program
           JOIN curriculum_levels version ON version.id = program.curriculum_level_id
           JOIN academic_years former_ending_year ON former_ending_year.id =
               (version.migration_provenance->'revisionSelection'->>'legacyEndAcademicYearId')::uuid
           JOIN academic_years selected_year ON selected_year.id = $1
           WHERE former_ending_year.year < selected_year.year
           ORDER BY program.id
           LIMIT 1"#,
    )
    .bind(context.year_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let earlier_edition = offerings::preview_from_curriculum(
        &pool,
        PreviewCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![earlier_edition_program_id],
        },
    )
    .await;
    assert!(
        matches!(&earlier_edition,Err(AppError::ValidationError(message))
            if message == "ไม่พบห้องประจำชั้นที่ใช้แผนการเรียนนี้ในปีการศึกษาของภาคเรียนที่เลือก"),
        "the earlier catalog pin passes expiry removal; its independent missing-room guard remains: {earlier_edition:?}"
    );

    let preview = offerings::preview_from_curriculum(
        &pool,
        PreviewCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
        },
    )
    .await
    .unwrap();
    assert_eq!(preview.curriculum_sources.len(), 1);
    assert_eq!(preview.curriculum_sources[0].publication_no, 1);
    let publication_id = preview.curriculum_sources[0].publication_id;
    assert!(!preview.source_hash.is_empty());
    assert!(!preview.proposals.is_empty());
    assert!(
        preview
            .proposals
            .iter()
            .any(|proposal| !proposal.default_groups.is_empty())
    );
    let choices = default_preparation_choices(&preview);

    let mismatched = offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],

            source_hash: "stale-source-hash".to_string(),
            idempotency_key: Uuid::new_v4(),
            choices: choices.clone(),
        },
    )
    .await;
    assert!(matches!(mismatched, Err(AppError::Conflict(_))));

    let idempotency_key = Uuid::new_v4();
    let applied = offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],

            source_hash: preview.source_hash.clone(),
            idempotency_key,
            choices: choices.clone(),
        },
    )
    .await
    .unwrap();
    let recorded: serde_json::Value = sqlx::query_scalar(
        "SELECT curriculum_sources FROM learning_delivery_apply_runs WHERE idempotency_key=$1",
    )
    .bind(idempotency_key)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        recorded,
        serde_json::to_value(&preview.curriculum_sources).unwrap()
    );
    let source_count:i64=sqlx::query_scalar("SELECT count(DISTINCT learning_offering_id) FROM learning_offering_curriculum_sources WHERE learning_offering_id=ANY($1) AND publication_id=$2")
        .bind(&applied.offering_ids).bind(publication_id).fetch_one(&pool).await.unwrap();
    assert_eq!(
        source_count, applied.created_offering_count as i64,
        "retained legacy offerings must not acquire invented provenance"
    );
    let captured = current_workspace(&pool, context.year_id, context.term_id)
        .await
        .unwrap();
    assert_eq!(
        captured
            .offerings
            .iter()
            .filter(|offering| offering
                .curriculum_sources
                .iter()
                .any(|source| source.publication_id == publication_id))
            .count(),
        applied.created_offering_count
    );
    let retried = offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],

            source_hash: preview.source_hash.clone(),
            idempotency_key,
            choices,
        },
    )
    .await
    .unwrap();
    assert_eq!(retried.offering_ids, applied.offering_ids);
    assert_eq!(retried.group_ids, applied.group_ids);
    assert!(
        applied.created_offering_count > 0,
        "the first receipt must bind a newly created offering date"
    );
    let changed_date = offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],

            source_hash: preview.source_hash.clone(),
            idempotency_key,
            choices: default_preparation_choices(&preview),
        },
    )
    .await;
    assert!(matches!(changed_date, Err(AppError::Conflict(_))));

    assert!(!applied.group_ids.is_empty());
    let course_totals: Vec<String> = sqlx::query_scalar(
        "SELECT assessment_total_score::text FROM course_offering_details WHERE learning_offering_id = ANY($1)",
    ).bind(&applied.offering_ids).fetch_all(&pool).await.unwrap();
    assert!(!course_totals.is_empty());
    assert!(course_totals.iter().all(|total| total == "100.00"));
    let mut descriptor_ids = applied.offering_ids.clone();
    descriptor_ids.reverse();
    let descriptors = offerings::signal_descriptors(&pool, &descriptor_ids)
        .await
        .unwrap();
    assert_eq!(
        descriptors
            .iter()
            .map(|descriptor| descriptor.learning_offering_id)
            .collect::<Vec<_>>(),
        descriptor_ids
    );
    assert!(descriptors.iter().all(|descriptor| {
        descriptor.academic_term_id == context.term_id && descriptor.row_version > 0
    }));

    let retained_preview = offerings::preview_from_curriculum(
        &pool,
        PreviewCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
        },
    )
    .await
    .unwrap();
    let retained_choices = default_preparation_choices(&retained_preview);
    let retained_key = Uuid::new_v4();
    let retained = offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],

            source_hash: retained_preview.source_hash.clone(),
            idempotency_key: retained_key,
            choices: retained_choices.clone(),
        },
    )
    .await
    .unwrap();
    assert_eq!(retained.created_offering_count, 0);
    assert_eq!(retained.created_group_count, 0);
    assert_eq!(retained.offering_ids, applied.offering_ids);

    let facts_before:serde_json::Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(offering) ORDER BY id) FROM learning_offerings offering WHERE id=ANY($1)").bind(&retained.offering_ids).fetch_one(&pool).await.unwrap();
    let retained_replay = offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],

            source_hash: retained_preview.source_hash.clone(),
            idempotency_key: retained_key,
            choices: retained_choices,
        },
    )
    .await
    .unwrap();
    assert_eq!(retained_replay.offering_ids, retained.offering_ids);
    assert_eq!(retained_replay.group_ids, retained.group_ids);
    let facts_after:serde_json::Value=sqlx::query_scalar("SELECT jsonb_agg(to_jsonb(offering) ORDER BY id) FROM learning_offerings offering WHERE id=ANY($1)").bind(&retained.offering_ids).fetch_one(&pool).await.unwrap();
    assert_eq!(facts_before, facts_after);

    let closed_term_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_terms WHERE status IN ('closed', 'cancelled') ORDER BY (SELECT year FROM academic_years WHERE id=academic_terms.academic_year_id), sequence_no, id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut closed_request = course_request(&context);
    let CreateLearningOfferingRequest::Course(request) = &mut closed_request else {
        unreachable!();
    };
    request.academic_term_id = closed_term_id;
    let closed = offerings::create(&pool, context.teacher_id, closed_request).await;
    assert!(matches!(closed, Err(AppError::Conflict(_))));
}

#[tokio::test]
async fn delivery_signal_descriptors_preserve_unique_request_order_and_reject_unknown_ids() {
    let pool = prepare_delivery_runtime_fixture("delivery_signal_descriptor_identity").await;
    let ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM learning_offerings ORDER BY id LIMIT 2")
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(ids.len(), 2);
    let known = offerings::signal_descriptors(&pool, &ids).await.unwrap();
    let signature = |descriptor: &school_academic_delivery::services::offerings::LearningOfferingSignalDescriptor| {
        (descriptor.learning_offering_id, descriptor.academic_term_id, descriptor.row_version)
    };
    let requested = [ids[1], ids[0], ids[1], ids[0]];
    let expected = vec![signature(&known[1]), signature(&known[0])];
    let descriptors = offerings::signal_descriptors(&pool, &requested)
        .await
        .unwrap();
    assert_eq!(
        descriptors.iter().map(signature).collect::<Vec<_>>(),
        expected
    );
    let mut connection = pool.acquire().await.unwrap();
    let descriptors = offerings::signal_descriptors_on_connection(&mut connection, &requested)
        .await
        .unwrap();
    assert_eq!(
        descriptors.iter().map(signature).collect::<Vec<_>>(),
        expected
    );
    let missing = Uuid::from_u128(u128::MAX);
    assert!(!ids.contains(&missing));
    for requested in [vec![ids[0], missing, ids[0]], vec![missing, missing]] {
        assert!(matches!(
            offerings::signal_descriptors_on_connection(&mut connection, &requested).await,
            Err(AppError::InternalServerError(_))
        ));
    }
    drop(connection);
    assert!(matches!(
        offerings::signal_descriptors(&pool, &[ids[1], missing, ids[1]]).await,
        Err(AppError::InternalServerError(_))
    ));
}

#[tokio::test]
async fn curriculum_preparation_groups_support_reviewed_combined_split_and_manual_conflicts() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_preparation_groups").await;
    let context = planning_runtime_context(&pool).await;
    let second_homeroom_id = stable_uuid("preparation-second-homeroom");
    sqlx::query(
        r#"INSERT INTO homerooms (
               id, code, name, academic_year_id, grade_level_id, room_number,
               is_active, metadata, study_program_id, capacity
           )
           SELECT $1, 'PREP-ROOM-2', 'ห้องเตรียมสอง', academic_year_id,
                  grade_level_id, 'PREP-2', true, '{}'::jsonb,
                  study_program_id, capacity
           FROM homerooms
           WHERE id = $2"#,
    )
    .bind(second_homeroom_id)
    .bind(context.homeroom_id)
    .execute(&pool)
    .await
    .unwrap();

    let preview = offerings::preview_from_curriculum(
        &pool,
        PreviewCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
        },
    )
    .await
    .unwrap();
    let proposed_ids = preview
        .proposals
        .iter()
        .filter(|proposal| {
            proposal.grouping_state == PreparationGroupingState::Proposed
                && proposal.target_homeroom_ids.contains(&context.homeroom_id)
                && proposal.target_homeroom_ids.contains(&second_homeroom_id)
        })
        .map(|proposal| proposal.proposal_id.clone())
        .take(2)
        .collect::<Vec<_>>();
    assert_eq!(proposed_ids.len(), 2);

    let mut empty_apply_choices = default_preparation_choices(&preview);
    empty_apply_choices
        .iter_mut()
        .find(|choice| choice.proposal_id == proposed_ids[0])
        .unwrap()
        .groups = Vec::new();
    let empty_apply = offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],

            source_hash: preview.source_hash.clone(),
            idempotency_key: Uuid::new_v4(),
            choices: empty_apply_choices,
        },
    )
    .await;
    assert!(matches!(empty_apply, Err(AppError::ValidationError(_))));

    let mut choices = default_preparation_choices(&preview);
    let combined = choices
        .iter_mut()
        .find(|choice| choice.proposal_id == proposed_ids[0])
        .unwrap();
    combined.groups = vec![super::models::CurriculumGroupProposal {
        group_key: "a".repeat(64),
        name: "กลุ่มเรียนรวมสองห้อง".to_string(),
        homeroom_ids: vec![context.homeroom_id, second_homeroom_id],
    }];
    let split = choices
        .iter_mut()
        .find(|choice| choice.proposal_id == proposed_ids[1])
        .unwrap();
    split.groups = vec![
        super::models::CurriculumGroupProposal {
            group_key: "b".repeat(64),
            name: "กลุ่มแบ่ง A".to_string(),
            homeroom_ids: vec![context.homeroom_id],
        },
        super::models::CurriculumGroupProposal {
            group_key: "c".repeat(64),
            name: "กลุ่มแบ่ง B".to_string(),
            homeroom_ids: vec![context.homeroom_id],
        },
    ];

    let result = offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],

            source_hash: preview.source_hash,
            idempotency_key: Uuid::new_v4(),
            choices,
        },
    )
    .await
    .unwrap();
    assert!(result.created_group_count >= 3);
    let combined_coverage: i64 = sqlx::query_scalar(
        r#"SELECT count(*)
           FROM learning_group_homerooms coverage
           JOIN learning_groups learning_group
             ON learning_group.id = coverage.learning_group_id
           WHERE learning_group.generation_key = $1"#,
    )
    .bind("a".repeat(64))
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(combined_coverage, 2);
    let split_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM learning_groups WHERE generation_key = ANY($1)")
            .bind(vec!["b".repeat(64), "c".repeat(64)])
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(split_count, 2);

    let offering_id: Uuid = sqlx::query_scalar(
        "SELECT learning_offering_id FROM learning_groups WHERE generation_key = $1",
    )
    .bind("a".repeat(64))
    .fetch_one(&pool)
    .await
    .unwrap();
    let manual_group = groups::create(
        &pool,
        context.teacher_id,
        offering_id,
        CreateLearningGroupRequest {
            registration_open: false,
            name: "กลุ่มที่ครูจัดเอง".to_string(),
            description: None,
            capacity: Some(40),
            preferred_room_ids: Vec::new(),
        },
    )
    .await
    .unwrap();
    groups::replace_homerooms(
        &pool,
        context.teacher_id,
        manual_group.id,
        ReplaceLearningGroupHomeroomsRequest {
            row_version: manual_group.row_version,
            homeroom_ids: vec![context.homeroom_id],
        },
    )
    .await
    .unwrap();
    let next_preview = offerings::preview_from_curriculum(
        &pool,
        PreviewCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
        },
    )
    .await
    .unwrap();
    assert!(next_preview.proposals.iter().any(|proposal| {
        proposal.existing_offering_id == Some(offering_id)
            && proposal.grouping_state == PreparationGroupingState::Conflict
            && proposal
                .conflicts
                .iter()
                .any(|conflict| conflict.code == "manual_group_overlap")
    }));
}

#[tokio::test]
async fn self_registration_activity_uses_common_delivery() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_runtime_self_activity").await;
    let context = planning_runtime_context(&pool).await;
    let (activity_version_id, scheduling_mode): (Uuid, ActivitySchedulingMode) = sqlx::query_as(
        r#"SELECT version.id, version.scheduling_mode
           FROM activity_versions version
           JOIN academic_terms term ON term.id = $1
           WHERE version.status = 'published'
             AND version.effective_from <= $2::date
           ORDER BY version.id
           LIMIT 1"#,
    )
    .bind(context.term_id)
    .bind(runtime_operation_date())
    .fetch_one(&pool)
    .await
    .unwrap();
    let offering = offerings::create(
        &pool,
        context.teacher_id,
        CreateLearningOfferingRequest::Activity(CreateActivityOfferingRequest {
            weekly_period_target: 1,

            create_homeroom_groups: false,
            academic_term_id: context.term_id,
            activity_version_id,
            curriculum_activity_requirement_id: None,
            targets: vec![OfferingTargetInput {
                target_kind: OfferingTargetKind::Homeroom,
                homeroom_id: Some(context.homeroom_id),
                grade_level_id: context.grade_level_id,
                study_program_id: context.study_program_id,
            }],
            registration_type: ActivityRegistrationType::SelfRegistration,
            scheduling_mode,
            capacity: Some(1),
            attendance_requirement: ActivityAttendanceRequirement {
                minimum_percent: Some("80.00".to_string()),
                required_sessions: None,
            },
            pass_criteria: ActivityPassCriteria {
                require_attendance: true,
                require_teacher_confirmation: true,
                outcomes: vec!["pass".to_string(), "fail".to_string()],
            },
        }),
    )
    .await
    .unwrap();
    assert_eq!(offering.kind, LearningOfferingKind::Activity);

    let group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            registration_open: false,
            name: "กิจกรรมสมัครเอง".to_string(),
            description: None,
            capacity: Some(1),
            preferred_room_ids: Vec::new(),
        },
    )
    .await
    .unwrap();
    let group = groups::replace_teachers(
        &pool,
        context.teacher_id,
        group.id,
        ReplaceLearningGroupTeachersRequest {
            row_version: group.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: context.teacher_id,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap();
    let group = groups::replace_homerooms(
        &pool,
        context.teacher_id,
        group.id,
        ReplaceLearningGroupHomeroomsRequest {
            row_version: group.row_version,
            homeroom_ids: vec![context.homeroom_id],
        },
    )
    .await
    .unwrap();

    let group = groups::get(&pool, group.id).await.unwrap();
    let preview = groups::preview_roster(&pool, group.id).await.unwrap();
    assert_eq!(preview.added, 0);
    let student_academic_year_id: Uuid = sqlx::query_scalar(
        r#"SELECT placement.student_academic_year_id
           FROM homeroom_placements placement
           WHERE placement.homeroom_id = $1
             AND placement.status = 'current' AND placement.end_date IS NULL
           ORDER BY placement.id
           LIMIT 1"#,
    )
    .bind(context.homeroom_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let group = groups::apply_roster(
        &pool,
        context.teacher_id,
        group.id,
        ApplyRosterRequest {
            row_version: group.row_version,
            source_hash: preview.source_hash,
            overrides: vec![RosterOverrideInput {
                student_academic_year_id,
                action: RosterOverrideAction::Add,
            }],
        },
    )
    .await
    .unwrap();

    let roster_sources: Vec<String> = sqlx::query_scalar(
        "SELECT roster_source FROM learning_group_students WHERE learning_group_id = $1",
    )
    .bind(group.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(roster_sources, vec!["manual_add"]);
}

#[tokio::test]
async fn student_activity_registration_without_academic_date_boundaries_is_term_scoped_eligible_and_revisioned()
 {
    let pool =
        prepare_delivery_runtime_fixture("academic_delivery_student_activity_registration").await;
    let context = planning_runtime_context(&pool).await;
    let student_id = Uuid::parse_str("50000000-0000-0000-0000-000000000001").unwrap();
    let (activity_version_id, scheduling_mode): (Uuid, ActivitySchedulingMode) = sqlx::query_as(
        r#"SELECT version.id, version.scheduling_mode
           FROM activity_versions version
           JOIN academic_terms term ON term.id = $1
           WHERE version.status = 'published'
             AND version.effective_from <= $2::date
           ORDER BY version.id
           LIMIT 1"#,
    )
    .bind(context.term_id)
    .bind(runtime_operation_date())
    .fetch_one(&pool)
    .await
    .unwrap();
    let offering = offerings::create(
        &pool,
        context.teacher_id,
        CreateLearningOfferingRequest::Activity(CreateActivityOfferingRequest {
            weekly_period_target: 1,

            create_homeroom_groups: false,
            academic_term_id: context.term_id,
            activity_version_id,
            curriculum_activity_requirement_id: None,
            targets: vec![OfferingTargetInput {
                target_kind: OfferingTargetKind::Homeroom,
                homeroom_id: Some(context.homeroom_id),
                grade_level_id: context.grade_level_id,
                study_program_id: context.study_program_id,
            }],
            registration_type: ActivityRegistrationType::SelfRegistration,
            scheduling_mode,
            capacity: Some(2),
            attendance_requirement: ActivityAttendanceRequirement {
                minimum_percent: Some("80.00".to_string()),
                required_sessions: None,
            },
            pass_criteria: ActivityPassCriteria {
                require_attendance: true,
                require_teacher_confirmation: true,
                outcomes: vec!["pass".to_string(), "fail".to_string()],
            },
        }),
    )
    .await
    .unwrap();
    let first_group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            registration_open: false,
            name: "ชุมนุมดาราศาสตร์".to_string(),
            description: Some("เรียนรู้ท้องฟ้าผ่านการสังเกตจริง".to_string()),
            capacity: Some(1),
            preferred_room_ids: Vec::new(),
        },
    )
    .await
    .unwrap();
    let unlinked_workspace = workspaces::homeroom_delivery_workspace(
        &pool,
        context.year_id,
        context.term_id,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let unlinked_activity = unlinked_workspace
        .unlinked
        .iter()
        .find(|item| item.group_id == Some(first_group.id))
        .expect("an activity group without homerooms must retain its resource kind");
    assert_eq!(
        unlinked_activity.resource_kind,
        LearningOfferingKind::Activity
    );
    let first_group = groups::replace_teachers(
        &pool,
        context.teacher_id,
        first_group.id,
        ReplaceLearningGroupTeachersRequest {
            row_version: first_group.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: context.teacher_id,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap();
    let first_group = groups::replace_homerooms(
        &pool,
        context.teacher_id,
        first_group.id,
        ReplaceLearningGroupHomeroomsRequest {
            row_version: first_group.row_version,
            homeroom_ids: vec![context.homeroom_id],
        },
    )
    .await
    .unwrap();
    let second_group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            registration_open: false,
            name: "ชุมนุมหุ่นยนต์".to_string(),
            description: None,
            capacity: Some(2),
            preferred_room_ids: Vec::new(),
        },
    )
    .await
    .unwrap();
    let second_group = groups::replace_teachers(
        &pool,
        context.teacher_id,
        second_group.id,
        ReplaceLearningGroupTeachersRequest {
            row_version: second_group.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: context.teacher_id,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap();
    let second_group = groups::replace_homerooms(
        &pool,
        context.teacher_id,
        second_group.id,
        ReplaceLearningGroupHomeroomsRequest {
            row_version: second_group.row_version,
            homeroom_ids: vec![context.homeroom_id],
        },
    )
    .await
    .unwrap();

    let query = StudentActivityRegistrationQuery {
        academic_term_id: context.term_id,
    };
    assert!(
        activities::list_registration_options(&pool, student_id, query.clone())
            .await
            .unwrap()
            .is_empty()
    );

    let available = activities::list_registration_options(&pool, student_id, query.clone())
        .await
        .unwrap();
    assert_eq!(available.len(), 1);
    assert_eq!(available[0].id, offering.id);
    assert_eq!(available[0].groups.len(), 2);
    assert_eq!(available[0].groups[0].capacity, Some(1));
    assert_eq!(available[0].groups[0].member_count, 0);
    assert!(available[0].groups[0].registration_open);
    assert!(!available[0].groups[0].teacher_names.is_empty());
    assert_eq!(available[0].enrolled_group_id, None);

    let enrolled = activities::enroll(&pool, student_id, first_group.id, query.clone())
        .await
        .unwrap();
    assert!(enrolled.enrolled);
    assert_eq!(enrolled.learning_offering_id, offering.id);
    assert_eq!(enrolled.learning_group_id, first_group.id);

    let duplicate = activities::enroll(&pool, student_id, second_group.id, query.clone()).await;
    assert!(matches!(duplicate, Err(AppError::Conflict(_))));

    let after_enroll = activities::list_registration_options(&pool, student_id, query.clone())
        .await
        .unwrap();
    assert_eq!(after_enroll[0].enrolled_group_id, Some(first_group.id));
    assert_eq!(after_enroll[0].groups[0].member_count, 1);

    let removed = activities::unenroll(&pool, student_id, first_group.id, query.clone())
        .await
        .unwrap();
    assert!(!removed.enrolled);

    let after_remove = activities::list_registration_options(&pool, student_id, query)
        .await
        .unwrap();
    assert_eq!(after_remove[0].enrolled_group_id, None);
    assert_eq!(after_remove[0].groups[0].member_count, 0);

    let audit_events: Vec<String> = sqlx::query_scalar(
        r#"SELECT event_code
           FROM academic_audit_events
           WHERE actor_user_id = $1
             AND event_code IN ('activity.self_registered', 'activity.self_unregistered')
           ORDER BY event_code"#,
    )
    .bind(student_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        audit_events,
        vec![
            "activity.self_registered".to_string(),
            "activity.self_unregistered".to_string()
        ]
    );
}

#[tokio::test]
async fn offering_list_batch_hydrates_mixed_snapshots_and_targets() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_batch_list").await;
    let academic_term_id: Uuid = sqlx::query_scalar(
        r#"SELECT academic_term_id
           FROM learning_offerings
           GROUP BY academic_term_id
           ORDER BY COUNT(*) DESC, academic_term_id
           LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();

    let values = offerings::list(
        &pool,
        LearningOfferingQuery { academic_term_id },
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert!(values.len() >= 2);
    assert!(values.iter().all(|offering| !offering.targets.is_empty()));
    assert!(
        values
            .iter()
            .any(|offering| matches!(&offering.snapshot, LearningOfferingSnapshot::Course(_)))
    );
    assert!(
        values
            .iter()
            .any(|offering| matches!(&offering.snapshot, LearningOfferingSnapshot::Activity(_)))
    );
    assert!(values.iter().all(|offering| matches!(
        (offering.kind, &offering.snapshot),
        (
            LearningOfferingKind::Course,
            LearningOfferingSnapshot::Course(_)
        ) | (
            LearningOfferingKind::Activity,
            LearningOfferingSnapshot::Activity(_)
        )
    )));
}

#[tokio::test]
async fn delivery_overview_batches_labels_and_group_coverage() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_workspace_overview").await;
    let context = planning_runtime_context(&pool).await;
    let offering = offerings::create(&pool, context.teacher_id, course_request(&context))
        .await
        .unwrap();

    let first_group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            registration_open: false,
            name: "กลุ่มภาพรวมหนึ่ง".to_string(),
            description: None,
            capacity: Some(40),
            preferred_room_ids: Vec::new(),
        },
    )
    .await
    .unwrap();
    let first_group = groups::replace_teachers(
        &pool,
        context.teacher_id,
        first_group.id,
        ReplaceLearningGroupTeachersRequest {
            row_version: first_group.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: context.teacher_id,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap();
    let first_group = groups::replace_homerooms(
        &pool,
        context.teacher_id,
        first_group.id,
        ReplaceLearningGroupHomeroomsRequest {
            row_version: first_group.row_version,
            homeroom_ids: vec![context.homeroom_id],
        },
    )
    .await
    .unwrap();

    let second_group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            registration_open: false,
            name: "กลุ่มภาพรวมสอง".to_string(),
            description: None,
            capacity: Some(40),
            preferred_room_ids: Vec::new(),
        },
    )
    .await
    .unwrap();
    let _second_group = groups::replace_teachers(
        &pool,
        context.teacher_id,
        second_group.id,
        ReplaceLearningGroupTeachersRequest {
            row_version: second_group.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: context.teacher_id,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap();
    let current_offering = offerings::get(&pool, offering.id).await.unwrap();

    let first_group = groups::get(&pool, first_group.id).await.unwrap();
    let roster_preview = groups::preview_roster(&pool, first_group.id).await.unwrap();
    let first_group = groups::apply_roster(
        &pool,
        context.teacher_id,
        first_group.id,
        ApplyRosterRequest {
            row_version: first_group.row_version,
            source_hash: roster_preview.source_hash,
            overrides: Vec::new(),
        },
    )
    .await
    .unwrap();

    assert_eq!(
        first_group.teacher_assignments[0].teacher_id,
        context.teacher_id
    );
    let overview = workspaces::delivery_overview(
        &pool,
        context.term_id,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .expect("delivery overview should load");
    let summary = overview
        .offerings
        .iter()
        .find(|item| item.offering.id == current_offering.id)
        .expect("created offering should appear");
    assert_eq!(summary.grade_levels.len(), 1);
    assert_eq!(summary.grade_levels[0].id, context.grade_level_id);
    assert!(!summary.grade_levels[0].name.is_empty());
    assert_eq!(summary.study_programs.len(), 1);
    assert_eq!(summary.study_programs[0].id, context.study_program_id);
    assert!(!summary.study_programs[0].name.is_empty());
    assert_eq!(summary.group_count, 2);
    assert_eq!(summary.teacher_assignment_count, 2);
    assert_eq!(summary.groups_without_primary_teacher, 0);

    let outside_owner_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM organization_units WHERE is_active AND id <> $1 ORDER BY id LIMIT 1",
    )
    .bind(context.owner_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let outside_scope = workspaces::delivery_overview(
        &pool,
        context.term_id,
        &AcademicResourceListFilter {
            organization_unit_ids: vec![outside_owner_id],
            ..Default::default()
        },
    )
    .await
    .expect("organization overview should load");
    assert!(
        !outside_scope
            .offerings
            .iter()
            .any(|item| item.offering.id == current_offering.id)
    );
}

#[tokio::test]
async fn homeroom_delivery_classification_is_pinned_and_independent_of_requirement_kind() {
    let pool = create_named_test_pool("homeroom_delivery_catalog_classification").await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    // Classifications are fixture input before the append-only catalog is created.
    sqlx::query("UPDATE subjects SET type='ADDITIONAL' WHERE code='SCI-CORE'")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE subjects SET type='LEGACY-UNKNOWN' WHERE id='20000000-0000-0000-0000-000000000024'",
    )
    .execute(&pool)
    .await
    .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, 138).await.unwrap();
    let context = planning_runtime_context(&pool).await;
    let edition_id: Uuid = sqlx::query_scalar(
        "SELECT level.edition_id FROM study_programs program
         JOIN curriculum_levels level ON level.id=program.curriculum_level_id WHERE program.id=$1",
    )
    .bind(context.study_program_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let edition = school_academic_core::services::curriculum::get(&pool, edition_id)
        .await
        .unwrap();
    school_academic_core::services::curriculum_publications::open_draft(
        &pool,
        edition_id,
        school_academic_core::models::OpenCurriculumDraftRequest {
            row_version: edition.row_version,
        },
    )
    .await
    .unwrap();
    sqlx::query(
        "UPDATE curriculum_course_requirements requirement SET requirement_kind='elective'
         FROM subject_versions version WHERE version.id=requirement.subject_version_id
         AND version.type='BASIC' AND requirement.study_program_id=$1",
    )
    .bind(context.study_program_id)
    .execute(&pool)
    .await
    .unwrap();
    // Historical fixture rows retain pinned classifications independently of required study.
    for classification in ["ADDITIONAL", "LEGACY-UNKNOWN"] {
        sqlx::query(
        "INSERT INTO curriculum_course_requirements
         (id,curriculum_level_id,study_program_id,subject_version_id,grade_level_id,term_slot_id,requirement_kind,display_order)
         SELECT gen_random_uuid(),source.curriculum_level_id,source.study_program_id,version.id,
                source.grade_level_id,source.term_slot_id,'required',100
         FROM curriculum_course_requirements source CROSS JOIN subject_versions version
         JOIN academic_terms term ON term.id=$3
         JOIN curriculum_term_slots slot ON slot.id=source.term_slot_id
         WHERE source.study_program_id=$1 AND source.grade_level_id=$2 AND version.type=$4
           AND slot.term_type=term.term_type AND slot.type_occurrence=(SELECT count(*)
               FROM academic_terms sibling WHERE sibling.academic_year_id=term.academic_year_id
                 AND sibling.term_type=term.term_type AND sibling.sequence_no<=term.sequence_no)
           AND NOT EXISTS (SELECT 1 FROM curriculum_course_requirements existing
               WHERE existing.study_program_id=source.study_program_id
                 AND existing.grade_level_id=source.grade_level_id
                 AND existing.term_slot_id=source.term_slot_id AND existing.subject_version_id=version.id)
         ORDER BY source.id,version.id LIMIT 1",
    )
    .bind(context.study_program_id)
    .bind(context.grade_level_id)
    .bind(context.term_id)
    .bind(classification)
    .execute(&pool)
    .await
    .unwrap();
    }
    sqlx::query(
        "SELECT capture_curriculum_publication($1,$2,'Classification presentation fixture')",
    )
    .bind(edition_id)
    .bind(context.teacher_id)
    .execute(&pool)
    .await
    .unwrap();
    async fn pinned_classifications(pool: &PgPool, program: Uuid) -> serde_json::Value {
        sqlx::query_scalar(
            "SELECT jsonb_agg(fact ORDER BY fact::text) FROM (
                SELECT jsonb_build_object('requirementId',requirement.id,'kind','course',
                    'catalogVersionId',version.id,'subjectType',version.type,
                    'requirementKind',requirement.requirement_kind) AS fact
                FROM published_curriculum_course_requirements requirement
                JOIN subject_versions version ON version.id=requirement.subject_version_id
                WHERE requirement.study_program_id=$1
                UNION ALL
                SELECT jsonb_build_object('requirementId',requirement.id,'kind','activity',
                    'catalogVersionId',requirement.activity_version_id,'subjectType',NULL,
                    'requirementKind',requirement.requirement_kind)
                FROM published_curriculum_activity_requirements requirement
                WHERE requirement.study_program_id=$1
            ) pinned",
        )
        .bind(program)
        .fetch_one(pool)
        .await
        .unwrap()
    }
    let pinned_before = pinned_classifications(&pool, context.study_program_id).await;
    // Current workspace readers require the supported current schema, including 151.
    school_migrations::run_tenant_migrations(&pool)
        .await
        .expect("the canonical upgrade must preserve historical classification input");
    assert_eq!(
        pinned_classifications(&pool, context.study_program_id).await,
        pinned_before,
        "published requirement identities, catalog pins and classifications survive upgrade"
    );
    let workspace = workspaces::homeroom_delivery_workspace(
        &pool,
        context.year_id,
        context.term_id,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let items = &workspace
        .homerooms
        .iter()
        .find(|room| room.homeroom.id == context.homeroom_id)
        .unwrap()
        .items;
    assert!(
        items
            .iter()
            .any(|item| item.subject_type.as_deref() == Some("BASIC")
                && item.requirement_kind
                    == school_academic_core::models::RequirementKind::Elective)
    );
    assert!(
        items
            .iter()
            .any(|item| item.subject_type.as_deref() == Some("ADDITIONAL")
                && item.requirement_kind
                    == school_academic_core::models::RequirementKind::Required)
    );
    assert!(
        items
            .iter()
            .any(|item| item.subject_type.as_deref() == Some("LEGACY-UNKNOWN"))
    );
    assert!(
        items
            .iter()
            .any(|item| item.resource_kind == LearningOfferingKind::Activity)
    );
    assert!(
        items
            .iter()
            .filter(|item| item.resource_kind == LearningOfferingKind::Activity)
            .all(|item| item.subject_type.is_none())
    );
}

#[tokio::test]
async fn homeroom_delivery_workspace_maps_curriculum_offerings_and_group_coverage() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_homeroom_workspace").await;
    let context = planning_runtime_context(&pool).await;
    let preview = offerings::preview_from_curriculum(
        &pool,
        PreviewCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
        },
    )
    .await
    .unwrap();
    let deferred_choices = preview
        .proposals
        .iter()
        .map(|proposal| CurriculumPreparationChoice {
            proposal_id: proposal.proposal_id.clone(),
            action: PreparationAction::DeferGroups,
            groups: Vec::new(),
        })
        .collect();
    let applied = offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],

            source_hash: preview.source_hash,
            idempotency_key: Uuid::new_v4(),
            choices: deferred_choices,
        },
    )
    .await
    .unwrap();
    assert!(applied.created_offering_count > 0);
    assert_eq!(applied.created_group_count, 0);
    assert!(applied.group_ids.is_empty());
    let deferred_group_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM learning_groups WHERE learning_offering_id = ANY($1)",
    )
    .bind(&applied.offering_ids)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(deferred_group_count, 0);
    let opening = current_workspace(&pool, context.year_id, context.term_id)
        .await
        .unwrap();
    assert!(
        applied
            .offering_ids
            .iter()
            .all(|id| opening.offerings.iter().any(|item| item.id == *id))
    );

    let filter = AcademicResourceListFilter {
        includes_school_owned: true,
        ..Default::default()
    };
    let before =
        workspaces::homeroom_delivery_workspace(&pool, context.year_id, context.term_id, &filter)
            .await
            .expect("homeroom workspace should load");
    let room = before
        .homerooms
        .iter()
        .find(|room| room.homeroom.id == context.homeroom_id)
        .expect("selected homeroom should appear");
    assert!(room.expected_count > 0);
    let central_count = room
        .items
        .iter()
        .filter(|item| item.group_mode == super::models::HomeroomGroupMode::Central)
        .count();
    assert!(central_count > 0);
    assert_eq!(room.ready_count, central_count);
    assert!(
        room.items
            .iter()
            .filter(|item| item.group_mode == super::models::HomeroomGroupMode::Central)
            .all(|item| item.teacher_state == super::models::HomeroomTeacherState::Deferred)
    );
    assert!(room.items.iter().all(|item| match item.resource_kind {
        LearningOfferingKind::Course => {
            item.standard_periods_per_week
                .is_some_and(|value| value > 0)
                && item.scheduling_mode.is_none()
                && item.subject_type.is_some()
        }
        LearningOfferingKind::Activity => {
            item.standard_periods_per_week.is_none()
                && item.scheduling_mode.is_some()
                && item.subject_type.is_none()
        }
    }));
    let offering_id = room
        .items
        .iter()
        .filter(|item| item.resource_kind == LearningOfferingKind::Course)
        .find_map(|item| item.offering_id)
        .expect("curriculum apply should create an applicable offering");

    let group = groups::create(
        &pool,
        context.teacher_id,
        offering_id,
        CreateLearningGroupRequest {
            registration_open: false,
            name: "กลุ่มห้องประจำชั้น".to_string(),
            description: None,
            capacity: Some(40),
            preferred_room_ids: Vec::new(),
        },
    )
    .await
    .unwrap();
    groups::replace_homerooms(
        &pool,
        context.teacher_id,
        group.id,
        ReplaceLearningGroupHomeroomsRequest {
            row_version: group.row_version,
            homeroom_ids: vec![context.homeroom_id],
        },
    )
    .await
    .unwrap();

    let after =
        workspaces::homeroom_delivery_workspace(&pool, context.year_id, context.term_id, &filter)
            .await
            .expect("homeroom workspace should refresh");
    let room = after
        .homerooms
        .iter()
        .find(|room| room.homeroom.id == context.homeroom_id)
        .unwrap();
    assert_eq!(room.ready_count, central_count + 1);
    assert!(
        room.items
            .iter()
            .any(|item| item.offering_id == Some(offering_id) && item.groups.len() == 1)
    );
}

#[tokio::test]
async fn homeroom_alignment_reports_missing_and_extra_delivery_per_room() {
    let pool =
        prepare_delivery_runtime_fixture("academic_delivery_workspace_alignment_states").await;
    let mut context = planning_runtime_context(&pool).await;
    context.subject_version_id = sqlx::query_scalar(
        r#"SELECT version.id
           FROM subject_versions version
           JOIN subjects subject ON subject.id = version.subject_id
           JOIN academic_terms term ON term.id = $1
           WHERE version.status = 'published'
             AND version.periods_per_week IS NOT NULL
             AND version.effective_from <= $4::date
             AND NOT EXISTS (
                 SELECT 1
                 FROM curriculum_course_requirements requirement
                 JOIN curriculum_term_slots slot ON slot.id = requirement.term_slot_id
                 WHERE requirement.study_program_id = $2
                   AND requirement.grade_level_id = $3
                   AND requirement.subject_version_id = version.id
                   AND slot.term_type = term.term_type
                   AND slot.type_occurrence = (
                       SELECT count(*)::integer
                       FROM academic_terms occurrence
                       WHERE occurrence.academic_year_id = term.academic_year_id
                         AND occurrence.term_type = term.term_type
                         AND occurrence.sequence_no <= term.sequence_no
                   )
             )
           ORDER BY subject.code, version.id
           LIMIT 1"#,
    )
    .bind(context.term_id)
    .bind(context.study_program_id)
    .bind(context.grade_level_id)
    .bind(runtime_operation_date())
    .fetch_one(&pool)
    .await
    .expect("fixture must expose a published catalog version outside this program term");
    let extra = offerings::create(&pool, context.teacher_id, course_request(&context))
        .await
        .unwrap();
    let workspace = workspaces::homeroom_delivery_workspace(
        &pool,
        context.year_id,
        context.term_id,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();
    let room = workspace
        .homerooms
        .iter()
        .find(|room| room.homeroom.id == context.homeroom_id)
        .expect("target homeroom should be present");
    assert!(room.items.iter().any(|item| {
        item.alignment_states
            .contains(&CurriculumDeliveryAlignmentState::CurriculumRequirementNotOffered)
    }));
    let extra_alignment = room
        .extra_offerings
        .iter()
        .find(|item| item.offering_id == extra.id)
        .expect("extra offering should be reported inside its targeted homeroom");
    assert_eq!(
        extra_alignment.alignment_states,
        vec![CurriculumDeliveryAlignmentState::ExtraOffering]
    );
}

#[tokio::test]
async fn delivery_management_options_are_scoped_and_human_readable() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_management_options").await;
    let mut context = planning_runtime_context(&pool).await;
    context.owner_id = sqlx::query_scalar(
        "SELECT subject.owning_organization_unit_id FROM subject_versions version JOIN subjects subject ON subject.id=version.subject_id WHERE version.id=$1",
    )
    .bind(context.subject_version_id)
    .fetch_one(&pool)
    .await
    .expect("selected catalog fixture must have an organization owner");
    let owned_catalog_ids: Vec<Uuid> = sqlx::query_scalar(
        "SELECT version.id FROM subject_versions version JOIN subjects subject ON subject.id=version.subject_id WHERE subject.owning_organization_unit_id=$1 UNION ALL SELECT version.id FROM activity_versions version JOIN activities activity ON activity.id=version.activity_id WHERE activity.owning_organization_unit_id=$1",
    )
    .bind(context.owner_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    let offering = offerings::create(&pool, context.teacher_id, course_request(&context))
        .await
        .expect("offering should be created for management options");
    let group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            registration_open: false,
            name: "กลุ่มตัวเลือกกลางภาค".to_string(),
            description: None,
            capacity: None,
            preferred_room_ids: Vec::new(),
        },
    )
    .await
    .expect("group should be created for management options");

    let options = workspaces::delivery_management_options(
        &pool,
        context.term_id,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .expect("management options should load");

    assert_eq!(options.academic_term_id, context.term_id);
    assert_eq!(options.academic_year_id, context.year_id);
    assert!(options.catalog_versions.iter().any(|item| {
        item.id == context.subject_version_id
            && item.label.contains(&item.code)
            && item.label.contains(&item.name)
            && item
                .standard_periods_per_week
                .is_some_and(|periods| periods > 0)
    }));
    assert!(
        options
            .catalog_versions
            .iter()
            .filter(|item| item.kind == LearningOfferingKind::Activity)
            .all(|item| item.standard_periods_per_week.is_none())
    );
    assert!(
        options
            .grade_levels
            .iter()
            .any(|item| item.id == context.grade_level_id && !item.name.is_empty())
    );
    assert!(
        options
            .study_programs
            .iter()
            .any(|item| item.id == context.study_program_id && !item.name.is_empty())
    );
    assert!(
        options
            .homerooms
            .iter()
            .any(|item| item.id == context.homeroom_id && item.grade_level.is_some())
    );
    assert!(
        options
            .teachers
            .iter()
            .any(|item| item.id == context.teacher_id && !item.name.trim().is_empty())
    );
    assert!(
        options
            .learning_groups
            .iter()
            .any(|item| item.id == group.id && item.learning_offering_id == offering.id)
    );
    assert!(options.rooms.iter().all(|item| !item.name_th.is_empty()));

    let scoped = workspaces::delivery_management_options(
        &pool,
        context.term_id,
        &AcademicResourceListFilter {
            organization_unit_ids: vec![context.owner_id],
            ..Default::default()
        },
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .expect("organization-scoped options should load");
    assert!(
        scoped
            .catalog_versions
            .iter()
            .all(|item| owned_catalog_ids.contains(&item.id))
    );
    assert!(
        scoped
            .catalog_versions
            .iter()
            .any(|item| item.id == context.subject_version_id)
    );
    assert!(
        scoped
            .study_programs
            .iter()
            .any(|item| item.id == context.study_program_id),
        "school curriculum choices must not be filtered by the offering owner's organization"
    );
    let without_curriculum = workspaces::delivery_management_options(
        &pool,
        context.term_id,
        &AcademicResourceListFilter {
            organization_unit_ids: vec![context.owner_id],
            ..Default::default()
        },
        &AcademicResourceListFilter::default(),
    )
    .await
    .expect("a limited offering manager can still load their non-curriculum options");
    assert!(
        without_curriculum.study_programs.is_empty(),
        "offering scope must not authorize school curriculum choices"
    );
    assert!(!without_curriculum.catalog_versions.is_empty());
    assert!(
        without_curriculum
            .catalog_versions
            .iter()
            .all(|item| owned_catalog_ids.contains(&item.id))
    );
    assert_eq!(
        without_curriculum
            .catalog_versions
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>(),
        scoped
            .catalog_versions
            .iter()
            .map(|item| item.id)
            .collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn roster_preview_exposes_minimal_display_data_without_hashing_names() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_roster_display").await;
    let context = planning_runtime_context(&pool).await;
    let offering = offerings::create(&pool, context.teacher_id, course_request(&context))
        .await
        .unwrap();
    let group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            registration_open: false,
            name: "กลุ่มตรวจข้อมูลรายชื่อ".to_string(),
            description: None,
            capacity: Some(40),
            preferred_room_ids: Vec::new(),
        },
    )
    .await
    .unwrap();
    let group = groups::replace_homerooms(
        &pool,
        context.teacher_id,
        group.id,
        ReplaceLearningGroupHomeroomsRequest {
            row_version: group.row_version,
            homeroom_ids: vec![context.homeroom_id],
        },
    )
    .await
    .unwrap();

    let preview = groups::preview_roster(&pool, group.id)
        .await
        .expect("preview should contain named students");
    let student = preview.students.first().expect("fixture roster student");
    assert!(!student.display_name.trim().is_empty());
    assert!(!student.grade_level_name.trim().is_empty());
    assert!(
        student
            .homeroom_name
            .as_deref()
            .is_some_and(|name| !name.is_empty())
    );
    let student_json = serde_json::to_value(student).expect("student should serialize");
    assert!(student_json.get("studentCode").is_some());
    for forbidden in [
        "nationalId",
        "nationalIdHash",
        "phone",
        "email",
        "guardian",
        "medical",
        "document",
    ] {
        assert!(student_json.get(forbidden).is_none());
    }

    sqlx::query("UPDATE users SET first_name = 'ชื่อใหม่', last_name = 'สำหรับทดสอบ' WHERE id = $1")
        .bind(student.student_id)
        .execute(&pool)
        .await
        .unwrap();
    let renamed = groups::preview_roster(&pool, group.id).await.unwrap();
    assert_eq!(renamed.source_hash, preview.source_hash);
    assert!(
        renamed
            .students
            .iter()
            .any(|item| item.display_name.contains("ชื่อใหม่"))
    );
}

#[tokio::test]
async fn list_groups_for_term_preserves_access_union_and_relations() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_term_group_list").await;
    let context = planning_runtime_context(&pool).await;
    let secondary_teacher_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM users WHERE user_type = 'staff' AND status = 'active' AND id <> $1 \
         ORDER BY id LIMIT 1",
    )
    .bind(context.teacher_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let existing_room_id: Uuid = sqlx::query_scalar("SELECT id FROM rooms ORDER BY id LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let secondary_room_id = Uuid::new_v4();
    sqlx::query(
        "INSERT INTO rooms (id, name_th, code, room_type, capacity, status) \
         VALUES ($1, 'ห้องทดสอบการโหลดแบบชุด', 'BATCH-ROOM', 'GENERAL', 40, 'ACTIVE')",
    )
    .bind(secondary_room_id)
    .execute(&pool)
    .await
    .unwrap();
    let room_ids = vec![existing_room_id, secondary_room_id];

    let course_offering = offerings::create(&pool, context.teacher_id, course_request(&context))
        .await
        .unwrap();
    let (activity_version_id, scheduling_mode): (Uuid, ActivitySchedulingMode) = sqlx::query_as(
        r#"SELECT version.id, version.scheduling_mode
           FROM activity_versions version
           JOIN academic_terms term ON term.id = $1
           WHERE version.status = 'published'
             AND version.effective_from <= $2::date
           ORDER BY version.id
           LIMIT 1"#,
    )
    .bind(context.term_id)
    .bind(runtime_operation_date())
    .fetch_one(&pool)
    .await
    .unwrap();
    let activity_offering = offerings::create(
        &pool,
        context.teacher_id,
        CreateLearningOfferingRequest::Activity(CreateActivityOfferingRequest {
            weekly_period_target: 1,

            create_homeroom_groups: false,
            academic_term_id: context.term_id,
            activity_version_id,
            curriculum_activity_requirement_id: None,
            targets: vec![OfferingTargetInput {
                target_kind: OfferingTargetKind::Homeroom,
                homeroom_id: Some(context.homeroom_id),
                grade_level_id: context.grade_level_id,
                study_program_id: context.study_program_id,
            }],
            registration_type: ActivityRegistrationType::Assigned,
            scheduling_mode,
            capacity: Some(40),
            attendance_requirement: ActivityAttendanceRequirement {
                minimum_percent: Some("80.00".to_string()),
                required_sessions: None,
            },
            pass_criteria: ActivityPassCriteria {
                require_attendance: true,
                require_teacher_confirmation: true,
                outcomes: vec!["pass".to_string(), "fail".to_string()],
            },
        }),
    )
    .await
    .unwrap();
    let secondary_owner_id = activity_offering.owning_organization_unit_id;

    let course_group_a = groups::create(
        &pool,
        context.teacher_id,
        course_offering.id,
        CreateLearningGroupRequest {
            registration_open: false,
            name: "กลุ่มชุดที่หนึ่ง".to_string(),
            description: None,
            capacity: Some(40),
            preferred_room_ids: vec![room_ids[0]],
        },
    )
    .await
    .unwrap();
    let course_group_a = groups::replace_teachers(
        &pool,
        context.teacher_id,
        course_group_a.id,
        ReplaceLearningGroupTeachersRequest {
            row_version: course_group_a.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: context.teacher_id,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap();
    let course_group_a = groups::replace_homerooms(
        &pool,
        context.teacher_id,
        course_group_a.id,
        ReplaceLearningGroupHomeroomsRequest {
            row_version: course_group_a.row_version,
            homeroom_ids: vec![context.homeroom_id],
        },
    )
    .await
    .unwrap();
    let course_group_b = groups::create(
        &pool,
        context.teacher_id,
        course_offering.id,
        CreateLearningGroupRequest {
            registration_open: false,
            name: "กลุ่มชุดที่สอง".to_string(),
            description: None,
            capacity: Some(40),
            preferred_room_ids: vec![room_ids[1]],
        },
    )
    .await
    .unwrap();
    let activity_group = groups::create(
        &pool,
        context.teacher_id,
        activity_offering.id,
        CreateLearningGroupRequest {
            registration_open: false,
            name: "กลุ่มกิจกรรม".to_string(),
            description: None,
            capacity: Some(40),
            preferred_room_ids: vec![room_ids[1]],
        },
    )
    .await
    .unwrap();
    let activity_group = groups::replace_teachers(
        &pool,
        context.teacher_id,
        activity_group.id,
        ReplaceLearningGroupTeachersRequest {
            row_version: activity_group.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: secondary_teacher_id,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap();
    let activity_group = groups::replace_homerooms(
        &pool,
        context.teacher_id,
        activity_group.id,
        ReplaceLearningGroupHomeroomsRequest {
            row_version: activity_group.row_version,
            homeroom_ids: vec![context.homeroom_id],
        },
    )
    .await
    .unwrap();

    let assigned = groups::list_for_term(
        &pool,
        context.term_id,
        &AcademicResourceListFilter {
            assigned_actor_id: Some(context.teacher_id),
            ..AcademicResourceListFilter::default()
        },
    )
    .await
    .unwrap();
    assert!(assigned.iter().any(|group| group.id == course_group_a.id));
    assert!(assigned.iter().any(|group| group.id == course_group_b.id));
    assert!(!assigned.iter().any(|group| group.id == activity_group.id));

    let organization_unit = groups::list_for_term(
        &pool,
        context.term_id,
        &AcademicResourceListFilter {
            organization_unit_ids: vec![secondary_owner_id],
            ..AcademicResourceListFilter::default()
        },
    )
    .await
    .unwrap();
    assert!(
        !organization_unit
            .iter()
            .any(|group| group.id == course_group_a.id)
    );
    assert!(
        organization_unit
            .iter()
            .any(|group| group.id == activity_group.id)
    );

    let organization_tree = groups::list_for_term(
        &pool,
        context.term_id,
        &AcademicResourceListFilter {
            organization_tree_unit_ids: vec![secondary_owner_id],
            ..AcademicResourceListFilter::default()
        },
    )
    .await
    .unwrap();
    let organization_unit_ids: Vec<Uuid> = organization_unit.iter().map(|group| group.id).collect();
    let organization_tree_ids: Vec<Uuid> = organization_tree.iter().map(|group| group.id).collect();
    assert_eq!(organization_tree_ids, organization_unit_ids);

    let union = groups::list_for_term(
        &pool,
        context.term_id,
        &AcademicResourceListFilter {
            assigned_actor_id: Some(context.teacher_id),
            organization_unit_ids: vec![secondary_owner_id],
            ..AcademicResourceListFilter::default()
        },
    )
    .await
    .unwrap();
    let mut expected_union_ids: Vec<Uuid> = assigned
        .iter()
        .chain(organization_unit.iter())
        .map(|group| group.id)
        .collect();
    expected_union_ids.sort_unstable();
    expected_union_ids.dedup();
    let mut union_ids: Vec<Uuid> = union.iter().map(|group| group.id).collect();
    union_ids.sort_unstable();
    assert_eq!(union_ids, expected_union_ids);

    let no_access = groups::list_for_term(
        &pool,
        context.term_id,
        &AcademicResourceListFilter::default(),
    )
    .await
    .unwrap();
    assert!(no_access.is_empty());

    let school = groups::list_for_term(
        &pool,
        context.term_id,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..AcademicResourceListFilter::default()
        },
    )
    .await
    .unwrap();
    let expected_school_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM learning_groups WHERE academic_term_id = $1")
            .bind(context.term_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(school.len() as i64, expected_school_count);

    let hydrated_course_group = school
        .iter()
        .find(|group| group.id == course_group_a.id)
        .unwrap();
    assert_eq!(hydrated_course_group.teacher_assignments.len(), 1);
    assert_eq!(
        hydrated_course_group.teacher_assignments[0].teacher_id,
        context.teacher_id
    );
    assert_eq!(
        hydrated_course_group.homeroom_ids,
        vec![context.homeroom_id]
    );
    assert_eq!(hydrated_course_group.preferred_room_ids, vec![room_ids[0]]);

    let hydrated_activity_group = school
        .iter()
        .find(|group| group.id == activity_group.id)
        .unwrap();
    assert_eq!(hydrated_activity_group.teacher_assignments.len(), 1);
    assert_eq!(
        hydrated_activity_group.teacher_assignments[0].teacher_id,
        secondary_teacher_id
    );
    assert_eq!(
        hydrated_activity_group.homeroom_ids,
        vec![context.homeroom_id]
    );
    assert_eq!(
        hydrated_activity_group.preferred_room_ids,
        vec![room_ids[1]]
    );

    let nested = groups::list(&pool, course_offering.id).await.unwrap();
    assert!(nested.iter().any(|group| group.id == course_group_a.id));
    assert!(nested.iter().any(|group| group.id == course_group_b.id));
    assert!(nested.iter().all(|group| {
        group.learning_offering_id == course_offering.id
            && group.academic_term_id == context.term_id
    }));
}

#[tokio::test]
async fn list_groups_for_term_rejects_unbounded_workspace() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_term_group_limit").await;
    let context = planning_runtime_context(&pool).await;
    let offering_id = offerings::create(&pool, context.teacher_id, course_request(&context))
        .await
        .unwrap()
        .id;

    sqlx::query(
        r#"INSERT INTO learning_groups (
               id, learning_offering_id, academic_term_id, academic_year_id,
               code, name, status, roster_status
           )
           SELECT gen_random_uuid(), $1, $2, $3,
                  'LIMIT-' || to_char(sequence, 'FM00000'),
                  'กลุ่มทดสอบขีดจำกัด ' || sequence::text,
                  'draft', 'draft'
           FROM generate_series(1, 2001) AS sequence"#,
    )
    .bind(offering_id)
    .bind(context.term_id)
    .bind(context.year_id)
    .execute(&pool)
    .await
    .unwrap();

    let error = groups::list_for_term(
        &pool,
        context.term_id,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..AcademicResourceListFilter::default()
        },
    )
    .await
    .expect_err("an oversized term workspace must fail instead of truncating");

    assert!(matches!(error, AppError::ValidationError(message) if message.contains("2000")));
}

#[tokio::test]
async fn manual_activity_activation_creates_reviewed_groups_atomically_or_stays_central() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_manual_activity_groups").await;
    let context = planning_runtime_context(&pool).await;
    let activity_version_id: Uuid = sqlx::query_scalar("SELECT version.id FROM activity_versions version JOIN academic_terms term ON term.id=$1 WHERE version.status='published' AND version.scheduling_mode='synchronized' AND version.effective_from <= $2::date ORDER BY version.id LIMIT 1")
        .bind(context.term_id).bind(runtime_operation_date())
    .fetch_one(&pool).await.unwrap();
    let request = CreateActivityOfferingRequest {
        weekly_period_target: 1,

        create_homeroom_groups: false,
        academic_term_id: context.term_id,
        activity_version_id,
        curriculum_activity_requirement_id: None,
        targets: vec![OfferingTargetInput {
            target_kind: OfferingTargetKind::Homeroom,
            homeroom_id: Some(context.homeroom_id),
            grade_level_id: context.grade_level_id,
            study_program_id: context.study_program_id,
        }],
        registration_type: ActivityRegistrationType::Assigned,
        scheduling_mode: ActivitySchedulingMode::Synchronized,
        capacity: None,
        attendance_requirement: ActivityAttendanceRequirement {
            minimum_percent: Some("80.00".into()),
            required_sessions: None,
        },
        pass_criteria: ActivityPassCriteria {
            require_attendance: true,
            require_teacher_confirmation: true,
            outcomes: vec!["pass".into(), "fail".into()],
        },
    };
    let central = offerings::create(
        &pool,
        context.teacher_id,
        CreateLearningOfferingRequest::Activity(request.clone()),
    )
    .await
    .unwrap();
    assert!(groups::list(&pool, central.id).await.unwrap().is_empty());
    let mut grouped = request.clone();
    grouped.create_homeroom_groups = true;
    let grouped = offerings::create(
        &pool,
        context.teacher_id,
        CreateLearningOfferingRequest::Activity(grouped),
    )
    .await
    .unwrap();
    let created = groups::list(&pool, grouped.id).await.unwrap();
    assert_eq!(created.len(), 1);
    assert_eq!(created[0].homeroom_ids, vec![context.homeroom_id]);
    assert!(created[0].teacher_assignments.is_empty());
    assert!(!created[0].name.trim().is_empty());
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM learning_offerings")
        .fetch_one(&pool)
        .await
        .unwrap();
    let mut invalid = request;
    invalid.create_homeroom_groups = true;
    invalid.targets[0].homeroom_id = Some(Uuid::new_v4());
    assert!(
        offerings::create(
            &pool,
            context.teacher_id,
            CreateLearningOfferingRequest::Activity(invalid)
        )
        .await
        .is_err()
    );
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM learning_offerings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, after);
}

#[path = "roster_tracking_tests.rs"]
mod roster_tracking_tests;

async fn zero_workload_subject(
    pool: &PgPool,
    context: &RuntimeContext,
) -> school_academic_core::models::SubjectVersion {
    use school_academic_core::{models as core_models, services::catalog};
    let group: Uuid = sqlx::query_scalar("SELECT g.id FROM subject_groups g JOIN organization_units u ON u.subject_group_id=g.id WHERE g.is_active AND u.is_active ORDER BY g.id LIMIT 1")
        .fetch_one(pool).await.unwrap();
    let start = runtime_operation_date();
    let stable = catalog::create_subject(
        pool,
        core_models::CreateCatalogSubjectRequest {
            code: "ZERO-OPENING".into(),
            subject_group_id: group,
        },
    )
    .await
    .unwrap();
    let version = catalog::create_subject_version(
        pool,
        stable.id,
        core_models::CreateSubjectVersionRequest {
            name_th: "รายวิชาศูนย์ชั่วโมงและคาบ".into(),
            name_en: None,
            credit: "1.00".into(),
            hours_per_semester: Some(0),
            periods_per_week: Some(0),
            subject_type: "BASIC".into(),
            description: None,
            effective_from: start,
            term_code: None,
            grade_level_ids: vec![context.grade_level_id],
        },
    )
    .await
    .unwrap();
    catalog::publish_subject_version(
        pool,
        version.id,
        core_models::PublishVersionRequest {
            row_version: version.row_version,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn subject_zero_workload_manual_opening_keeps_course_and_activity_zero_targets() {
    use school_academic_core::{models as core_models, services::catalog};
    let pool = prepare_delivery_runtime_fixture("subject_zero_workload_manual").await;
    let context = planning_runtime_context(&pool).await;
    let version = zero_workload_subject(&pool, &context).await;
    let mut request = course_request(&context);
    let CreateLearningOfferingRequest::Course(course) = &mut request else {
        panic!("course fixture");
    };
    course.subject_version_id = version.id;
    course.weekly_period_target = 0;
    let offering = offerings::create(&pool, context.teacher_id, request)
        .await
        .unwrap();
    let draft = current_workspace(&pool, context.year_id, context.term_id)
        .await
        .unwrap();
    let captured = draft
        .offerings
        .iter()
        .find(|item| item.id == offering.id)
        .unwrap();
    assert_eq!(captured.weekly_period_target, 0);
    let LearningOfferingSnapshot::Course(course) = &captured.catalog else {
        panic!("course snapshot");
    };
    assert_eq!(course.standard_periods_per_week, 0);
    assert_eq!(course.hours.as_deref(), Some("0.00"));
    assert_eq!(course.credit, "1.00");

    for (catalog_mode, scheduling_mode) in [
        ("independent", ActivitySchedulingMode::Independent),
        ("synchronized", ActivitySchedulingMode::Synchronized),
    ] {
        // Explicit legacy actual targets may be zero; curriculum hours do not infer that target.
        let activity = catalog::create_activity(
            &pool,
            core_models::CreateCatalogActivityRequest {
                activity_type: "guidance".into(),
            },
        )
        .await
        .unwrap();
        let activity = catalog::create_activity_version(
            &pool,
            activity.id,
            core_models::CreateActivityVersionRequest {
                name: "กิจกรรมตรวจข้อจำกัดคาบจริงศูนย์".into(),
                description: None,
                hours_per_week: "0.00".into(),
                hours_per_term: Some("0.00".into()),
                scheduling_mode: catalog_mode.into(),
                effective_from: version.effective_from,
                term_code: None,
                grade_level_ids: vec![context.grade_level_id],
            },
        )
        .await
        .unwrap();
        sqlx::query("UPDATE activity_versions SET periods_per_week=0 WHERE id=$1")
            .bind(activity.id)
            .execute(&pool)
            .await
            .unwrap();
        let activity = catalog::publish_activity_version(
            &pool,
            activity.id,
            core_models::PublishVersionRequest {
                row_version: activity.row_version,
            },
        )
        .await
        .unwrap();
        let before: i64 = sqlx::query_scalar("SELECT count(*) FROM learning_offerings")
            .fetch_one(&pool)
            .await
            .unwrap();
        let included = offerings::create(
            &pool,
            context.teacher_id,
            CreateLearningOfferingRequest::Activity(CreateActivityOfferingRequest {
                weekly_period_target: 0,

                create_homeroom_groups: false,
                academic_term_id: context.term_id,
                activity_version_id: activity.id,
                curriculum_activity_requirement_id: None,
                targets: vec![OfferingTargetInput {
                    target_kind: OfferingTargetKind::Homeroom,
                    homeroom_id: Some(context.homeroom_id),
                    grade_level_id: context.grade_level_id,
                    study_program_id: context.study_program_id,
                }],
                registration_type: ActivityRegistrationType::Assigned,
                scheduling_mode,
                capacity: None,
                attendance_requirement: ActivityAttendanceRequirement {
                    minimum_percent: None,
                    required_sessions: None,
                },
                pass_criteria: ActivityPassCriteria {
                    require_attendance: false,
                    require_teacher_confirmation: true,
                    outcomes: vec!["pass".into(), "fail".into()],
                },
            }),
        )
        .await
        .unwrap();
        let snapshot = current_workspace(&pool, context.year_id, context.term_id)
            .await
            .unwrap();
        assert_eq!(
            snapshot
                .offerings
                .iter()
                .find(|offering| offering.id == included.id)
                .unwrap()
                .weekly_period_target,
            0
        );
        let after: i64 = sqlx::query_scalar("SELECT count(*) FROM learning_offerings")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(before + 1, after);
    }
}

#[tokio::test]
async fn subject_zero_workload_curriculum_generated_opening_keeps_zero() {
    use school_academic_core::{
        models as core_models,
        services::{curriculum, curriculum_structure},
    };
    let pool = prepare_delivery_runtime_fixture("subject_zero_workload_generated").await;
    let context = planning_runtime_context(&pool).await;
    let version = zero_workload_subject(&pool, &context).await;
    let edition = curriculum::create(
        &pool,
        core_models::CreateCurriculumRequest {
            name: "หลักสูตรเปิดรายวิชาศูนย์คาบ".into(),
            revision_year: 2569,
            description: None,
        },
    )
    .await
    .unwrap();
    let token = edition.draft_id.unwrap();
    let level = curriculum::create_level(
        &pool,
        edition.id,
        core_models::CreateCurriculumLevelRequest {
            draft_id: token,
            name_th: "ระดับทดสอบ".into(),
            grade_level_ids: vec![context.grade_level_id],
            description: None,
        },
    )
    .await
    .unwrap();
    let program = curriculum::create_program(
        &pool,
        level.id,
        core_models::CreateStudyProgramRequest {
            draft_id: token,
            name_th: "แผนศูนย์คาบ".into(),
        },
    )
    .await
    .unwrap();
    let workspace = curriculum_structure::get_workspace(&pool, level.id)
        .await
        .unwrap();
    curriculum_structure::replace_program_structure(
        &pool,
        program.id,
        core_models::ReplaceCurriculumStructureRequest {
            draft_id: token,
            row_version: program.row_version,
            requirements: vec![core_models::CurriculumStructureRequirementInput {
                resource_kind: core_models::RequirementResourceKind::Course,
                catalog_version_id: version.id,
                grade_level_id: context.grade_level_id,
                term_slot_id: workspace.term_slots[0].id,
                requirement_kind: core_models::RequirementKind::Required,
                display_order: 1,
            }],
        },
    )
    .await
    .unwrap();
    let edition = curriculum::get(&pool, edition.id).await.unwrap();
    curriculum::publish(
        &pool,
        edition.id,
        core_models::PublishCurriculumRequest {
            draft_id: token,
            change_note: "เพิ่มรายวิชาศูนย์คาบ".into(),
            row_version: edition.row_version,
        },
        context.teacher_id,
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO homerooms(id,code,name,academic_year_id,grade_level_id,study_program_id,capacity,is_active) VALUES(gen_random_uuid(),'ZERO-GEN','ห้องศูนย์คาบ',$1,$2,$3,40,true)")
        .bind(context.year_id).bind(context.grade_level_id).bind(program.id).execute(&pool).await.unwrap();
    let preview = offerings::preview_from_curriculum(
        &pool,
        PreviewCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![program.id],
        },
    )
    .await
    .unwrap();
    assert_eq!(preview.proposals.len(), 1);
    assert_eq!(preview.proposals[0].hours.as_deref(), Some("0"));
    let choices = default_preparation_choices(&preview);
    let applied = offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![program.id],

            source_hash: preview.source_hash,
            idempotency_key: Uuid::new_v4(),
            choices,
        },
    )
    .await
    .unwrap();
    assert_eq!(applied.created_offering_count, 1);
    let draft = current_workspace(&pool, context.year_id, context.term_id)
        .await
        .unwrap();
    let captured = draft
        .offerings
        .iter()
        .find(|item| applied.offering_ids.contains(&item.id))
        .unwrap();
    assert_eq!(captured.weekly_period_target, 0);
    let LearningOfferingSnapshot::Course(course) = &captured.catalog else {
        panic!("course snapshot");
    };
    assert_eq!(course.standard_periods_per_week, 0);
    assert_eq!(course.hours.as_deref(), Some("0.00"));
    assert_eq!(course.credit, "1.00");
    assert!(course.curriculum_course_requirement_id.is_some());
}

#[path = "bulk_group_setup_tests.rs"]
mod bulk_group_setup_tests;

#[path = "preparation_target_tests.rs"]
mod preparation_target_tests;

async fn current_workspace(
    pool: &PgPool,
    year: Uuid,
    term: Uuid,
) -> Result<school_academic_delivery::models::current::DeliveryWorkspace, AppError> {
    let mut connection = pool.acquire().await?;
    current::workspace_on_connection(&mut connection, year, term).await
}

#[path = "composed_deletion_tests.rs"]
mod composed_deletion_tests;
