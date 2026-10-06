use crate::modules::academic::{
    cutover_test_preflight::run_academic_core_preflight,
    cutover_test_support::{
        apply_migrations_through, apply_phase_b_runtime_migrations, seed_academic_cutover_fixture,
        CutoverFixture,
    },
    models::{
        timetable_block::CreateOrdinaryTimetableBlockRequest,
        timetable_version::CloneTimetableVersionRequest,
    },
    services::{timetable_block_service, timetable_version_service},
};
use chrono::{Duration, NaiveDate, Utc};
use school_test_db::{create_named_test_pool, create_named_test_pool_with_max_connections};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::{
    models::{
        AcademicChangeFindingCode, AcademicChangeFindingSeverity, AcademicChangeImpactCounts,
        AcademicTermChangeSetStatus, ActivityAttendanceRequirement, ActivityPassCriteria,
        ActivityRegistrationType, ActivitySchedulingMode, AddDatedRosterMembershipRequest,
        ApplyCurriculumOfferingsRequest, ApplyRosterRequest, ApplyTeacherHandoffRequest,
        CreateAcademicTermChangeSetRequest, CreateActivityOfferingRequest,
        CreateCourseOfferingRequest, CreateLearningGroupRequest, CreateLearningOfferingRequest,
        CurriculumDeliveryAlignmentState, CurriculumOfferingPreview, CurriculumPreparationChoice,
        DeleteAcademicTermChangeItemRequest, LearningOfferingKind, LearningOfferingQuery,
        LearningOfferingSnapshot, LearningOfferingStatus, LearningTeacherRole, OfferingTargetInput,
        OfferingTargetKind, PreparationAction, PreparationGroupingState,
        PreviewCurriculumOfferingsRequest, PreviewTeacherHandoffRequest,
        PublishAcademicTermChangeSetRequest, PublishLearningOfferingRequest, PublishRosterRequest,
        RemoveDatedRosterMembershipRequest, ReplaceLearningGroupHomeroomsRequest,
        ReplaceLearningGroupTeachersRequest, RosterOverrideAction, RosterOverrideInput,
        StudentActivityRegistrationQuery, TeacherAssignmentInput, TeacherHandoffEntryVersion,
        TeacherHandoffMode, UpdateAcademicTermChangeSetRequest, UpdateLearningOfferingRequest,
        UpsertAcademicTermChangeItemRequest,
    },
    services::{
        activities, change_sets, groups, offerings, roster_memberships, teacher_handoff, workspaces,
    },
};
use school_authorization::AcademicResourceListFilter;
use school_errors::AppError;

const ACADEMIC_CORE_NAMESPACE: Uuid = Uuid::from_u128(0x5c33_b984_10df_58db_bf80_62db_c4a0_3d1b);

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
    assert!(duplicate_placement
        .to_string()
        .contains("homeroom_placements_one_current_key"));

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
    assert!(duplicate_student
        .to_string()
        .contains("learning_group_students_one_active_key"));

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
    assert!(cross_year_homeroom
        .to_string()
        .contains("learning_group_homerooms_homeroom_context_fkey"));

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
    assert!(mismatched_group
        .to_string()
        .contains("learning_groups_offering_context_fkey"));

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
    assert!(published_snapshot_error
        .to_string()
        .contains("ACADEMIC_CORE_PUBLISHED_OFFERING_IMMUTABLE"));
}

async fn prepare_delivery_runtime_fixture(name: &str) -> PgPool {
    let pool = create_named_test_pool(name).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, 94).await.unwrap();
    pool
}

async fn prepare_concurrent_delivery_runtime_fixture(name: &str) -> PgPool {
    let pool = create_named_test_pool_with_max_connections(name, 5).await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
    apply_migrations_through(&pool, 94).await.unwrap();
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
    sqlx::query("UPDATE academic_terms SET status='closed',closed_on=COALESCE(planned_end_date,start_date) WHERE id<>$1 AND status IN ('active','closing')").bind(context.term_id).execute(&pool).await.unwrap();
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
        sqlx::query("UPDATE academic_terms SET status=$2, closed_on=CASE WHEN $2='closed' THEN start_date ELSE NULL END WHERE id=$1")
            .bind(context.term_id).bind(term_status).execute(&pool).await.unwrap();
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
                _ => offerings::publish(
                    &worker_pool,
                    actor,
                    offering.id,
                    PublishLearningOfferingRequest {
                        row_version: offering.row_version,
                        idempotency_key: Uuid::new_v4(),
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
        sqlx::query("UPDATE academic_terms SET status=$2,closed_on=CASE WHEN $2='closed' THEN start_date ELSE NULL END WHERE id=$1")
            .bind(context.term_id).bind(term_status).execute(&pool).await.unwrap();
        let outcomes = [
            offerings::create(&pool, context.teacher_id, course_request(&context))
                .await
                .map(|_| ()),
            offerings::update(
                &pool,
                context.teacher_id,
                offering.id,
                UpdateLearningOfferingRequest {
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
            offerings::publish(
                &pool,
                context.teacher_id,
                offering.id,
                PublishLearningOfferingRequest {
                    row_version: offering.row_version,
                    idempotency_key: Uuid::new_v4(),
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

async fn wait_for_offering_lock_waiters(pool: &PgPool, expected: i64) {
    for _ in 0..200 {
        let waiting: i64 = sqlx::query_scalar(
            r#"SELECT count(*)::bigint
               FROM pg_stat_activity
               WHERE datname = current_database()
                 AND wait_event_type = 'Lock'
                 AND query LIKE '%learning_offerings%'
                 AND query LIKE '%FOR %'"#,
        )
        .fetch_one(pool)
        .await
        .unwrap();
        if waiting >= expected {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("expected {expected} delivery transactions to wait on an offering lock");
}

#[tokio::test]
async fn group_read_model_returns_effective_teacher_episodes() {
    let pool = prepare_delivery_runtime_fixture("academic_group_teacher_episode_read").await;
    let (offering_id, term_id, year_id, starts_on, teacher_id, display_name): (
        Uuid,
        Uuid,
        Uuid,
        NaiveDate,
        Uuid,
        String,
    ) = sqlx::query_as(
        r#"SELECT offering.id, offering.academic_term_id, offering.academic_year_id,
                  offering.starts_on, teacher.teacher_id,
                  concat_ws(' ',
                      nullif(concat(coalesce(user_account.title, ''), user_account.first_name), ''),
                      nullif(user_account.last_name, '')) AS display_name
           FROM learning_offerings offering
           JOIN learning_groups learning_group
             ON learning_group.learning_offering_id = offering.id
           JOIN learning_group_teachers teacher
             ON teacher.learning_group_id = learning_group.id
           JOIN users user_account ON user_account.id = teacher.teacher_id
           WHERE teacher.role = 'primary'
           ORDER BY offering.id, teacher.starts_on, teacher.id
           LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .expect("fixture must contain a migrated teacher episode");

    let group_id = Uuid::new_v4();
    let assignment_id = Uuid::new_v4();
    let second_teacher_id = Uuid::new_v4();
    let third_teacher_id = Uuid::new_v4();
    let fourth_teacher_id = Uuid::new_v4();
    let second_assignment_id = Uuid::parse_str("f1000000-0000-0000-0000-000000000001").unwrap();
    let third_assignment_id = Uuid::parse_str("f1000000-0000-0000-0000-000000000003").unwrap();
    let fourth_assignment_id = Uuid::parse_str("f1000000-0000-0000-0000-000000000002").unwrap();
    let actor_user_id = Uuid::parse_str("50000000-0000-0000-0000-000000000002").unwrap();
    let early_secondary_starts_on = starts_on.succ_opt().unwrap();
    let late_secondary_starts_on = early_secondary_starts_on.succ_opt().unwrap();
    sqlx::query(
        r#"INSERT INTO learning_groups (
               id, learning_offering_id, academic_term_id, academic_year_id,
               code, name, status, roster_status
           ) VALUES ($1, $2, $3, $4, $5, 'กลุ่มทดสอบ episode ครู', 'draft', 'draft')"#,
    )
    .bind(group_id)
    .bind(offering_id)
    .bind(term_id)
    .bind(year_id)
    .bind(format!("EPISODE-{}", &group_id.to_string()[..8]))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO users (
               id, email, username, password_hash, first_name, last_name,
               user_type, status
           ) VALUES
               ($1, $4, $7, 'fixture-not-a-login', 'ครูรองหนึ่ง', 'ทดสอบ', 'staff', 'active'),
               ($2, $5, $8, 'fixture-not-a-login', 'ครูรองสอง', 'ทดสอบ', 'staff', 'active'),
               ($3, $6, $9, 'fixture-not-a-login', 'ครูรองสาม', 'ทดสอบ', 'staff', 'active')"#,
    )
    .bind(second_teacher_id)
    .bind(third_teacher_id)
    .bind(fourth_teacher_id)
    .bind(format!("{second_teacher_id}@example.invalid"))
    .bind(format!("{third_teacher_id}@example.invalid"))
    .bind(format!("{fourth_teacher_id}@example.invalid"))
    .bind(format!("teacher-{second_teacher_id}"))
    .bind(format!("teacher-{third_teacher_id}"))
    .bind(format!("teacher-{fourth_teacher_id}"))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO learning_group_teachers (
               id, learning_group_id, academic_term_id, academic_year_id,
               teacher_id, role, starts_on, created_by, updated_by
           ) VALUES
               ($1, $5, $6, $7, $8, 'primary', $12, $13, $13),
               ($2, $5, $6, $7, $9, 'secondary', $14, $13, $13),
               ($3, $5, $6, $7, $10, 'secondary', $15, $13, $13),
               ($4, $5, $6, $7, $11, 'secondary', $15, $13, $13)"#,
    )
    .bind(assignment_id)
    .bind(second_assignment_id)
    .bind(third_assignment_id)
    .bind(fourth_assignment_id)
    .bind(group_id)
    .bind(term_id)
    .bind(year_id)
    .bind(teacher_id)
    .bind(second_teacher_id)
    .bind(third_teacher_id)
    .bind(fourth_teacher_id)
    .bind(starts_on)
    .bind(actor_user_id)
    .bind(late_secondary_starts_on)
    .bind(early_secondary_starts_on)
    .execute(&pool)
    .await
    .unwrap();

    let group = groups::get(&pool, group_id).await.unwrap();
    let assignment = group
        .teacher_assignments
        .iter()
        .find(|assignment| assignment.id == assignment_id)
        .expect("group read model must expose the assignment identity");
    assert_eq!(assignment.teacher_id, teacher_id);
    assert_eq!(assignment.display_name, display_name);
    assert_eq!(assignment.role, LearningTeacherRole::Primary);
    assert_eq!(assignment.starts_on, starts_on);
    assert_eq!(assignment.ends_on, None);
    assert_eq!(assignment.row_version, 1);
    let second_assignment = group
        .teacher_assignments
        .iter()
        .find(|assignment| assignment.id == second_assignment_id)
        .expect("group read model must expose every teacher episode");
    assert_eq!(second_assignment.teacher_id, second_teacher_id);
    assert_eq!(second_assignment.role, LearningTeacherRole::Secondary);
    assert_eq!(second_assignment.starts_on, late_secondary_starts_on);
    assert_eq!(second_assignment.ends_on, None);
    assert_eq!(second_assignment.row_version, 1);
    assert_eq!(
        group
            .teacher_assignments
            .iter()
            .map(|assignment| assignment.id)
            .collect::<Vec<_>>(),
        vec![
            assignment_id,
            fourth_assignment_id,
            third_assignment_id,
            second_assignment_id
        ]
    );
    let primary_position = group
        .teacher_assignments
        .iter()
        .position(|assignment| assignment.id == assignment_id)
        .unwrap();
    let secondary_position = group
        .teacher_assignments
        .iter()
        .position(|assignment| assignment.id == second_assignment_id)
        .unwrap();
    assert!(primary_position < secondary_position);
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
           ORDER BY term.start_date
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
             AND version.effective_from <= term.start_date
             AND (version.effective_until IS NULL OR version.effective_until > term.start_date)
           ORDER BY version.id
           LIMIT 1"#,
    )
    .bind(term_id)
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
    let (term_id, fixture_future_start, base_version_id): (Uuid, NaiveDate, Uuid) = sqlx::query_as(
        r#"SELECT term.id, year.end_date, version.id
           FROM academic_terms term
           JOIN academic_years year ON year.id = term.academic_year_id
           JOIN academic_timetable_versions version
             ON version.academic_term_id = term.id
            AND version.status = 'published'
           WHERE term.status = 'active'
           ORDER BY term.start_date, term.id, version.effective_from DESC, version.id
           LIMIT 1"#,
    )
    .fetch_one(pool)
    .await
    .expect("fixture must contain an active term with a published timetable base");
    sqlx::query(
        r#"UPDATE learning_offerings offering
           SET starts_on = $3,
               updated_at = now()
           WHERE offering.academic_term_id = $1
             AND offering.status IN ('draft', 'published')
             AND NOT EXISTS (
                 SELECT 1
                 FROM academic_timetable_versions version
                 JOIN academic_delivery_versions opening ON opening.id=version.delivery_version_id
                 CROSS JOIN LATERAL jsonb_array_elements(opening.snapshot->'offerings') entry
                 WHERE version.id=$2 AND (entry->>'id')::uuid=offering.id
             )"#,
    )
    .bind(term_id)
    .bind(base_version_id)
    .bind(fixture_future_start)
    .execute(pool)
    .await
    .expect("operational change fixture must defer offerings absent from its published baseline");
    sqlx::query("UPDATE academic_terms SET status = 'planning' WHERE id = $1")
        .bind(term_id)
        .execute(pool)
        .await
        .unwrap();
    let context = planning_runtime_context(pool).await;
    assert_eq!(context.term_id, term_id);
    context
}

async fn create_runtime_change_set(
    pool: &PgPool,
    actor_id: Uuid,
    term_id: Uuid,
    offset_days: i64,
    idempotency_name: &str,
) -> super::models::AcademicTermChangeSet {
    let term_start: NaiveDate =
        sqlx::query_scalar("SELECT start_date FROM academic_terms WHERE id = $1")
            .bind(term_id)
            .fetch_one(pool)
            .await
            .unwrap();
    let created = change_sets::create_change_set(
        pool,
        actor_id,
        CreateAcademicTermChangeSetRequest {
            academic_term_id: term_id,
            reason: "  ปรับการเปิดสอนระหว่างภาคเรียน  ".to_string(),
            idempotency_key: stable_uuid(idempotency_name),
        },
    )
    .await
    .expect("a planning term must accept a draft operational change");
    let date = term_start + chrono::Duration::days(offset_days);
    if created.reference_date == date {
        return created;
    }
    change_sets::update_change_set(
        pool,
        actor_id,
        created.id,
        UpdateAcademicTermChangeSetRequest {
            row_version: created.row_version,
            reference_date: date,
            reason: created.reason.clone(),
        },
    )
    .await
    .unwrap()
}

async fn independent_table_draft(
    pool: &PgPool,
    context: &RuntimeContext,
) -> school_academic_timetable::models::timetable_version::TimetableVersion {
    let source_id: Uuid=sqlx::query_scalar("SELECT id FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC,id LIMIT 1")
        .bind(context.term_id).fetch_one(pool).await.unwrap();
    let source = timetable_version_service::get_version(pool, source_id, Utc::now().date_naive())
        .await
        .unwrap();
    timetable_version_service::clone_draft(
        pool,
        context.teacher_id,
        source_id,
        CloneTimetableVersionRequest {
            source_row_version: source.row_version,
            resume_draft_id: None,
            draft_row_version: None,
        },
    )
    .await
    .unwrap()
}

async fn table_preview(
    pool: &PgPool,
    draft_id: Uuid,
) -> school_academic_timetable::models::timetable_publication::TimetablePublicationPreview {
    let table = timetable_version_service::get_version(pool, draft_id, Utc::now().date_naive())
        .await
        .unwrap();
    let opening =
        school_academic_delivery::services::versions::get_version(pool, table.delivery_version_id)
            .await
            .unwrap();
    school_academic_timetable::services::timetable_lifecycle::preview(pool,draft_id,school_academic_timetable::models::timetable_publication::PreviewTimetablePublicationRequest {
        row_version:table.row_version,effective_from:opening.effective_from.unwrap().succ_opt().unwrap()
    }).await.unwrap()
}

async fn fill_change_set_target_deficits(
    pool: &PgPool,
    actor_user_id: Uuid,
    academic_term_id: Uuid,
    timetable_version_id: Uuid,
) {
    let table =
        timetable_version_service::get_version(pool, timetable_version_id, Utc::now().date_naive())
            .await
            .unwrap();
    let opening =
        school_academic_delivery::services::versions::get_version(pool, table.delivery_version_id)
            .await
            .unwrap();
    for offering in &opening.snapshot.offerings {
        if matches!(&offering.catalog,LearningOfferingSnapshot::Activity(activity) if activity.scheduling_mode==ActivitySchedulingMode::Synchronized)
        {
            let placed_count: i64=sqlx::query_scalar("SELECT count(*) FROM academic_timetable_blocks WHERE timetable_version_id=$1 AND learning_offering_id=$2 AND is_active").bind(timetable_version_id).bind(offering.id).fetch_one(pool).await.unwrap();
            for _ in placed_count..i64::from(offering.weekly_period_target) {
                let slots: Vec<(String,Uuid)>=sqlx::query_as("SELECT day.code,period.id FROM academic_terms term JOIN bell_schedule_periods period ON period.bell_schedule_id=term.bell_schedule_id CROSS JOIN (VALUES('MON'),('TUE'),('WED'),('THU'),('FRI')) day(code) WHERE term.id=$1 AND period.is_active ORDER BY day.code,period.order_index").bind(academic_term_id).fetch_all(pool).await.unwrap();
                let mut placed = false;
                for (day_of_week, bell_schedule_period_id) in slots {
                    match timetable_block_service::create_synchronized_block(pool,actor_user_id,school_academic_timetable::models::timetable_block::CreateSynchronizedTimetableBlockRequest {
                    timetable_version_id,academic_term_id,learning_offering_id:offering.id,day_of_week,bell_schedule_period_id,intended_homeroom_ids:offering.homeroom_ids.clone(),teacher_ids:vec![],room_id:None,note:None
                }).await {
                    Ok(_) => {placed=true;break;},Err(AppError::Conflict(_))=>{},Err(error)=>panic!("unexpected activity placement error: {error:?}")
                }
                }
                assert!(placed, "fixture requires a conflict-free activity slot");
            }
            continue;
        }
        for group in &offering.groups {
            let group_id = group.id;
            let placed_count: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_timetable_block_groups placed JOIN academic_timetable_blocks block ON block.id=placed.block_id WHERE block.timetable_version_id=$1 AND placed.learning_group_id=$2 AND placed.is_active AND block.is_active")
            .bind(timetable_version_id).bind(group_id).fetch_one(pool).await.unwrap();
            let missing_count = i64::from(offering.weekly_period_target) - placed_count;
            let instructor_ids = group
                .teachers
                .iter()
                .map(|teacher| teacher.teacher_id)
                .collect::<Vec<_>>();
            for _ in 0..missing_count {
                let slots: Vec<(String, Uuid)> = sqlx::query_as(
                    r#"SELECT day.code, period.id
                   FROM (VALUES ('MON'), ('TUE'), ('WED'), ('THU'), ('FRI')) AS day(code)
                   JOIN academic_terms term ON term.id = $1
                   JOIN bell_schedule_periods period
                     ON period.bell_schedule_id = term.bell_schedule_id
                    AND period.is_active
                   ORDER BY day.code, period.order_index
                   LIMIT 100"#,
                )
                .bind(academic_term_id)
                .fetch_all(pool)
                .await
                .expect("fixture must leave enough empty timetable slots");
                let mut placed = false;
                for (day_of_week, bell_schedule_period_id) in slots {
                    let result = timetable_block_service::create_ordinary_block(
                        pool,
                        actor_user_id,
                        CreateOrdinaryTimetableBlockRequest {
                            timetable_version_id,
                            academic_term_id,
                            learning_group_id: group_id,
                            day_of_week,
                            bell_schedule_period_id,
                            room_id: None,
                            note: Some("เติมคาบสำหรับทดสอบ readiness".to_string()),
                            instructor_ids: instructor_ids.clone(),
                        },
                    )
                    .await;
                    if result.is_ok() {
                        placed = true;
                        break;
                    }
                }
                assert!(placed, "fixture must leave a valid timetable slot");
            }
        }
    }
}

#[tokio::test]
async fn teacher_change_items_support_add_adjust_stop_and_delete() {
    let pool = prepare_delivery_runtime_fixture("academic_teacher_change_items").await;
    let context = operational_change_runtime_context(&pool).await;
    let change_set = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        14,
        "teacher-change-items:create",
    )
    .await;
    let (group_id, assignment_id, current_teacher_id): (Uuid, Uuid, Uuid) = sqlx::query_as(
        r#"SELECT learning_group.id, teacher.id, teacher.teacher_id
           FROM learning_groups learning_group
           JOIN learning_group_teachers teacher
             ON teacher.learning_group_id = learning_group.id
           WHERE learning_group.academic_term_id = $1
             AND learning_group.status = 'published'
             AND teacher.starts_on < $2
             AND (teacher.ends_on IS NULL OR teacher.ends_on >= $2)
           ORDER BY learning_group.id, teacher.id
           LIMIT 1"#,
    )
    .bind(context.term_id)
    .bind(change_set.reference_date)
    .fetch_one(&pool)
    .await
    .expect("fixture must contain an effective teacher episode");
    let new_teacher_id = stable_uuid("teacher-change-items:new-teacher");
    sqlx::query(
        r#"INSERT INTO users (
               id, email, username, password_hash, first_name, last_name,
               user_type, status
           ) VALUES ($1, $2, $3, 'fixture-not-a-login', 'ครูใหม่', 'กลางภาค',
                     'staff', 'active')"#,
    )
    .bind(new_teacher_id)
    .bind(format!("{new_teacher_id}@example.invalid"))
    .bind(format!("teacher-{new_teacher_id}"))
    .execute(&pool)
    .await
    .unwrap();

    let added = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        UpsertAcademicTermChangeItemRequest::AddGroupTeacher {
            change_set_row_version: change_set.row_version,
            item_row_version: None,
            learning_group_id: group_id,
            teacher_id: new_teacher_id,
            teacher_role: LearningTeacherRole::Secondary,
        },
    )
    .await
    .expect("an active staff member must be addable at the effective date");
    let (add_item_id, add_item_row_version) = added
        .items
        .iter()
        .find_map(|item| match item {
            super::models::AcademicTermChangeItem::AddGroupTeacher {
                id,
                learning_group_id,
                teacher_id,
                teacher_role,
                row_version,
                learning_group_label,
                teacher_label,
                ..
            } => {
                assert_eq!(*learning_group_id, group_id);
                assert_eq!(*teacher_id, new_teacher_id);
                assert_eq!(*teacher_role, LearningTeacherRole::Secondary);
                assert!(!learning_group_label.is_empty());
                assert!(teacher_label.contains("ครูใหม่"));
                Some((*id, *row_version))
            }
            _ => None,
        })
        .expect("response must hydrate the typed add-teacher item");
    let group_offering_id: Uuid =
        sqlx::query_scalar("SELECT learning_offering_id FROM learning_groups WHERE id = $1")
            .bind(group_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(
        offerings::operational_change_offering_ids(&pool, added.id)
            .await
            .expect("teacher items must authorize and signal through their learning group"),
        vec![group_offering_id]
    );

    let stale = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        UpsertAcademicTermChangeItemRequest::AddGroupTeacher {
            change_set_row_version: change_set.row_version,
            item_row_version: Some(add_item_row_version),
            learning_group_id: group_id,
            teacher_id: new_teacher_id,
            teacher_role: LearningTeacherRole::Primary,
        },
    )
    .await
    .expect_err("a stale change-set revision must reject the edit");
    assert!(matches!(stale, AppError::Conflict(_)));

    let adjusted = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        UpsertAcademicTermChangeItemRequest::AdjustGroupTeacherRole {
            change_set_row_version: added.row_version,
            item_row_version: None,
            learning_group_id: group_id,
            learning_group_teacher_id: assignment_id,
            teacher_id: current_teacher_id,
            teacher_role: LearningTeacherRole::Assistant,
        },
    )
    .await
    .expect("an effective assignment must accept a role adjustment item");
    let (adjust_item_id, adjust_item_row_version) = adjusted
        .items
        .iter()
        .find_map(|item| match item {
            super::models::AcademicTermChangeItem::AdjustGroupTeacherRole {
                id,
                learning_group_teacher_id,
                teacher_role,
                row_version,
                ..
            } if *learning_group_teacher_id == assignment_id => {
                assert_eq!(*teacher_role, LearningTeacherRole::Assistant);
                Some((*id, *row_version))
            }
            _ => None,
        })
        .expect("response must hydrate the typed role-adjustment item");

    let after_delete_adjust = change_sets::delete_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        adjust_item_id,
        DeleteAcademicTermChangeItemRequest {
            change_set_row_version: adjusted.row_version,
            item_row_version: adjust_item_row_version,
        },
    )
    .await
    .expect("a draft teacher-role item must be deletable");
    let stopped = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        UpsertAcademicTermChangeItemRequest::StopGroupTeacher {
            change_set_row_version: after_delete_adjust.row_version,
            item_row_version: None,
            learning_group_id: group_id,
            learning_group_teacher_id: assignment_id,
            teacher_id: current_teacher_id,
        },
    )
    .await
    .expect("an effective assignment must be stoppable");
    let (stop_item_id, stop_item_row_version) = stopped
        .items
        .iter()
        .find_map(|item| match item {
            super::models::AcademicTermChangeItem::StopGroupTeacher {
                id,
                learning_group_teacher_id,
                row_version,
                ..
            } if *learning_group_teacher_id == assignment_id => Some((*id, *row_version)),
            _ => None,
        })
        .expect("response must hydrate the typed stop-teacher item");
    let after_delete_stop = change_sets::delete_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        stop_item_id,
        DeleteAcademicTermChangeItemRequest {
            change_set_row_version: stopped.row_version,
            item_row_version: stop_item_row_version,
        },
    )
    .await
    .expect("a draft stop-teacher item must be deletable");
    let add_item = after_delete_stop
        .items
        .iter()
        .find_map(|item| match item {
            super::models::AcademicTermChangeItem::AddGroupTeacher {
                id, row_version, ..
            } if *id == add_item_id => Some((*id, *row_version)),
            _ => None,
        })
        .expect("the independent add-teacher item must remain");
    change_sets::delete_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        add_item.0,
        DeleteAcademicTermChangeItemRequest {
            change_set_row_version: after_delete_stop.row_version,
            item_row_version: add_item.1,
        },
    )
    .await
    .expect("a draft add-teacher item must be deletable");
}

#[tokio::test]
async fn teacher_handoff_preview_and_apply_replace_exact_instructors_atomically() {
    let pool = prepare_delivery_runtime_fixture("academic_teacher_handoff_apply").await;
    let context = operational_change_runtime_context(&pool).await;
    let source_id: Uuid=sqlx::query_scalar("SELECT id FROM academic_timetable_versions WHERE academic_term_id=$1 AND status='published' ORDER BY effective_from DESC,id LIMIT 1").bind(context.term_id).fetch_one(&pool).await.unwrap();
    let source = timetable_version_service::get_version(&pool, source_id, Utc::now().date_naive())
        .await
        .unwrap();
    let base = school_academic_delivery::services::versions::get_version(
        &pool,
        source.delivery_version_id,
    )
    .await
    .unwrap();
    let group = base
        .snapshot
        .offerings
        .iter()
        .flat_map(|offering| &offering.groups)
        .find(|group| !group.teachers.is_empty())
        .unwrap();
    let stopped = &group.teachers[0];
    let change_set = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        14,
        "teacher-handoff:create",
    )
    .await;
    let replacement = stable_uuid("teacher-handoff:replacement");
    sqlx::query("INSERT INTO users(id,email,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,$3,'fixture-not-a-login','ครูรับช่วง','ทดสอบ','staff','active')")
        .bind(replacement).bind(format!("{replacement}@example.invalid")).bind(format!("teacher-{replacement}")).execute(&pool).await.unwrap();
    let added = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        UpsertAcademicTermChangeItemRequest::AddGroupTeacher {
            change_set_row_version: change_set.row_version,
            item_row_version: None,
            learning_group_id: group.id,
            teacher_id: replacement,
            teacher_role: LearningTeacherRole::Primary,
        },
    )
    .await
    .unwrap();
    let changed = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        UpsertAcademicTermChangeItemRequest::StopGroupTeacher {
            change_set_row_version: added.row_version,
            item_row_version: None,
            learning_group_id: group.id,
            learning_group_teacher_id: stopped.assignment_id,
            teacher_id: stopped.teacher_id,
        },
    )
    .await
    .unwrap();
    let stop_item = changed
        .items
        .iter()
        .find_map(|item| match item {
            super::models::AcademicTermChangeItem::StopGroupTeacher { id, .. } => Some(*id),
            _ => None,
        })
        .unwrap();
    let opening_preview = publication_preview(&pool, changed.id).await.unwrap();
    let published = change_sets::publish_change_set(
        &pool,
        context.teacher_id,
        changed.id,
        PublishAcademicTermChangeSetRequest {
            effective_from: opening_preview.effective_from,
            row_version: opening_preview.change_set_row_version,
            target_delivery_version_row_version: opening_preview
                .target_delivery_version_row_version,
            preview_hash: opening_preview.preview_hash,
            acknowledged_warning_codes: vec![],
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let draft = timetable_version_service::clone_draft(
        &pool,
        context.teacher_id,
        source_id,
        CloneTimetableVersionRequest {
            source_row_version: source.row_version,
            resume_draft_id: None,
            draft_row_version: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(
        draft.delivery_version_id,
        published.target_delivery_version_id
    );
    fill_change_set_target_deficits(&pool, context.teacher_id, context.term_id, draft.id).await;
    let draft = timetable_version_service::get_version(&pool, draft.id, Utc::now().date_naive())
        .await
        .unwrap();
    let entry_id: Uuid=sqlx::query_scalar("SELECT placed.id FROM academic_timetable_block_groups placed JOIN academic_timetable_blocks block ON block.id=placed.block_id JOIN academic_timetable_block_group_instructors instructor ON instructor.block_group_id=placed.id WHERE block.timetable_version_id=$1 AND placed.learning_group_id=$2 AND instructor.instructor_id=$3 ORDER BY placed.id LIMIT 1")
        .bind(draft.id).bind(group.id).bind(stopped.teacher_id).fetch_one(&pool).await.unwrap();
    let request = PreviewTeacherHandoffRequest {
        timetable_version_id: draft.id,
        change_set_row_version: published.row_version,
        target_timetable_version_row_version: draft.row_version,
        teacher_change_item_id: stop_item,
        entry_ids: vec![entry_id],
        mode: TeacherHandoffMode::AssignOne,
        instructor_ids: vec![replacement],
    };
    let preview = teacher_handoff::preview(&pool, published.id, request.clone())
        .await
        .unwrap();
    assert!(preview.conflicts.is_empty());
    assert_eq!(preview.proposed_entries.len(), 1);
    assert!(preview.proposed_entries[0]
        .after_instructors
        .iter()
        .all(|teacher| teacher.instructor_id != stopped.teacher_id));
    let ineligible = teacher_handoff::preview(
        &pool,
        published.id,
        PreviewTeacherHandoffRequest {
            instructor_ids: vec![stopped.teacher_id],
            ..request
        },
    )
    .await
    .unwrap();
    assert!(!ineligible.conflicts.is_empty());
    let apply = ApplyTeacherHandoffRequest {
        timetable_version_id: draft.id,
        change_set_row_version: published.row_version,
        target_timetable_version_row_version: draft.row_version,
        teacher_change_item_id: stop_item,
        entries: preview
            .proposed_entries
            .iter()
            .map(|entry| TeacherHandoffEntryVersion {
                entry_id: entry.entry_id,
                row_version: entry.row_version,
            })
            .collect(),
        mode: TeacherHandoffMode::AssignOne,
        instructor_ids: vec![replacement],
        preview_hash: preview.preview_hash.unwrap(),
        idempotency_key: Uuid::new_v4(),
    };
    let stale = teacher_handoff::apply(
        &pool,
        context.teacher_id,
        published.id,
        ApplyTeacherHandoffRequest {
            preview_hash: "0".repeat(64),
            ..apply.clone()
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(stale, AppError::Conflict(_)));
    let applied = teacher_handoff::apply(&pool, context.teacher_id, published.id, apply.clone())
        .await
        .unwrap();
    assert!(!applied.response.replayed);
    let replayed = teacher_handoff::apply(&pool, context.teacher_id, published.id, apply)
        .await
        .unwrap();
    assert!(replayed.response.replayed);
    let instructors: Vec<Uuid>=sqlx::query_scalar("SELECT instructor_id FROM academic_timetable_block_group_instructors WHERE block_group_id=$1 ORDER BY instructor_id").bind(entry_id).fetch_all(&pool).await.unwrap();
    assert!(instructors.contains(&replacement));
    assert!(!instructors.contains(&stopped.teacher_id));
    let historical: bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_timetable_block_group_instructors instructor JOIN academic_timetable_block_groups placed ON placed.id=instructor.block_group_id JOIN academic_timetable_blocks block ON block.id=placed.block_id WHERE block.timetable_version_id=$1 AND placed.learning_group_id=$2 AND instructor.instructor_id=$3)")
        .bind(source_id).bind(group.id).bind(stopped.teacher_id).fetch_one(&pool).await.unwrap();
    assert!(historical, "handoff never rewrites the published table");
}

async fn add_operational_change_catalog_fixture(
    pool: &PgPool,
    context: &RuntimeContext,
) -> (Uuid, Uuid, ActivitySchedulingMode) {
    let subject_id = stable_uuid("change-set:catalog:subject");
    let subject_version_id = stable_uuid("change-set:catalog:subject-version");
    sqlx::query(
        r#"INSERT INTO subjects (
               id, code, identity_key, subject_group_id, owning_organization_unit_id
           )
           SELECT $1, 'ท99999', 'change-set-test-subject',
                  source.subject_group_id, source.owning_organization_unit_id
           FROM subject_versions version
           JOIN subjects source ON source.id = version.subject_id
           WHERE version.id = $2"#,
    )
    .bind(subject_id)
    .bind(context.subject_version_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO subject_versions (
               id, subject_id, version_no, code, name_th, name_en, credit,
               hours_per_semester, type, description, effective_from,
               effective_until, start_academic_year_id, term, is_active,
               periods_per_week, status, published_at
           )
           SELECT $1, $2, 1, 'ท99999', 'รายวิชาทดสอบกลางภาค', NULL, credit,
                  hours_per_semester, type, 'fixture', effective_from,
                  effective_until, start_academic_year_id, term, true,
                  periods_per_week, 'published', now()
           FROM subject_versions
           WHERE id = $3"#,
    )
    .bind(subject_version_id)
    .bind(subject_id)
    .bind(context.subject_version_id)
    .execute(pool)
    .await
    .unwrap();

    let source_activity_version_id: Uuid = sqlx::query_scalar(
        r#"SELECT version.id
           FROM activity_versions version
           JOIN academic_terms term ON term.id = $1
           WHERE version.status = 'published'
             AND version.effective_from <= term.start_date
             AND (version.effective_until IS NULL OR version.effective_until > term.start_date)
           ORDER BY version.id
           LIMIT 1"#,
    )
    .bind(context.term_id)
    .fetch_one(pool)
    .await
    .unwrap();
    let activity_id = stable_uuid("change-set:catalog:activity");
    let activity_version_id = stable_uuid("change-set:catalog:activity-version");
    sqlx::query(
        r#"INSERT INTO activities (
               id, code, identity_key, activity_type, owning_organization_unit_id
           )
           SELECT $1, 'ACT-CHANGE', 'change-set-test-activity', activity.activity_type,
                  activity.owning_organization_unit_id
           FROM activity_versions version
           JOIN activities activity ON activity.id = version.activity_id
           WHERE version.id = $2"#,
    )
    .bind(activity_id)
    .bind(source_activity_version_id)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO activity_versions (
               id, activity_id, version_no, name, activity_type, description,
               periods_per_week, hours_per_week, hours_per_term, scheduling_mode,
               is_active, term, grade_level_ids, start_academic_year_id,
               effective_from, effective_until, status, published_at
           )
           SELECT $1, $2, 1, 'กิจกรรมทดสอบกลางภาค', activity_type, 'fixture',
                  periods_per_week, hours_per_week, hours_per_term, scheduling_mode,
                  true, term, grade_level_ids, start_academic_year_id,
                  effective_from, effective_until, 'published', now()
           FROM activity_versions
           WHERE id = $3"#,
    )
    .bind(activity_version_id)
    .bind(activity_id)
    .bind(source_activity_version_id)
    .execute(pool)
    .await
    .unwrap();
    let scheduling_mode: ActivitySchedulingMode =
        sqlx::query_scalar("SELECT scheduling_mode FROM activity_versions WHERE id = $1")
            .bind(activity_version_id)
            .fetch_one(pool)
            .await
            .unwrap();
    (subject_version_id, activity_version_id, scheduling_mode)
}

#[tokio::test]
async fn change_set_creation_clones_the_effective_base_and_is_idempotent() {
    let pool = prepare_delivery_runtime_fixture("academic_opening_clone").await;
    let context = operational_change_runtime_context(&pool).await;
    let before: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_timetable_versions")
        .fetch_one(&pool)
        .await
        .unwrap();
    let created = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        10,
        "opening:clone",
    )
    .await;
    let base = school_academic_delivery::services::versions::get_version(
        &pool,
        created.base_delivery_version_id.unwrap(),
    )
    .await
    .unwrap();
    let target = school_academic_delivery::services::versions::get_version(
        &pool,
        created.target_delivery_version_id,
    )
    .await
    .unwrap();
    assert_eq!(target.source_version_id, Some(base.id));
    assert_eq!(
        target.status,
        school_academic_delivery::models::versions::DeliveryVersionStatus::Draft
    );
    assert_eq!(
        serde_json::to_value(&target.snapshot).unwrap(),
        serde_json::to_value(&base.snapshot).unwrap()
    );
    assert!(created.items.is_empty());
    assert_eq!(created.reason, "ปรับการเปิดสอนระหว่างภาคเรียน");
    assert_eq!(target.effective_from, None);
    assert_eq!(target.reference_date, created.reference_date);
    let retried = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        10,
        "opening:clone",
    )
    .await;
    assert_eq!(retried.id, created.id);
    let mismatched = change_sets::create_change_set(
        &pool,
        context.teacher_id,
        CreateAcademicTermChangeSetRequest {
            academic_term_id: context.term_id,
            reason: "คนละเหตุผล".into(),
            idempotency_key: stable_uuid("opening:clone"),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(mismatched, AppError::Conflict(_)));
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM academic_timetable_versions")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        before, after,
        "opening creation never creates timetable drafts"
    );
    assert!(
        change_sets::list_change_set_summaries(&pool, context.term_id)
            .await
            .unwrap()
            .iter()
            .any(|item| item.id == created.id)
    );
}

#[tokio::test]
async fn academic_term_change_set_summary_omits_hydrated_detail() {
    let pool = prepare_delivery_runtime_fixture("academic_change_set_summary").await;
    let context = operational_change_runtime_context(&pool).await;
    let change_set = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        12,
        "change-set:summary:idempotency",
    )
    .await;
    let base = school_academic_delivery::services::versions::get_version(
        &pool,
        change_set.base_delivery_version_id.unwrap(),
    )
    .await
    .unwrap();
    let first = &base.snapshot.offerings[0];
    let (offering_id, base_target) = (first.id, first.weekly_period_target);
    let detailed = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        UpsertAcademicTermChangeItemRequest::AdjustWeeklyPeriodTarget {
            change_set_row_version: change_set.row_version,
            item_row_version: None,
            learning_offering_id: offering_id,
            weekly_period_target: base_target + 1,
        },
    )
    .await
    .expect("fixture change set must contain a detail item");
    assert_eq!(detailed.items.len(), 1);

    let summaries = change_sets::list_change_set_summaries(&pool, context.term_id)
        .await
        .expect("summary list must load without detail hydration");
    let summary = summaries
        .iter()
        .find(|summary| summary.id == change_set.id)
        .expect("created change set must be listed");
    let value = serde_json::to_value(summary).expect("summary must serialize");
    let keys = value
        .as_object()
        .expect("summary must serialize as an object")
        .keys()
        .cloned()
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        keys,
        std::collections::BTreeSet::from([
            "academicTermId".to_string(),
            "academicYearId".to_string(),
            "effectiveFrom".to_string(),
            "referenceDate".to_string(),
            "id".to_string(),
            "reason".to_string(),
            "status".to_string(),
            "targetDeliveryVersionId".to_string(),
            "updatedAt".to_string(),
        ])
    );
    assert_eq!(
        summary.target_delivery_version_id,
        detailed.target_delivery_version_id
    );
}

#[tokio::test]
async fn draft_change_set_update_uses_row_versions_and_cancel_preserves_the_base() {
    let pool = prepare_delivery_runtime_fixture("academic_change_set_update_cancel").await;
    let context = operational_change_runtime_context(&pool).await;
    let created = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        11,
        "change-set:update:idempotency",
    )
    .await;
    let revised_effective_from = created
        .reference_date
        .checked_add_signed(chrono::Duration::days(1))
        .unwrap();

    let updated = change_sets::update_change_set(
        &pool,
        context.teacher_id,
        created.id,
        UpdateAcademicTermChangeSetRequest {
            row_version: created.row_version,
            reference_date: revised_effective_from,
            reason: "เหตุผลที่ปรับแล้ว".to_string(),
        },
    )
    .await
    .expect("a draft reason must remain editable");
    assert_eq!(updated.reason, "เหตุผลที่ปรับแล้ว");
    assert_eq!(updated.effective_from, None);
    assert_eq!(updated.reference_date, revised_effective_from);
    assert_eq!(updated.row_version, created.row_version + 1);
    let target_effective_from: NaiveDate =
        sqlx::query_scalar("SELECT reference_date FROM academic_delivery_versions WHERE id = $1")
            .bind(created.target_delivery_version_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(target_effective_from, revised_effective_from);

    let stale = change_sets::update_change_set(
        &pool,
        context.teacher_id,
        created.id,
        UpdateAcademicTermChangeSetRequest {
            row_version: created.row_version,
            reference_date: created.reference_date,
            reason: "ข้อมูลล้าสมัย".to_string(),
        },
    )
    .await
    .expect_err("a stale draft update must conflict");
    assert!(matches!(stale, AppError::Conflict(_)));

    let version = school_academic_delivery::services::versions::get_version(
        &pool,
        created.target_delivery_version_id,
    )
    .await
    .unwrap();
    let deleted = change_sets::delete_version(
        &pool,
        context.teacher_id,
        version.id,
        delete_opening_request(&updated, &version),
    )
    .await
    .unwrap();
    assert_eq!(deleted.id, version.id);
    assert!(matches!(
        change_sets::get_change_set(&pool, created.id).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        school_academic_delivery::services::versions::get_version(&pool, version.id).await,
        Err(AppError::NotFound(_))
    ));
    assert_eq!(
        school_academic_delivery::services::versions::get_version(
            &pool,
            created.base_delivery_version_id.unwrap()
        )
        .await
        .unwrap()
        .status,
        school_academic_delivery::models::versions::DeliveryVersionStatus::Published
    );
}

#[tokio::test]
async fn change_set_creation_rejects_unwritable_terms_and_out_of_range_dates() {
    let pool = prepare_delivery_runtime_fixture("academic_change_set_date_guards").await;
    let context = operational_change_runtime_context(&pool).await;
    let (term_start, year_end): (NaiveDate, NaiveDate) = sqlx::query_as(
        r#"SELECT term.start_date, year.end_date
           FROM academic_terms term
           JOIN academic_years year ON year.id = term.academic_year_id
           WHERE term.id = $1"#,
    )
    .bind(context.term_id)
    .fetch_one(&pool)
    .await
    .unwrap();

    for (name, date) in [
        (
            "change-set:before-term",
            term_start
                .checked_sub_signed(chrono::Duration::days(1))
                .unwrap(),
        ),
        (
            "change-set:after-year",
            year_end
                .checked_add_signed(chrono::Duration::days(1))
                .unwrap(),
        ),
    ] {
        let created = change_sets::create_change_set(
            &pool,
            context.teacher_id,
            CreateAcademicTermChangeSetRequest {
                academic_term_id: context.term_id,
                reason: "วันที่ไม่ถูกต้อง".into(),
                idempotency_key: stable_uuid(name),
            },
        )
        .await
        .unwrap();
        let error = change_sets::update_change_set(
            &pool,
            context.teacher_id,
            created.id,
            UpdateAcademicTermChangeSetRequest {
                row_version: created.row_version,
                reference_date: date,
                reason: created.reason,
            },
        )
        .await
        .expect_err("invalid reference date must fail");
        assert!(matches!(error, AppError::ValidationError(_)));
    }

    sqlx::query("UPDATE academic_terms SET status = 'closing' WHERE id = $1")
        .bind(context.term_id)
        .execute(&pool)
        .await
        .unwrap();
    let closing = change_sets::create_change_set(
        &pool,
        context.teacher_id,
        CreateAcademicTermChangeSetRequest {
            academic_term_id: context.term_id,
            reason: "ภาคเรียนกำลังปิด".to_string(),
            idempotency_key: stable_uuid("change-set:closing-term"),
        },
    )
    .await
    .expect("closing allows preparing outstanding operational changes");
    assert_eq!(closing.status, AcademicTermChangeSetStatus::Draft);
    for (year_status, term_status) in [("closed", "planning"), ("active", "closed")] {
        sqlx::query("UPDATE academic_years SET status=$2 WHERE id=$1")
            .bind(context.year_id)
            .bind(year_status)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE academic_terms SET status=$2,closed_on=CASE WHEN $2='closed' THEN start_date ELSE NULL END WHERE id=$1")
            .bind(context.term_id).bind(term_status).execute(&pool).await.unwrap();
        let closed = change_sets::create_change_set(
            &pool,
            context.teacher_id,
            CreateAcademicTermChangeSetRequest {
                academic_term_id: context.term_id,
                reason: "Must not create".into(),
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await;
        assert!(matches!(closed, Err(AppError::Conflict(_))));
        let update = change_sets::update_change_set(
            &pool,
            context.teacher_id,
            closing.id,
            UpdateAcademicTermChangeSetRequest {
                row_version: closing.row_version,
                reference_date: closing.reference_date,
                reason: "Must not change".into(),
            },
        )
        .await;
        assert!(matches!(update, Err(AppError::Conflict(_))));
        let version = school_academic_delivery::services::versions::get_version(
            &pool,
            closing.target_delivery_version_id,
        )
        .await
        .unwrap();
        let deleted = change_sets::delete_version(
            &pool,
            context.teacher_id,
            version.id,
            delete_opening_request(&closing, &version),
        )
        .await;
        assert!(matches!(deleted, Err(AppError::Conflict(_))));
        assert_eq!(
            change_sets::get_change_set(&pool, closing.id)
                .await
                .unwrap()
                .row_version,
            closing.row_version
        );
    }
}

#[tokio::test]
async fn add_change_items_create_draft_course_and_activity_delivery_then_delete_cleanly() {
    let pool = prepare_delivery_runtime_fixture("academic_change_set_add_items").await;
    let context = operational_change_runtime_context(&pool).await;
    let change_set = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        12,
        "change-set:add-items:idempotency",
    )
    .await;
    let (subject_version_id, activity_version_id, scheduling_mode) =
        add_operational_change_catalog_fixture(&pool, &context).await;
    let CreateLearningOfferingRequest::Course(mut course) = course_request(&context) else {
        unreachable!();
    };
    course.subject_version_id = subject_version_id;

    let with_course = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        UpsertAcademicTermChangeItemRequest::AddCourse {
            change_set_row_version: change_set.row_version,
            offering: course,
        },
    )
    .await
    .expect("a course add item must create draft delivery resources");
    let (course_item_id, course_offering_id, course_item_row_version) = with_course
        .items
        .iter()
        .find_map(|item| match item {
            super::models::AcademicTermChangeItem::AddOffering {
                id,
                learning_offering_id,
                row_version,
                ..
            } => Some((*id, *learning_offering_id, *row_version)),
            _ => None,
        })
        .expect("the change set must expose the added course item");
    let (course_status, starts_on): (String, NaiveDate) =
        sqlx::query_as("SELECT status, starts_on FROM learning_offerings WHERE id = $1")
            .bind(course_offering_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(course_status, "draft");
    assert_eq!(starts_on, change_set.reference_date);
    let target = school_academic_delivery::services::versions::get_version(
        &pool,
        change_set.target_delivery_version_id,
    )
    .await
    .unwrap();
    let course_target = target
        .snapshot
        .offerings
        .iter()
        .find(|item| item.id == course_offering_id)
        .unwrap()
        .weekly_period_target;
    let catalog_standard: i32 =
        sqlx::query_scalar("SELECT periods_per_week FROM subject_versions WHERE id = $1")
            .bind(subject_version_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(course_target, catalog_standard);
    let course_group_count: i64 =
        sqlx::query_scalar("SELECT count(*) FROM learning_groups WHERE learning_offering_id = $1")
            .bind(course_offering_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(course_group_count, 1);

    let without_course = change_sets::delete_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        course_item_id,
        DeleteAcademicTermChangeItemRequest {
            change_set_row_version: with_course.row_version,
            item_row_version: course_item_row_version,
        },
    )
    .await
    .expect("deleting a draft-only add item must remove its resource graph");
    let course_still_exists: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM learning_offerings WHERE id = $1)")
            .bind(course_offering_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!course_still_exists);

    let with_activity = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        UpsertAcademicTermChangeItemRequest::AddActivity {
            change_set_row_version: without_course.row_version,
            weekly_period_target: 2,
            offering: CreateActivityOfferingRequest {
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
                capacity: None,
                attendance_requirement: ActivityAttendanceRequirement {
                    minimum_percent: None,
                    required_sessions: None,
                },
                pass_criteria: ActivityPassCriteria {
                    require_attendance: false,
                    require_teacher_confirmation: true,
                    outcomes: vec!["pass".to_string(), "fail".to_string()],
                },
            },
        },
    )
    .await
    .expect("an activity add item must require and store an explicit period target");
    let target = school_academic_delivery::services::versions::get_version(
        &pool,
        change_set.target_delivery_version_id,
    )
    .await
    .unwrap();
    let activity_target = target.snapshot.offerings.iter().find(|item| item.kind==LearningOfferingKind::Activity && matches!(&item.catalog,LearningOfferingSnapshot::Activity(activity) if activity.activity_version_id==activity_version_id)).map(|item| item.weekly_period_target);
    assert_eq!(activity_target, Some(2));
    assert_eq!(with_activity.items.len(), 1);
}

#[tokio::test]
async fn adjust_and_stop_items_mutate_only_the_target_version_and_delete_restores_it() {
    let pool = prepare_delivery_runtime_fixture("academic_opening_adjust_stop").await;
    let context = operational_change_runtime_context(&pool).await;
    let created = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        13,
        "opening:adjust-stop",
    )
    .await;
    let base = school_academic_delivery::services::versions::get_version(
        &pool,
        created.base_delivery_version_id.unwrap(),
    )
    .await
    .unwrap();
    let first = &base.snapshot.offerings[0];
    let before_tables: serde_json::Value = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(block) ORDER BY id) FROM academic_timetable_blocks block",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let adjusted = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        created.id,
        UpsertAcademicTermChangeItemRequest::AdjustWeeklyPeriodTarget {
            change_set_row_version: created.row_version,
            item_row_version: None,
            learning_offering_id: first.id,
            weekly_period_target: first.weekly_period_target + 1,
        },
    )
    .await
    .unwrap();
    let target = school_academic_delivery::services::versions::get_version(
        &pool,
        created.target_delivery_version_id,
    )
    .await
    .unwrap();
    assert_eq!(
        target
            .snapshot
            .offerings
            .iter()
            .find(|offering| offering.id == first.id)
            .unwrap()
            .weekly_period_target,
        first.weekly_period_target + 1
    );
    let item = &adjusted.items[0];
    let restored = change_sets::delete_change_item(
        &pool,
        context.teacher_id,
        created.id,
        match item {
            super::models::AcademicTermChangeItem::AdjustWeeklyPeriodTarget { id, .. }
            | super::models::AcademicTermChangeItem::StopOffering { id, .. } => *id,
            _ => panic!("expected opening adjustment"),
        },
        DeleteAcademicTermChangeItemRequest {
            change_set_row_version: adjusted.row_version,
            item_row_version: match item {
                super::models::AcademicTermChangeItem::AdjustWeeklyPeriodTarget {
                    row_version,
                    ..
                }
                | super::models::AcademicTermChangeItem::StopOffering { row_version, .. } => {
                    *row_version
                }
                _ => panic!("expected opening adjustment"),
            },
        },
    )
    .await
    .unwrap();
    let stopped = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        created.id,
        UpsertAcademicTermChangeItemRequest::StopOffering {
            change_set_row_version: restored.row_version,
            item_row_version: None,
            learning_offering_id: first.id,
        },
    )
    .await
    .unwrap();
    let target = school_academic_delivery::services::versions::get_version(
        &pool,
        created.target_delivery_version_id,
    )
    .await
    .unwrap();
    assert!(!target
        .snapshot
        .offerings
        .iter()
        .any(|offering| offering.id == first.id));
    let item = &stopped.items[0];
    change_sets::delete_change_item(
        &pool,
        context.teacher_id,
        created.id,
        match item {
            super::models::AcademicTermChangeItem::AdjustWeeklyPeriodTarget { id, .. }
            | super::models::AcademicTermChangeItem::StopOffering { id, .. } => *id,
            _ => panic!("expected opening adjustment"),
        },
        DeleteAcademicTermChangeItemRequest {
            change_set_row_version: stopped.row_version,
            item_row_version: match item {
                super::models::AcademicTermChangeItem::AdjustWeeklyPeriodTarget {
                    row_version,
                    ..
                }
                | super::models::AcademicTermChangeItem::StopOffering { row_version, .. } => {
                    *row_version
                }
                _ => panic!("expected opening adjustment"),
            },
        },
    )
    .await
    .unwrap();
    let restored = school_academic_delivery::services::versions::get_version(
        &pool,
        created.target_delivery_version_id,
    )
    .await
    .unwrap();
    assert_eq!(
        serde_json::to_value(&restored.snapshot).unwrap(),
        serde_json::to_value(&base.snapshot).unwrap()
    );
    let after_tables: serde_json::Value = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(block) ORDER BY id) FROM academic_timetable_blocks block",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        before_tables, after_tables,
        "opening adjustments never rewrite placements"
    );
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
                 AND learning_group.status = 'published'
                 AND learning_group.roster_status = 'published'
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
async fn dated_roster_rejects_closed_groups_and_dates_outside_availability() {
    let pool = prepare_delivery_runtime_fixture("academic_dated_roster_guards").await;
    let context = operational_change_runtime_context(&pool).await;
    let (group_id, membership_id): (Uuid, Uuid) = sqlx::query_as(
        r#"SELECT learning_group.id, membership.id
           FROM learning_groups learning_group
           JOIN learning_group_students membership
             ON membership.learning_group_id = learning_group.id
            AND membership.membership_status = 'active'
           WHERE learning_group.academic_term_id = $1
             AND learning_group.status = 'published'
             AND learning_group.roster_status = 'published'
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
    let before_offering: NaiveDate = sqlx::query_scalar(
        r#"SELECT offering.starts_on - 1
           FROM learning_groups learning_group
           JOIN learning_offerings offering ON offering.id = learning_group.learning_offering_id
           WHERE learning_group.id = $1"#,
    )
    .bind(group_id)
    .fetch_one(&pool)
    .await
    .unwrap();
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

    sqlx::query("UPDATE learning_groups SET status = 'closed' WHERE id = $1")
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

#[tokio::test]
async fn change_set_preview_blocks_an_empty_change_set_with_a_stable_hash() {
    let pool = prepare_delivery_runtime_fixture("academic_change_set_preview_empty").await;
    let context = operational_change_runtime_context(&pool).await;
    let change_set = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        14,
        "change-set:preview-empty:idempotency",
    )
    .await;

    let preview = publication_preview(&pool, change_set.id)
        .await
        .expect("a draft change set must return a typed preview");

    assert_eq!(preview.change_set_id, change_set.id);
    assert_eq!(preview.change_set_row_version, change_set.row_version);
    assert_eq!(
        preview.target_delivery_version_id,
        change_set.target_delivery_version_id
    );
    assert_eq!(preview.preview_hash.len(), 64);
    assert!(preview.findings.iter().any(|finding| {
        finding.code == AcademicChangeFindingCode::ChangeSetNoItems
            && finding.severity == AcademicChangeFindingSeverity::Blocking
    }));
    for (year_status, term_status, blocked) in [
        ("closing", "closing", false),
        ("closed", "planning", true),
        ("active", "closed", true),
    ] {
        sqlx::query("UPDATE academic_years SET status=$2 WHERE id=$1")
            .bind(context.year_id)
            .bind(year_status)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("UPDATE academic_terms SET status=$2,closed_on=CASE WHEN $2='closed' THEN start_date ELSE NULL END WHERE id=$1")
            .bind(context.term_id).bind(term_status).execute(&pool).await.unwrap();
        let state = publication_preview(&pool, change_set.id).await.unwrap();
        assert_eq!(
            state
                .findings
                .iter()
                .any(|finding| finding.code == AcademicChangeFindingCode::TermNotWritable),
            blocked
        );
    }
    sqlx::query("UPDATE academic_years SET status='active' WHERE id=$1")
        .bind(context.year_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_terms SET status='planning',closed_on=NULL WHERE id=$1")
        .bind(context.term_id)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(preview.impact_counts.groups, 0);
    let repeated = publication_preview(&pool, change_set.id).await.unwrap();
    assert_eq!(repeated.preview_hash, preview.preview_hash);
}

#[tokio::test]
async fn timetable_readiness_blocks_missing_and_ineligible_exact_entry_instructors() {
    let pool = prepare_delivery_runtime_fixture("academic_table_exact_instructors").await;
    let context = operational_change_runtime_context(&pool).await;
    let draft = independent_table_draft(&pool, &context).await;
    let (block_id,entry_id): (Uuid,Uuid)=sqlx::query_as("SELECT block.id,placed.id FROM academic_timetable_blocks block JOIN academic_timetable_block_groups placed ON placed.block_id=block.id WHERE block.timetable_version_id=$1 AND block.is_active AND placed.is_active ORDER BY placed.id LIMIT 1").bind(draft.id).fetch_one(&pool).await.unwrap();
    sqlx::query("DELETE FROM academic_timetable_block_group_instructors WHERE block_group_id=$1")
        .bind(entry_id)
        .execute(&pool)
        .await
        .unwrap();
    let missing = table_preview(&pool, draft.id).await;
    assert!(!missing.can_publish);
    assert!(missing.source_issues.iter().any(|issue| issue.block_id==block_id && issue.code==school_academic_timetable::models::timetable_source::TimetableSourceIssueCode::MissingInstructor));
    let outsider = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,email,username,password_hash,first_name,last_name,user_type,status) VALUES($1,$2,$3,'fixture','ครูนอกกลุ่ม','ทดสอบ','staff','active')")
        .bind(outsider).bind(format!("{outsider}@example.invalid")).bind(outsider.to_string()).execute(&pool).await.unwrap();
    sqlx::query("INSERT INTO academic_timetable_block_group_instructors(block_group_id,instructor_id,role,display_order) VALUES($1,$2,'primary',1)").bind(entry_id).bind(outsider).execute(&pool).await.unwrap();
    let ineligible = table_preview(&pool, draft.id).await;
    assert!(!ineligible.can_publish);
    assert!(ineligible.source_issues.iter().any(|issue| issue.block_id==block_id && issue.teacher_id==Some(outsider) && issue.code==school_academic_timetable::models::timetable_source::TimetableSourceIssueCode::IneligibleInstructor));
    assert_ne!(missing.preview_hash, ineligible.preview_hash);
}

#[tokio::test]
async fn independent_timetable_can_publish_position_changes_without_a_delivery_revision() {
    use school_academic_timetable::services::timetable_lifecycle;
    let pool = prepare_delivery_runtime_fixture("academic_table_publish_positions").await;
    let context = operational_change_runtime_context(&pool).await;
    let draft = independent_table_draft(&pool, &context).await;
    let before_openings: serde_json::Value = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(opening) ORDER BY id) FROM academic_delivery_versions opening",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    fill_change_set_target_deficits(&pool, context.teacher_id, context.term_id, draft.id).await;
    let preview = table_preview(&pool, draft.id).await;
    assert!(preview.content_changed);
    assert!(
        preview.can_publish,
        "table findings: {:?}",
        preview.findings
    );
    let request =
        school_academic_timetable::models::timetable_publication::PublishTimetableVersionRequest {
            row_version: preview.row_version,
            effective_from: preview.effective_from,
            preview_hash: preview.preview_hash,
            idempotency_key: Uuid::new_v4(),
        };
    let published =
        timetable_lifecycle::publish(&pool, context.teacher_id, draft.id, request.clone())
            .await
            .unwrap();
    assert_eq!(
        published.status,
        school_academic_core::models::TimetableVersionStatus::Published
    );
    assert_eq!(published.delivery_version_id, draft.delivery_version_id);
    assert_eq!(published.effective_from, Some(request.effective_from));
    let repeated =
        timetable_lifecycle::publish(&pool, context.teacher_id, draft.id, request.clone())
            .await
            .unwrap();
    assert_eq!(repeated.row_version, published.row_version);
    let changed_retry = timetable_lifecycle::publish(
        &pool,
        context.teacher_id,
        draft.id,
        school_academic_timetable::models::timetable_publication::PublishTimetableVersionRequest {
            effective_from: request.effective_from.succ_opt().unwrap(),
            ..request
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(changed_retry, AppError::Conflict(_)));
    let after_openings: serde_json::Value = sqlx::query_scalar(
        "SELECT jsonb_agg(to_jsonb(opening) ORDER BY id) FROM academic_delivery_versions opening",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        before_openings, after_openings,
        "table publication never publishes or revises opening data"
    );
}

#[tokio::test]
async fn change_set_preview_blocks_a_past_effective_date_after_the_term_becomes_active() {
    let pool = prepare_delivery_runtime_fixture("academic_change_set_preview_past_date").await;
    let context = operational_change_runtime_context(&pool).await;
    let change_set = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        1,
        "change-set:preview-past-date:idempotency",
    )
    .await;
    sqlx::query("UPDATE academic_term_change_sets SET reference_date = $1 WHERE id = $2")
        .bind(Utc::now().date_naive() - chrono::Duration::days(1))
        .bind(change_set.id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_delivery_versions SET reference_date=$1 WHERE id=$2")
        .bind(Utc::now().date_naive() - Duration::days(1))
        .bind(change_set.target_delivery_version_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE academic_terms SET status = 'active' WHERE id = $1")
        .bind(context.term_id)
        .execute(&pool)
        .await
        .unwrap();

    let preview = publication_preview(&pool, change_set.id).await.unwrap();

    assert!(preview.findings.iter().any(|finding| {
        finding.code == AcademicChangeFindingCode::EffectiveDateInvalid
            && finding.severity == AcademicChangeFindingSeverity::Blocking
    }));
}

#[tokio::test]
async fn change_set_preview_counts_stop_impact_without_exposing_roster_identities() {
    let pool = prepare_delivery_runtime_fixture("academic_change_set_preview_stop_impact").await;
    let context = operational_change_runtime_context(&pool).await;
    let change_set = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        15,
        "change-set:preview-stop-impact:idempotency",
    )
    .await;
    let (offering_id, learning_group_id, subject_id, student_academic_year_id): (
        Uuid,
        Uuid,
        Uuid,
        Uuid,
    ) = sqlx::query_as(
        r#"SELECT target.learning_offering_id,
                      learning_group.id,
                      detail.subject_id,
                      membership.student_academic_year_id
           FROM (SELECT opening.id AS delivery_version_id,(entry->>'id')::uuid AS learning_offering_id,(entry->>'weeklyPeriodTarget')::integer AS weekly_period_target FROM academic_delivery_versions opening CROSS JOIN LATERAL jsonb_array_elements(opening.snapshot->'offerings') entry) target
           JOIN course_offering_details detail
             ON detail.learning_offering_id = target.learning_offering_id
           JOIN learning_groups learning_group
             ON learning_group.learning_offering_id = target.learning_offering_id
            AND learning_group.academic_term_id = detail.academic_term_id
            AND learning_group.academic_year_id = detail.academic_year_id
           JOIN learning_group_students membership
             ON membership.learning_group_id = learning_group.id
           WHERE target.delivery_version_id = $1
             AND learning_group.status <> 'closed'
             AND membership.left_at IS NULL
           ORDER BY target.learning_offering_id, learning_group.id, membership.id
           LIMIT 1"#,
    )
    .bind(change_set.base_delivery_version_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"INSERT INTO learning_group_result_overrides
              (learning_group_id,learning_offering_id,academic_term_id,academic_year_id,
               subject_id,student_academic_year_id,outcome,updated_by)
           VALUES ($1,$2,$3,$4,$5,$6,'incomplete',$7)
           ON CONFLICT (learning_group_id,student_academic_year_id)
           DO UPDATE SET outcome = EXCLUDED.outcome,
                         updated_by = EXCLUDED.updated_by,
                         updated_at = now()"#,
    )
    .bind(learning_group_id)
    .bind(offering_id)
    .bind(context.term_id)
    .bind(context.year_id)
    .bind(subject_id)
    .bind(student_academic_year_id)
    .bind(context.teacher_id)
    .execute(&pool)
    .await
    .unwrap();
    let expected = sqlx::query(
        r#"SELECT
             (SELECT count(*) FROM learning_groups WHERE learning_offering_id = $1) AS groups,
             (SELECT count(*) FROM learning_group_homerooms coverage
                JOIN learning_groups learning_group ON learning_group.id = coverage.learning_group_id
                WHERE learning_group.learning_offering_id = $1) AS homerooms,
             (SELECT count(*) FROM learning_group_students membership
                JOIN learning_groups learning_group ON learning_group.id = membership.learning_group_id
                WHERE learning_group.learning_offering_id = $1) AS membership_intervals,
             (SELECT count(*) FROM learning_group_teachers teacher
                JOIN learning_groups learning_group ON learning_group.id = teacher.learning_group_id
                WHERE learning_group.learning_offering_id = $1) AS teacher_assignments,
             (SELECT count(*) FROM academic_timetable_blocks block
                WHERE block.timetable_version_id IN (SELECT id FROM academic_timetable_versions WHERE delivery_version_id=$2)
                  AND block.learning_offering_id = $1 AND block.is_active) AS target_timetable_entries,
             (SELECT count(*) FROM course_assessment_plans plan
                WHERE plan.learning_offering_id = $1) AS course_assessment_plans,
             (SELECT count(*) FROM course_assessment_phases phase
                JOIN course_assessment_plans plan ON plan.id = phase.plan_id
                WHERE plan.learning_offering_id = $1) AS course_assessment_phases,
             (SELECT count(*) FROM learning_group_score_items item
                WHERE item.learning_offering_id = $1) AS learning_group_score_items,
             (SELECT count(*) FROM learning_group_student_scores score
                WHERE score.learning_offering_id = $1) AS student_scores,
             (SELECT count(*) FROM learning_group_result_overrides selection
                JOIN learning_groups learning_group
                  ON learning_group.id = selection.learning_group_id
                WHERE learning_group.learning_offering_id = $1) AS result_selections,
             ((SELECT count(*) FROM learning_group_phase_confirmations confirmation
                 JOIN learning_groups learning_group
                   ON learning_group.id = confirmation.learning_group_id
                 WHERE learning_group.learning_offering_id = $1)
              + (SELECT count(*) FROM learning_group_result_confirmations confirmation
                 JOIN learning_groups learning_group
                   ON learning_group.id = confirmation.learning_group_id
                 WHERE learning_group.learning_offering_id = $1)
              + (SELECT count(*) FROM academic_activity_result_confirmations confirmation
                 JOIN learning_groups learning_group
                   ON learning_group.id = confirmation.learning_group_id
                 WHERE learning_group.learning_offering_id = $1)
              + (SELECT count(*) FROM learning_group_evaluation_confirmations confirmation
                 JOIN learning_groups learning_group
                   ON learning_group.id = confirmation.learning_group_id
                 WHERE learning_group.learning_offering_id = $1)) AS result_confirmations,
             (SELECT count(*) FROM academic_activity_evaluations evaluation
                WHERE evaluation.learning_offering_id = $1) AS activity_evaluations,
             (SELECT count(*) FROM learning_group_student_evaluations evaluation
                WHERE evaluation.learning_offering_id = $1) AS learner_evaluations,
             (SELECT count(*) FROM (
                  SELECT course_lock.id
                  FROM academic_course_result_locks course_lock
                  WHERE EXISTS (
                      SELECT 1 FROM course_offering_details detail
                      WHERE detail.learning_offering_id = $1
                        AND detail.subject_id = course_lock.subject_id
                        AND detail.academic_term_id = course_lock.academic_term_id
                        AND detail.academic_year_id = course_lock.academic_year_id
                  )
                  UNION ALL
                  SELECT activity_lock.id
                  FROM academic_activity_result_locks activity_lock
                  WHERE activity_lock.learning_offering_id = $1
                  UNION ALL
                  SELECT evaluation_lock.id
                  FROM subject_term_evaluation_locks evaluation_lock
                  WHERE EXISTS (
                      SELECT 1 FROM course_offering_details detail
                      WHERE detail.learning_offering_id = $1
                        AND detail.subject_id = evaluation_lock.subject_id
                        AND detail.academic_term_id = evaluation_lock.academic_term_id
                        AND detail.academic_year_id = evaluation_lock.academic_year_id
                  )
              ) official_lock) AS official_result_locks,
             ((SELECT count(*) FROM academic_course_results result
                 WHERE result.learning_offering_id = $1)
              + (SELECT count(*) FROM academic_activity_results result
                 WHERE result.learning_offering_id = $1)
              + (SELECT count(*) FROM subject_term_student_evaluations result
                 WHERE result.learning_offering_id = $1)) AS official_results,
             (SELECT count(*) FROM academic_result_corrections correction
                WHERE EXISTS (
                    SELECT 1 FROM academic_course_results result
                    WHERE result.id = correction.course_result_id
                      AND result.learning_offering_id = $1
                ) OR EXISTS (
                    SELECT 1 FROM academic_activity_results result
                    WHERE result.id = correction.activity_result_id
                      AND result.learning_offering_id = $1
                ) OR EXISTS (
                    SELECT 1 FROM subject_term_student_evaluations result
                    WHERE result.id = correction.subject_student_evaluation_id
                      AND result.learning_offering_id = $1
                )) AS result_corrections,
             (SELECT count(*) FROM academic_exam_schedule_items item
                WHERE item.learning_offering_id = $1) AS exam_schedule_items,
             (SELECT count(*) FROM supervision_observations observation
                JOIN learning_groups learning_group ON learning_group.id = observation.learning_group_id
                WHERE learning_group.learning_offering_id = $1) AS supervision_observations"#,
    )
    .bind(offering_id)
    .bind(change_set.base_delivery_version_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let expected = AcademicChangeImpactCounts {
        groups: expected.get("groups"),
        homerooms: expected.get("homerooms"),
        membership_intervals: expected.get("membership_intervals"),
        teacher_assignments: expected.get("teacher_assignments"),
        target_timetable_entries: expected.get("target_timetable_entries"),
        course_assessment_plans: expected.get("course_assessment_plans"),
        course_assessment_phases: expected.get("course_assessment_phases"),
        learning_group_score_items: expected.get("learning_group_score_items"),
        student_scores: expected.get("student_scores"),
        result_selections: expected.get("result_selections"),
        result_confirmations: expected.get("result_confirmations"),
        activity_evaluations: expected.get("activity_evaluations"),
        learner_evaluations: expected.get("learner_evaluations"),
        official_result_locks: expected.get("official_result_locks"),
        official_results: expected.get("official_results"),
        result_corrections: expected.get("result_corrections"),
        exam_schedule_items: expected.get("exam_schedule_items"),
        supervision_observations: expected.get("supervision_observations"),
    };
    assert!(expected.result_selections > 0);

    let changed = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        UpsertAcademicTermChangeItemRequest::StopOffering {
            change_set_row_version: change_set.row_version,
            item_row_version: None,
            learning_offering_id: offering_id,
        },
    )
    .await
    .unwrap();
    let preview = publication_preview(&pool, changed.id).await.unwrap();
    assert_eq!(
        offerings::operational_change_offering_ids(&pool, changed.id)
            .await
            .unwrap(),
        vec![offering_id]
    );

    assert_eq!(preview.impact_counts, expected);
    assert!(!preview
        .findings
        .iter()
        .any(|finding| finding.code == AcademicChangeFindingCode::ChangeSetNoItems));
}

#[tokio::test]
async fn timetable_preview_reports_each_group_below_its_opening_target() {
    let pool = prepare_delivery_runtime_fixture("academic_table_group_deficits").await;
    let context = operational_change_runtime_context(&pool).await;
    let draft = independent_table_draft(&pool, &context).await;
    let opening =
        school_academic_delivery::services::versions::get_version(&pool, draft.delivery_version_id)
            .await
            .unwrap();
    let offering = opening
        .snapshot
        .offerings
        .iter()
        .find(|offering| {
            matches!(&offering.catalog, LearningOfferingSnapshot::Course(_))
                && !offering.groups.is_empty()
        })
        .unwrap();
    sqlx::query("DELETE FROM academic_timetable_blocks WHERE timetable_version_id=$1 AND learning_offering_id=$2").bind(draft.id).bind(offering.id).execute(&pool).await.unwrap();
    let preview = table_preview(&pool, draft.id).await;
    assert!(!preview.can_publish);
    for group in &offering.groups {
        assert!(preview.findings.iter().any(|finding| finding.code==school_academic_timetable::models::timetable_publication::TimetablePublicationFindingCode::PeriodCountMismatch && finding.learning_group_id==Some(group.id) && finding.learning_offering_id==Some(offering.id)));
    }
}

#[tokio::test]
async fn timetable_preview_stays_unchanged_after_a_rejected_draft_collision() {
    let pool = prepare_delivery_runtime_fixture("academic_table_rejected_collision").await;
    let context = operational_change_runtime_context(&pool).await;
    let draft = independent_table_draft(&pool, &context).await;
    let block=timetable_block_service::get_block(&pool,sqlx::query_scalar("SELECT block.id FROM academic_timetable_blocks block JOIN academic_timetable_block_groups placed ON placed.block_id=block.id WHERE block.timetable_version_id=$1 AND block.is_active AND placed.is_active AND block.scheduling_mode='independent' ORDER BY block.id LIMIT 1").bind(draft.id).fetch_one(&pool).await.unwrap()).await.unwrap();
    let placed = &block.groups[0];
    let before = table_preview(&pool, draft.id).await;
    let collision = timetable_block_service::create_ordinary_block(
        &pool,
        context.teacher_id,
        CreateOrdinaryTimetableBlockRequest {
            timetable_version_id: draft.id,
            academic_term_id: context.term_id,
            learning_group_id: placed.learning_group_id,
            day_of_week: block.day_of_week.clone(),
            bell_schedule_period_id: block.bell_schedule_period_id,
            room_id: placed.room_id,
            note: None,
            instructor_ids: placed
                .instructors
                .iter()
                .map(|teacher| teacher.teacher_id)
                .collect(),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(collision, AppError::Conflict(_)));
    let after = table_preview(&pool, draft.id).await;
    assert_eq!(before.preview_hash, after.preview_hash);
    assert_eq!(before.block_count, after.block_count);
}

#[tokio::test]
async fn publishing_a_stop_change_set_is_atomic_and_idempotent() {
    let pool = prepare_delivery_runtime_fixture("academic_change_set_publish_stop").await;
    let context = operational_change_runtime_context(&pool).await;
    let change_set = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        17,
        "change-set:publish-stop:create",
    )
    .await;
    let offering_id: Uuid = sqlx::query_scalar(
        r#"SELECT target.learning_offering_id
           FROM (SELECT opening.id AS delivery_version_id,(entry->>'id')::uuid AS learning_offering_id,(entry->>'weeklyPeriodTarget')::integer AS weekly_period_target FROM academic_delivery_versions opening CROSS JOIN LATERAL jsonb_array_elements(opening.snapshot->'offerings') entry) target
           WHERE target.delivery_version_id = $1
           ORDER BY target.learning_offering_id
           LIMIT 1"#,
    )
    .bind(change_set.base_delivery_version_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let changed = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        UpsertAcademicTermChangeItemRequest::StopOffering {
            change_set_row_version: change_set.row_version,
            item_row_version: None,
            learning_offering_id: offering_id,
        },
    )
    .await
    .unwrap();
    let preview = publication_preview(&pool, changed.id).await.unwrap();
    let blocking = preview
        .findings
        .iter()
        .filter(|finding| finding.severity == AcademicChangeFindingSeverity::Blocking)
        .collect::<Vec<_>>();
    assert!(blocking.is_empty(), "blocking findings: {blocking:#?}");
    let warning_codes = preview
        .findings
        .iter()
        .filter(|finding| finding.severity == AcademicChangeFindingSeverity::Warning)
        .map(|finding| finding.code)
        .collect::<Vec<_>>();
    let idempotency_key = stable_uuid("change-set:publish-stop:publication");
    let request = PublishAcademicTermChangeSetRequest {
        effective_from: preview.effective_from,
        row_version: preview.change_set_row_version,
        target_delivery_version_row_version: preview.target_delivery_version_row_version,
        preview_hash: preview.preview_hash.clone(),
        acknowledged_warning_codes: warning_codes,
        idempotency_key,
    };
    let published =
        change_sets::publish_change_set(&pool, context.teacher_id, changed.id, request.clone())
            .await
            .expect("a ready stop change must publish atomically");
    assert_eq!(published.status, AcademicTermChangeSetStatus::Published);

    let (ends_on, stop_reason, stopped_by, stop_change_set_id): (
        Option<NaiveDate>,
        Option<String>,
        Option<Uuid>,
        Option<Uuid>,
    ) = sqlx::query_as(
        r#"SELECT ends_on, stop_reason, stopped_by, stop_change_set_id
           FROM learning_offerings WHERE id = $1"#,
    )
    .bind(offering_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        ends_on,
        Some(
            changed
                .reference_date
                .checked_sub_signed(chrono::Duration::days(1))
                .unwrap()
        )
    );
    assert_eq!(stop_reason.as_deref(), Some(changed.reason.as_str()));
    assert_eq!(stopped_by, Some(context.teacher_id));
    assert_eq!(stop_change_set_id, Some(changed.id));
    let offering = offerings::get(&pool, offering_id)
        .await
        .expect("published availability must be exposed through the offering DTO");
    assert!(offering.starts_on.is_some());
    assert_eq!(offering.ends_on, ends_on);
    assert_eq!(offering.stop_reason, stop_reason);
    let target_status: String =
        sqlx::query_scalar("SELECT status FROM academic_delivery_versions WHERE id = $1")
            .bind(changed.target_delivery_version_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(target_status, "published");
    let base_status: String =
        sqlx::query_scalar("SELECT status FROM academic_delivery_versions WHERE id = $1")
            .bind(changed.base_delivery_version_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(base_status, "published");

    let retried =
        change_sets::publish_change_set(&pool, context.teacher_id, changed.id, request.clone())
            .await
            .expect("the same publication request must be idempotent");
    assert_eq!(retried.id, published.id);
    assert_eq!(retried.row_version, published.row_version);
    let mut changed_retry = request;
    changed_retry.acknowledged_warning_codes =
        vec![AcademicChangeFindingCode::MissingDeliveryGroup];
    let conflicting_retry =
        change_sets::publish_change_set(&pool, context.teacher_id, changed.id, changed_retry)
            .await
            .expect_err("the publication key cannot be reused with a different request");
    assert!(matches!(conflicting_retry, AppError::Conflict(_)));
}

#[tokio::test]
async fn timetable_period_excess_is_blocking_and_cannot_be_acknowledged_as_a_delivery_warning() {
    let pool = prepare_delivery_runtime_fixture("academic_table_period_excess").await;
    let context = operational_change_runtime_context(&pool).await;
    let draft = independent_table_draft(&pool, &context).await;
    fill_change_set_target_deficits(&pool, context.teacher_id, context.term_id, draft.id).await;
    let opening =
        school_academic_delivery::services::versions::get_version(&pool, draft.delivery_version_id)
            .await
            .unwrap();
    let offering = opening
        .snapshot
        .offerings
        .iter()
        .find(|offering| {
            matches!(&offering.catalog, LearningOfferingSnapshot::Course(_))
                && !offering.groups.is_empty()
        })
        .unwrap();
    let group = &offering.groups[0];
    let slots: Vec<(String,Uuid)>=sqlx::query_as("SELECT day.code,period.id FROM academic_terms term JOIN bell_schedule_periods period ON period.bell_schedule_id=term.bell_schedule_id CROSS JOIN (VALUES('MON'),('TUE'),('WED'),('THU'),('FRI')) day(code) WHERE term.id=$1 AND period.is_active ORDER BY day.code,period.order_index").bind(context.term_id).fetch_all(&pool).await.unwrap();
    let mut added = false;
    for (day_of_week, bell_schedule_period_id) in slots {
        match timetable_block_service::create_ordinary_block(
            &pool,
            context.teacher_id,
            CreateOrdinaryTimetableBlockRequest {
                timetable_version_id: draft.id,
                academic_term_id: context.term_id,
                learning_group_id: group.id,
                day_of_week,
                bell_schedule_period_id,
                room_id: None,
                note: None,
                instructor_ids: group
                    .teachers
                    .iter()
                    .map(|teacher| teacher.teacher_id)
                    .collect(),
            },
        )
        .await
        {
            Ok(_) => {
                added = true;
                break;
            }
            Err(AppError::Conflict(_)) => {}
            Err(error) => panic!("unexpected extra lesson error: {error:?}"),
        }
    }
    assert!(added);
    let preview = table_preview(&pool, draft.id).await;
    assert!(!preview.can_publish);
    assert!(preview.findings.iter().any(|finding| finding.code==school_academic_timetable::models::timetable_publication::TimetablePublicationFindingCode::PeriodCountMismatch && finding.learning_group_id==Some(group.id)));
    let publication = school_academic_timetable::services::timetable_lifecycle::publish(
        &pool,
        context.teacher_id,
        draft.id,
        school_academic_timetable::models::timetable_publication::PublishTimetableVersionRequest {
            row_version: preview.row_version,
            effective_from: preview.effective_from,
            preview_hash: preview.preview_hash,
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(publication, AppError::ValidationError(_)));
    let rejected = serde_json::from_value::<
        school_academic_timetable::models::timetable_publication::PublishTimetableVersionRequest,
    >(
        serde_json::json!({"rowVersion":1,"effectiveFrom":"2027-01-01","previewHash":"0".repeat(64),"idempotencyKey":Uuid::new_v4(),"acknowledgedWarningCodes":["weekly_period_excess"]}),
    );
    assert!(rejected.is_err());
}

#[tokio::test]
async fn publication_rolls_back_every_write_when_the_final_change_set_write_fails() {
    let pool = prepare_delivery_runtime_fixture("academic_change_set_publish_rollback").await;
    let context = operational_change_runtime_context(&pool).await;
    let change_set = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        19,
        "change-set:publish-rollback:create",
    )
    .await;
    let offering_id: Uuid = sqlx::query_scalar(
        r#"SELECT learning_offering_id
           FROM (SELECT opening.id AS delivery_version_id,(entry->>'id')::uuid AS learning_offering_id,(entry->>'weeklyPeriodTarget')::integer AS weekly_period_target FROM academic_delivery_versions opening CROSS JOIN LATERAL jsonb_array_elements(opening.snapshot->'offerings') entry)
           WHERE delivery_version_id = $1
           ORDER BY learning_offering_id LIMIT 1"#,
    )
    .bind(change_set.base_delivery_version_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let changed = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        UpsertAcademicTermChangeItemRequest::StopOffering {
            change_set_row_version: change_set.row_version,
            item_row_version: None,
            learning_offering_id: offering_id,
        },
    )
    .await
    .unwrap();
    let preview = publication_preview(&pool, changed.id).await.unwrap();
    assert!(!preview
        .findings
        .iter()
        .any(|finding| finding.severity == AcademicChangeFindingSeverity::Blocking));
    sqlx::raw_sql(
        r#"CREATE FUNCTION academic_test_fail_change_set_publication()
           RETURNS TRIGGER LANGUAGE plpgsql AS $$
           BEGIN
               IF NEW.status = 'published' THEN
                   RAISE EXCEPTION 'TEST_FINAL_CHANGE_SET_WRITE_FAILURE';
               END IF;
               RETURN NEW;
           END;
           $$;
           CREATE TRIGGER academic_test_fail_change_set_publication
           BEFORE UPDATE ON academic_term_change_sets
           FOR EACH ROW EXECUTE FUNCTION academic_test_fail_change_set_publication();"#,
    )
    .execute(&pool)
    .await
    .unwrap();
    let warning_codes = preview
        .findings
        .iter()
        .filter(|finding| finding.severity == AcademicChangeFindingSeverity::Warning)
        .map(|finding| finding.code)
        .collect();
    let failed = change_sets::publish_change_set(
        &pool,
        context.teacher_id,
        changed.id,
        PublishAcademicTermChangeSetRequest {
            effective_from: preview.effective_from,
            row_version: preview.change_set_row_version,
            target_delivery_version_row_version: preview.target_delivery_version_row_version,
            preview_hash: preview.preview_hash,
            acknowledged_warning_codes: warning_codes,
            idempotency_key: stable_uuid("change-set:publish-rollback:publication"),
        },
    )
    .await
    .expect_err("an injected final write failure must roll back the transaction");
    assert!(matches!(failed, AppError::DbError(_)));

    let offering_end: Option<NaiveDate> =
        sqlx::query_scalar("SELECT ends_on FROM learning_offerings WHERE id = $1")
            .bind(offering_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(offering_end, None);
    let target_status: String =
        sqlx::query_scalar("SELECT status FROM academic_delivery_versions WHERE id = $1")
            .bind(changed.target_delivery_version_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(target_status, "draft");
    assert_eq!(
        change_sets::get_change_set(&pool, changed.id)
            .await
            .unwrap()
            .status,
        AcademicTermChangeSetStatus::Draft
    );
}

#[tokio::test]
async fn publishing_an_added_course_preserves_the_separate_roster_publication() {
    let pool = prepare_delivery_runtime_fixture("academic_change_set_publish_added_course").await;
    let context = operational_change_runtime_context(&pool).await;
    let change_set = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        20,
        "change-set:publish-added-course:create",
    )
    .await;
    let (subject_version_id, _, _) = add_operational_change_catalog_fixture(&pool, &context).await;
    let CreateLearningOfferingRequest::Course(mut course) = course_request(&context) else {
        unreachable!();
    };
    course.subject_version_id = subject_version_id;
    let changed = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        UpsertAcademicTermChangeItemRequest::AddCourse {
            change_set_row_version: change_set.row_version,
            offering: course,
        },
    )
    .await
    .unwrap();
    let added_offering_id = changed
        .items
        .iter()
        .find_map(|item| match item {
            super::models::AcademicTermChangeItem::AddOffering {
                learning_offering_id,
                ..
            } => Some(*learning_offering_id),
            _ => None,
        })
        .expect("add item must expose the draft offering");
    let draft_group = groups::list(&pool, added_offering_id)
        .await
        .unwrap()
        .into_iter()
        .next()
        .expect("one homeroom target must create a draft group");
    let with_teacher = groups::replace_teachers(
        &pool,
        context.teacher_id,
        draft_group.id,
        ReplaceLearningGroupTeachersRequest {
            row_version: draft_group.row_version,
            teachers: vec![TeacherAssignmentInput {
                teacher_id: context.teacher_id,
                role: LearningTeacherRole::Primary,
            }],
        },
    )
    .await
    .unwrap();
    let roster_preview = groups::preview_roster(&pool, draft_group.id).await.unwrap();
    let prepared_group = groups::apply_roster(
        &pool,
        context.teacher_id,
        draft_group.id,
        ApplyRosterRequest {
            row_version: with_teacher.row_version,
            source_hash: roster_preview.source_hash,
            overrides: Vec::new(),
        },
    )
    .await
    .unwrap();
    assert_eq!(
        prepared_group.roster_status,
        super::models::RosterStatus::Draft
    );
    let preview = publication_preview(&pool, changed.id).await.unwrap();
    let blocking = preview
        .findings
        .iter()
        .filter(|finding| finding.severity == AcademicChangeFindingSeverity::Blocking)
        .collect::<Vec<_>>();
    assert!(blocking.is_empty(), "blocking findings: {blocking:#?}");
    let warning_codes = preview
        .findings
        .iter()
        .filter(|finding| finding.severity == AcademicChangeFindingSeverity::Warning)
        .map(|finding| finding.code)
        .collect();
    change_sets::publish_change_set(
        &pool,
        context.teacher_id,
        changed.id,
        PublishAcademicTermChangeSetRequest {
            effective_from: preview.effective_from,
            row_version: preview.change_set_row_version,
            target_delivery_version_row_version: preview.target_delivery_version_row_version,
            preview_hash: preview.preview_hash,
            acknowledged_warning_codes: warning_codes,
            idempotency_key: stable_uuid("change-set:publish-added-course:publication"),
        },
    )
    .await
    .expect("a fully prepared added course must publish atomically");

    let published_offering = offerings::get(&pool, added_offering_id).await.unwrap();
    assert_eq!(published_offering.status, LearningOfferingStatus::Published);
    let published_group = groups::get(&pool, draft_group.id).await.unwrap();
    assert_eq!(published_group.status, LearningOfferingStatus::Published);
    assert_eq!(
        published_group.roster_status,
        super::models::RosterStatus::Draft
    );
    assert!(published_group.teachers_locked);
    let unpublished_memberships: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM learning_group_students
           WHERE learning_group_id = $1 AND published_at IS NULL"#,
    )
    .bind(draft_group.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let membership_count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM learning_group_students WHERE learning_group_id=$1",
    )
    .bind(draft_group.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        unpublished_memberships, membership_count,
        "opening publication never publishes student memberships"
    );
    let roster = groups::publish_roster(
        &pool,
        context.teacher_id,
        draft_group.id,
        PublishRosterRequest {
            row_version: published_group.row_version,
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    assert_eq!(roster.roster_status, super::models::RosterStatus::Published);
}

#[tokio::test]
async fn publication_rejects_a_preview_after_a_resource_revision_changes() {
    let pool = prepare_delivery_runtime_fixture("academic_change_set_publish_stale_preview").await;
    let context = operational_change_runtime_context(&pool).await;
    let change_set = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        21,
        "change-set:publish-stale-preview:create",
    )
    .await;
    let offering_id: Uuid = sqlx::query_scalar(
        r#"SELECT learning_offering_id
           FROM (SELECT opening.id AS delivery_version_id,(entry->>'id')::uuid AS learning_offering_id,(entry->>'weeklyPeriodTarget')::integer AS weekly_period_target FROM academic_delivery_versions opening CROSS JOIN LATERAL jsonb_array_elements(opening.snapshot->'offerings') entry)
           WHERE delivery_version_id = $1
           ORDER BY learning_offering_id LIMIT 1"#,
    )
    .bind(change_set.base_delivery_version_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let changed = change_sets::upsert_change_item(
        &pool,
        context.teacher_id,
        change_set.id,
        UpsertAcademicTermChangeItemRequest::StopOffering {
            change_set_row_version: change_set.row_version,
            item_row_version: None,
            learning_offering_id: offering_id,
        },
    )
    .await
    .unwrap();
    let preview = publication_preview(&pool, changed.id).await.unwrap();
    assert!(!preview
        .findings
        .iter()
        .any(|finding| finding.severity == AcademicChangeFindingSeverity::Blocking));
    sqlx::query("UPDATE learning_offerings SET row_version = row_version + 1 WHERE id = $1")
        .bind(offering_id)
        .execute(&pool)
        .await
        .unwrap();
    let warning_codes = preview
        .findings
        .iter()
        .filter(|finding| finding.severity == AcademicChangeFindingSeverity::Warning)
        .map(|finding| finding.code)
        .collect();
    let stale = change_sets::publish_change_set(
        &pool,
        context.teacher_id,
        changed.id,
        PublishAcademicTermChangeSetRequest {
            effective_from: preview.effective_from,
            row_version: preview.change_set_row_version,
            target_delivery_version_row_version: preview.target_delivery_version_row_version,
            preview_hash: preview.preview_hash,
            acknowledged_warning_codes: warning_codes,
            idempotency_key: stable_uuid("change-set:publish-stale-preview:publication"),
        },
    )
    .await
    .expect_err("resource drift after preview must block publication");
    assert!(matches!(stale, AppError::Conflict(_)));
    let stopped_end: Option<NaiveDate> =
        sqlx::query_scalar("SELECT ends_on FROM learning_offerings WHERE id = $1")
            .bind(offering_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(stopped_end, None);
    assert_eq!(
        change_sets::get_change_set(&pool, changed.id)
            .await
            .unwrap()
            .status,
        AcademicTermChangeSetStatus::Draft
    );
}

fn course_request(context: &RuntimeContext) -> CreateLearningOfferingRequest {
    CreateLearningOfferingRequest::Course(CreateCourseOfferingRequest {
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
        "subjectVersionId": Uuid::new_v4(),
        "curriculumCourseRequirementId": null,
        "targets": [],
        "assessmentTotalScore": "120.50"
    });
    assert!(serde_json::from_value::<CreateCourseOfferingRequest>(base.clone()).is_ok());
    let mut legacy = base;
    legacy["gradingPolicy"] = serde_json::json!({"policyCode": "school_default", "totalScore": "100.00", "passingScore": "50.00"});
    assert!(serde_json::from_value::<CreateCourseOfferingRequest>(legacy).is_err());
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
async fn manual_offering_creation_adds_the_target_to_an_existing_draft() {
    let pool = prepare_delivery_runtime_fixture("academic_opening_manual_target").await;
    let context = planning_runtime_context(&pool).await;
    let created = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        7,
        "opening:manual-target",
    )
    .await;
    let offering = offerings::create_for_delivery(
        &pool,
        context.teacher_id,
        Some(created.target_delivery_version_id),
        course_request(&context),
    )
    .await
    .unwrap();
    let target = school_academic_delivery::services::versions::get_version(
        &pool,
        created.target_delivery_version_id,
    )
    .await
    .unwrap();
    let captured = target
        .snapshot
        .offerings
        .iter()
        .find(|item| item.id == offering.id)
        .unwrap();
    assert_eq!(captured.weekly_period_target, 3);
    let journal = change_sets::get_change_set(&pool, created.id)
        .await
        .unwrap();
    assert!(journal.items.iter().any(|item| matches!(item, super::models::AcademicTermChangeItem::AddOffering { learning_offering_id,.. } if *learning_offering_id==offering.id)));
    assert_eq!(
        target.status,
        school_academic_delivery::models::versions::DeliveryVersionStatus::Draft
    );
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
           ORDER BY start_date
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
        "subjectVersionId": Uuid::nil(),
        "targets": [],
        "assessmentTotalScore": "100.00"
    }))
    .unwrap();
    assert!(matches!(parsed, CreateLearningOfferingRequest::Course(_)));

    let wrong_subtype =
        serde_json::from_value::<CreateLearningOfferingRequest>(serde_json::json!({
            "kind": "course",
            "academicTermId": Uuid::nil(),
            "activityVersionId": Uuid::nil(),
            "targets": [],
            "assessmentTotalScore": "100.00"
        }));
    assert!(wrong_subtype.is_err());
}

#[tokio::test]
async fn offering_group_and_roster_publish_are_revisioned_and_idempotent() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_runtime_publish").await;
    let context = planning_runtime_context(&pool).await;

    let migrated_published_course_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM learning_offerings WHERE kind = 'course' AND status = 'published' \
         AND migration_provenance <> '{}'::jsonb ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let immutable_after_normalization = sqlx::query(
        "UPDATE course_offering_details SET credit = credit + 1 \
         WHERE learning_offering_id = $1",
    )
    .bind(migrated_published_course_id)
    .execute(&pool)
    .await
    .expect_err("migration 044 must restore published snapshot immutability");
    assert!(immutable_after_normalization
        .to_string()
        .contains("ACADEMIC_CORE_PUBLISHED_OFFERING_IMMUTABLE"));

    let ineffective_subject_version_id: Uuid = sqlx::query_scalar(
        r#"SELECT version.id
           FROM subject_versions version
           JOIN academic_terms term ON term.id = $1
           WHERE version.status = 'published'
             AND (term.start_date < version.effective_from
                  OR (version.effective_until IS NOT NULL
                      AND term.start_date >= version.effective_until))
           ORDER BY version.id
           LIMIT 1"#,
    )
    .bind(context.term_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let mut ineffective_version_request = course_request(&context);
    let CreateLearningOfferingRequest::Course(course) = &mut ineffective_version_request else {
        unreachable!();
    };
    course.subject_version_id = ineffective_subject_version_id;
    let ineffective_version =
        offerings::create(&pool, context.teacher_id, ineffective_version_request).await;
    assert!(matches!(
        ineffective_version,
        Err(AppError::ValidationError(_))
    ));

    let offering = offerings::create(&pool, context.teacher_id, course_request(&context))
        .await
        .unwrap();
    assert_eq!(offering.kind, LearningOfferingKind::Course);
    assert_eq!(offering.status, LearningOfferingStatus::Draft);
    assert_eq!(offering.academic_year_id, context.year_id);
    let expected_owner_id: Uuid = sqlx::query_scalar(
        r#"SELECT subject.owning_organization_unit_id
           FROM subject_versions version
           JOIN subjects subject ON subject.id = version.subject_id
           WHERE version.id = $1"#,
    )
    .bind(context.subject_version_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(offering.owning_organization_unit_id, expected_owner_id);

    let group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            name: "กลุ่มทดสอบ".to_string(),
            description: None,
            capacity: Some(40),
            preferred_room_ids: Vec::new(),
        },
    )
    .await
    .unwrap();

    let missing_primary_teacher = offerings::publish(
        &pool,
        context.teacher_id,
        offering.id,
        PublishLearningOfferingRequest {
            row_version: offering.row_version,
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await;
    assert!(matches!(
        missing_primary_teacher,
        Err(AppError::ValidationError(_))
    ));

    let duplicate_name = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            name: "  กลุ่มทดสอบ  ".to_string(),
            description: None,
            capacity: None,
            preferred_room_ids: Vec::new(),
        },
    )
    .await;
    assert!(matches!(duplicate_name, Err(AppError::Conflict(_))));

    let duplicate_teacher = groups::replace_teachers(
        &pool,
        context.teacher_id,
        group.id,
        ReplaceLearningGroupTeachersRequest {
            row_version: group.row_version,
            teachers: vec![
                TeacherAssignmentInput {
                    teacher_id: context.teacher_id,
                    role: LearningTeacherRole::Primary,
                },
                TeacherAssignmentInput {
                    teacher_id: context.teacher_id,
                    role: LearningTeacherRole::Secondary,
                },
            ],
        },
    )
    .await;
    assert!(matches!(
        duplicate_teacher,
        Err(AppError::ValidationError(_))
    ));

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
    assert!(!group.teachers_locked);
    let assignment = group
        .teacher_assignments
        .first()
        .expect("teacher replacement must return the created episode");
    let (
        offering_starts_on,
        stored_starts_on,
        stored_ends_on,
        stored_row_version,
        created_by,
        updated_by,
    ): (NaiveDate, NaiveDate, Option<NaiveDate>, i64, Uuid, Uuid) = sqlx::query_as(
        r#"SELECT offering.starts_on, teacher.starts_on, teacher.ends_on,
                  teacher.row_version, teacher.created_by, teacher.updated_by
           FROM learning_group_teachers teacher
           JOIN learning_groups learning_group ON learning_group.id = teacher.learning_group_id
           JOIN learning_offerings offering ON offering.id = learning_group.learning_offering_id
           WHERE teacher.id = $1"#,
    )
    .bind(assignment.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(assignment.teacher_id, context.teacher_id);
    assert_eq!(assignment.starts_on, offering_starts_on);
    assert_eq!(assignment.ends_on, None);
    assert_eq!(assignment.row_version, 1);
    assert_eq!(stored_starts_on, offering_starts_on);
    assert_eq!(stored_ends_on, None);
    assert_eq!(stored_row_version, 1);
    assert_eq!(created_by, context.teacher_id);
    assert_eq!(updated_by, context.teacher_id);

    let wrong_year_homeroom_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM homerooms WHERE academic_year_id <> $1 ORDER BY id LIMIT 1",
    )
    .bind(context.year_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let wrong_year_homeroom = groups::replace_homerooms(
        &pool,
        context.teacher_id,
        group.id,
        ReplaceLearningGroupHomeroomsRequest {
            row_version: group.row_version,
            homeroom_ids: vec![wrong_year_homeroom_id],
        },
    )
    .await;
    assert!(matches!(
        wrong_year_homeroom,
        Err(AppError::ValidationError(_))
    ));

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

    let published = offerings::publish(
        &pool,
        context.teacher_id,
        offering.id,
        PublishLearningOfferingRequest {
            row_version: offering.row_version,
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    assert_eq!(published.status, LearningOfferingStatus::Published);

    let group = groups::get(&pool, group.id).await.unwrap();
    assert!(group.teachers_locked);
    let teachers_before: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
        r#"SELECT id, teacher_id, role::text
           FROM learning_group_teachers
           WHERE learning_group_id = $1
           ORDER BY id"#,
    )
    .bind(group.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    let locked_replacement = groups::replace_teachers(
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
    .await;
    assert!(
        matches!(locked_replacement, Err(AppError::Conflict(message)) if message == "เผยแพร่กลุ่มเรียนแล้ว ไม่สามารถเปลี่ยนครูผู้สอนได้")
    );
    let teachers_after: Vec<(Uuid, Uuid, String)> = sqlx::query_as(
        r#"SELECT id, teacher_id, role::text
           FROM learning_group_teachers
           WHERE learning_group_id = $1
           ORDER BY id"#,
    )
    .bind(group.id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(teachers_after, teachers_before);
    assert_eq!(
        groups::get(&pool, group.id).await.unwrap().row_version,
        group.row_version
    );
    let preview = groups::preview_roster(&pool, group.id).await.unwrap();
    assert!(!preview.source_hash.is_empty());
    assert!(preview.added > 0);
    let group = groups::apply_roster(
        &pool,
        context.teacher_id,
        group.id,
        ApplyRosterRequest {
            row_version: group.row_version,
            source_hash: preview.source_hash,
            overrides: Vec::new(),
        },
    )
    .await
    .unwrap();

    let stale_publish = groups::publish_roster(
        &pool,
        context.teacher_id,
        group.id,
        PublishRosterRequest {
            row_version: group.row_version - 1,
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await;
    assert!(matches!(stale_publish, Err(AppError::Conflict(_))));

    let idempotency_key = Uuid::new_v4();
    let published_group = groups::publish_roster(
        &pool,
        context.teacher_id,
        group.id,
        PublishRosterRequest {
            row_version: group.row_version,
            idempotency_key,
        },
    )
    .await
    .unwrap();
    let retried = groups::publish_roster(
        &pool,
        context.teacher_id,
        group.id,
        PublishRosterRequest {
            row_version: group.row_version,
            idempotency_key,
        },
    )
    .await
    .unwrap();
    assert_eq!(retried.id, published_group.id);
    assert_eq!(retried.row_version, published_group.row_version);

    let republished_with_new_key = groups::publish_roster(
        &pool,
        context.teacher_id,
        group.id,
        PublishRosterRequest {
            row_version: published_group.row_version,
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await;
    assert!(matches!(
        republished_with_new_key,
        Err(AppError::Conflict(_))
    ));
}

// Catches publish_roster acquiring the group before the offering while roster mutations acquire
// the offering before the group, which lets the two transactions form a PostgreSQL deadlock.
#[tokio::test]
async fn roster_publication_and_apply_serialize_in_canonical_lock_order() {
    let pool =
        prepare_concurrent_delivery_runtime_fixture("academic_roster_publish_apply_lock_order")
            .await;
    let context = planning_runtime_context(&pool).await;
    let offering = offerings::create(&pool, context.teacher_id, course_request(&context))
        .await
        .unwrap();
    let group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            name: "ทดสอบลำดับล็อก roster".to_string(),
            description: None,
            capacity: Some(40),
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
    offerings::publish(
        &pool,
        context.teacher_id,
        offering.id,
        PublishLearningOfferingRequest {
            row_version: offering.row_version,
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let group = groups::get(&pool, group.id).await.unwrap();
    let preview = groups::preview_roster(&pool, group.id).await.unwrap();
    let group = groups::apply_roster(
        &pool,
        context.teacher_id,
        group.id,
        ApplyRosterRequest {
            row_version: group.row_version,
            source_hash: preview.source_hash.clone(),
            overrides: Vec::new(),
        },
    )
    .await
    .unwrap();

    let mut offering_blocker = pool.begin().await.unwrap();
    sqlx::query("SELECT id FROM learning_offerings WHERE id = $1 FOR UPDATE")
        .bind(offering.id)
        .execute(&mut *offering_blocker)
        .await
        .unwrap();

    let apply_pool = pool.clone();
    let source_hash = preview.source_hash;
    let group_id = group.id;
    let actor_id = context.teacher_id;
    let row_version = group.row_version;
    let apply = tokio::spawn(async move {
        groups::apply_roster(
            &apply_pool,
            actor_id,
            group_id,
            ApplyRosterRequest {
                row_version,
                source_hash,
                overrides: Vec::new(),
            },
        )
        .await
    });
    wait_for_offering_lock_waiters(&pool, 1).await;

    let publish_pool = pool.clone();
    let publish = tokio::spawn(async move {
        groups::publish_roster(
            &publish_pool,
            actor_id,
            group_id,
            PublishRosterRequest {
                row_version,
                idempotency_key: Uuid::new_v4(),
            },
        )
        .await
    });
    wait_for_offering_lock_waiters(&pool, 2).await;
    offering_blocker.commit().await.unwrap();

    let (apply_result, publish_result) =
        tokio::time::timeout(std::time::Duration::from_secs(15), async {
            tokio::join!(apply, publish)
        })
        .await
        .expect("competing roster mutations must finish without a database deadlock");
    let applied = apply_result
        .unwrap()
        .expect("the first offering-lock waiter must apply the roster");
    assert_eq!(applied.row_version, row_version + 1);
    let publish_result = publish_result.unwrap();
    assert!(
        matches!(publish_result, Err(AppError::Conflict(_))),
        "the serialized stale publication must return an optimistic conflict, got {publish_result:?}"
    );
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
           WHERE former_ending_year.end_date < selected_year.start_date
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
        matches!(&earlier_edition, Err(AppError::ValidationError(message))
            if message == "เวอร์ชันนี้ไม่มีผลในภาคเรียนที่เลือก"),
        "the earlier plan passes selection, while its expired catalog version remains guarded: {earlier_edition:?}"
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
    assert!(!preview.source_hash.is_empty());
    assert!(!preview.proposals.is_empty());
    assert!(preview
        .proposals
        .iter()
        .any(|proposal| !proposal.default_groups.is_empty()));
    let choices = default_preparation_choices(&preview);

    let mismatched = offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
            delivery_version_id: None,
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
            delivery_version_id: None,
            source_hash: preview.source_hash.clone(),
            idempotency_key,
            choices: choices.clone(),
        },
    )
    .await
    .unwrap();
    let retried = offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
            delivery_version_id: None,
            source_hash: preview.source_hash.clone(),
            idempotency_key,
            choices,
        },
    )
    .await
    .unwrap();
    assert_eq!(retried.offering_ids, applied.offering_ids);
    assert_eq!(retried.group_ids, applied.group_ids);
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
    let retained = offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
            delivery_version_id: None,
            source_hash: retained_preview.source_hash,
            idempotency_key: Uuid::new_v4(),
            choices: retained_choices,
        },
    )
    .await
    .unwrap();
    assert_eq!(retained.created_offering_count, 0);
    assert_eq!(retained.created_group_count, 0);
    assert_eq!(retained.offering_ids, applied.offering_ids);

    let closed_term_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM academic_terms WHERE status IN ('closed', 'cancelled') ORDER BY start_date LIMIT 1",
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
            delivery_version_id: None,
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
            delivery_version_id: None,
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
             AND version.effective_from <= term.start_date
             AND (version.effective_until IS NULL OR version.effective_until > term.start_date)
           ORDER BY version.id
           LIMIT 1"#,
    )
    .bind(context.term_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let offering = offerings::create(
        &pool,
        context.teacher_id,
        CreateLearningOfferingRequest::Activity(CreateActivityOfferingRequest {
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
    offerings::publish(
        &pool,
        context.teacher_id,
        offering.id,
        PublishLearningOfferingRequest {
            row_version: offering.row_version,
            idempotency_key: Uuid::new_v4(),
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
    groups::publish_roster(
        &pool,
        context.teacher_id,
        group.id,
        PublishRosterRequest {
            row_version: group.row_version,
            idempotency_key: Uuid::new_v4(),
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
async fn student_activity_registration_is_term_scoped_eligible_and_revisioned() {
    let pool =
        prepare_delivery_runtime_fixture("academic_delivery_student_activity_registration").await;
    let context = planning_runtime_context(&pool).await;
    sqlx::query(
        "UPDATE academic_terms SET planned_end_date = NULL, closed_on = NULL WHERE id = $1",
    )
    .bind(context.term_id)
    .execute(&pool)
    .await
    .expect("a planning term may omit its planned and actual end dates");
    let student_id = Uuid::parse_str("50000000-0000-0000-0000-000000000001").unwrap();
    let (activity_version_id, scheduling_mode): (Uuid, ActivitySchedulingMode) = sqlx::query_as(
        r#"SELECT version.id, version.scheduling_mode
           FROM activity_versions version
           JOIN academic_terms term ON term.id = $1
           WHERE version.status = 'published'
             AND version.effective_from <= term.start_date
             AND (version.effective_until IS NULL OR version.effective_until > term.start_date)
           ORDER BY version.id
           LIMIT 1"#,
    )
    .bind(context.term_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let offering = offerings::create(
        &pool,
        context.teacher_id,
        CreateLearningOfferingRequest::Activity(CreateActivityOfferingRequest {
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
            name: "ชุมนุมดาราศาสตร์".to_string(),
            description: Some("เรียนรู้ท้องฟ้าผ่านการสังเกตจริง".to_string()),
            capacity: Some(1),
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

    offerings::publish(
        &pool,
        context.teacher_id,
        offering.id,
        PublishLearningOfferingRequest {
            row_version: offering.row_version,
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();

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
    assert!(values
        .iter()
        .any(|offering| matches!(&offering.snapshot, LearningOfferingSnapshot::Course(_))));
    assert!(values
        .iter()
        .any(|offering| matches!(&offering.snapshot, LearningOfferingSnapshot::Activity(_))));
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
    let published_offering = offerings::publish(
        &pool,
        context.teacher_id,
        offering.id,
        PublishLearningOfferingRequest {
            row_version: offering.row_version,
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();

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
    groups::publish_roster(
        &pool,
        context.teacher_id,
        first_group.id,
        PublishRosterRequest {
            row_version: first_group.row_version,
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
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
        .find(|item| item.offering.id == published_offering.id)
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
    assert_eq!(summary.published_roster_count, 1);

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
    assert!(!outside_scope
        .offerings
        .iter()
        .any(|item| item.offering.id == published_offering.id));
}

#[tokio::test]
async fn homeroom_delivery_workspace_maps_curriculum_offerings_and_group_coverage() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_homeroom_workspace").await;
    let context = planning_runtime_context(&pool).await;
    let revision = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        0,
        "homeroom:opening",
    )
    .await;
    let draft_version_id = revision.target_delivery_version_id;
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
            delivery_version_id: Some(draft_version_id),
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
    let opening =
        school_academic_delivery::services::versions::get_version(&pool, draft_version_id)
            .await
            .unwrap();
    assert!(applied.offering_ids.iter().all(|id| opening
        .snapshot
        .offerings
        .iter()
        .any(|item| item.id == *id)));

    let filter = AcademicResourceListFilter {
        includes_school_owned: true,
        ..Default::default()
    };
    let before = workspaces::homeroom_delivery_workspace_for_version(
        &pool,
        context.year_id,
        context.term_id,
        None,
        &filter,
    )
    .await
    .expect("homeroom workspace should load");
    assert_eq!(before.delivery_version_id, Some(draft_version_id));
    assert_eq!(
        before.delivery_version_status,
        Some(school_academic_delivery::models::versions::DeliveryVersionStatus::Draft)
    );
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
    assert!(room
        .items
        .iter()
        .filter(|item| item.group_mode == super::models::HomeroomGroupMode::Central)
        .all(|item| item.teacher_state == super::models::HomeroomTeacherState::Deferred));
    assert!(room.items.iter().all(|item| match item.resource_kind {
        LearningOfferingKind::Course => {
            item.standard_periods_per_week
                .is_some_and(|value| value > 0)
                && item.scheduling_mode.is_none()
        }
        LearningOfferingKind::Activity => {
            item.standard_periods_per_week.is_none() && item.scheduling_mode.is_some()
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

    let after = workspaces::homeroom_delivery_workspace_for_version(
        &pool,
        context.year_id,
        context.term_id,
        None,
        &filter,
    )
    .await
    .expect("homeroom workspace should refresh");
    let room = after
        .homerooms
        .iter()
        .find(|room| room.homeroom.id == context.homeroom_id)
        .unwrap();
    assert_eq!(room.ready_count, central_count + 1);
    assert!(room
        .items
        .iter()
        .any(|item| item.offering_id == Some(offering_id) && item.groups.len() == 1));
}

#[tokio::test]
async fn active_homeroom_workspace_projects_the_effective_version_targets() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_workspace_version_target").await;
    let (year_id, term_id): (Uuid, Uuid) = sqlx::query_as(
        r#"SELECT academic_year_id, id
           FROM academic_terms
           WHERE status = 'active'
           ORDER BY start_date, id LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let workspace = workspaces::homeroom_delivery_workspace_for_version(
        &pool,
        year_id,
        term_id,
        None,
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert!(workspace.delivery_version_id.is_some());
    assert_eq!(
        workspace.delivery_version_status,
        Some(school_academic_delivery::models::versions::DeliveryVersionStatus::Published)
    );
    assert!(workspace
        .homerooms
        .iter()
        .flat_map(|room| &room.items)
        .filter(|item| item.offering_id.is_some())
        .any(|item| item.weekly_period_target.is_some_and(|target| target > 0)));
}

#[tokio::test]
async fn homeroom_alignment_uses_the_explicit_delivery_version_target() {
    let pool =
        prepare_delivery_runtime_fixture("academic_delivery_workspace_alignment_version").await;
    let (year_id, term_id): (Uuid, Uuid) = sqlx::query_as(
        r#"SELECT academic_year_id, id
           FROM academic_terms
           WHERE status = 'active'
           ORDER BY start_date, id LIMIT 1"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let actor_id: Uuid = sqlx::query_scalar(
        "SELECT id FROM users WHERE user_type='staff' AND status='active' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE academic_terms SET status='planning' WHERE id=$1")
        .bind(term_id)
        .execute(&pool)
        .await
        .unwrap();
    let revision =
        create_runtime_change_set(&pool, actor_id, term_id, 1, "alignment:opening").await;
    let opening = school_academic_delivery::services::versions::get_version(
        &pool,
        revision.target_delivery_version_id,
    )
    .await
    .unwrap();
    let offering=opening.snapshot.offerings.iter().find(|offering| matches!(&offering.catalog,LearningOfferingSnapshot::Course(course) if course.standard_periods_per_week > 0)).unwrap();
    let offering_id = offering.id;
    let standard_periods = match &offering.catalog {
        LearningOfferingSnapshot::Course(course) => course.standard_periods_per_week,
        _ => unreachable!(),
    };
    change_sets::upsert_change_item(
        &pool,
        actor_id,
        revision.id,
        UpsertAcademicTermChangeItemRequest::AdjustWeeklyPeriodTarget {
            change_set_row_version: revision.row_version,
            item_row_version: None,
            learning_offering_id: offering_id,
            weekly_period_target: standard_periods + 1,
        },
    )
    .await
    .unwrap();

    let workspace = workspaces::homeroom_delivery_workspace_for_version(
        &pool,
        year_id,
        term_id,
        Some(opening.id),
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .unwrap();

    assert_eq!(workspace.delivery_version_id, Some(opening.id));
    assert_eq!(
        workspace.delivery_version_effective_from,
        opening.effective_from
    );
    let aligned = workspace
        .homerooms
        .iter()
        .flat_map(|room| &room.items)
        .find(|item| item.offering_id == Some(offering_id))
        .expect("the selected offering should align to a curriculum requirement");
    assert_eq!(aligned.weekly_period_target, Some(standard_periods + 1));
    assert!(aligned
        .alignment_states
        .contains(&CurriculumDeliveryAlignmentState::OperationalPeriodsDiffer));
    assert!(!aligned
        .alignment_states
        .contains(&CurriculumDeliveryAlignmentState::MatchesCurriculum));

    let other_version_id: Uuid = sqlx::query_scalar(
        r#"SELECT id
           FROM academic_delivery_versions
           WHERE academic_term_id <> $1
           ORDER BY id LIMIT 1"#,
    )
    .bind(term_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let error = workspaces::homeroom_delivery_workspace_for_version(
        &pool,
        year_id,
        term_id,
        Some(other_version_id),
        &AcademicResourceListFilter {
            includes_school_owned: true,
            ..Default::default()
        },
    )
    .await
    .expect_err("an opening version from another term must be rejected");
    assert!(matches!(error, AppError::ValidationError(_)));
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
             AND version.effective_from <= term.start_date
             AND (version.effective_until IS NULL OR version.effective_until > term.start_date)
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
    .fetch_one(&pool)
    .await
    .expect("fixture must expose a published catalog version outside this program term");
    let revision = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        0,
        "alignment:extra",
    )
    .await;
    let extra = offerings::create_for_delivery(
        &pool,
        context.teacher_id,
        Some(revision.target_delivery_version_id),
        course_request(&context),
    )
    .await
    .unwrap();
    let workspace = workspaces::homeroom_delivery_workspace_for_version(
        &pool,
        context.year_id,
        context.term_id,
        Some(revision.target_delivery_version_id),
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
    assert!(room.items.iter().any(|item| item
        .alignment_states
        .contains(&CurriculumDeliveryAlignmentState::CurriculumRequirementNotOffered)));
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
    let context = planning_runtime_context(&pool).await;
    let offering = offerings::create(&pool, context.teacher_id, course_request(&context))
        .await
        .expect("offering should be created for management options");
    let group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
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
    )
    .await
    .expect("management options should load");

    assert_eq!(options.academic_term_id, context.term_id);
    assert_eq!(options.academic_year_id, context.year_id);
    assert!(options
        .catalog_versions
        .iter()
        .any(|item| item.id == context.subject_version_id
            && item.label.contains(&item.code)
            && item.label.contains(&item.name)
            && item
                .standard_periods_per_week
                .is_some_and(|periods| periods > 0)));
    assert!(options
        .catalog_versions
        .iter()
        .filter(|item| item.kind == LearningOfferingKind::Activity)
        .all(|item| item.standard_periods_per_week.is_none()));
    assert!(options
        .grade_levels
        .iter()
        .any(|item| item.id == context.grade_level_id && !item.name.is_empty()));
    assert!(options
        .study_programs
        .iter()
        .any(|item| item.id == context.study_program_id && !item.name.is_empty()));
    assert!(options
        .homerooms
        .iter()
        .any(|item| item.id == context.homeroom_id && item.grade_level.is_some()));
    assert!(options
        .teachers
        .iter()
        .any(|item| item.id == context.teacher_id && !item.name.trim().is_empty()));
    assert!(options
        .learning_groups
        .iter()
        .any(|item| item.id == group.id && item.learning_offering_id == offering.id));
    assert!(options.rooms.iter().all(|item| !item.name_th.is_empty()));

    let scoped = workspaces::delivery_management_options(
        &pool,
        context.term_id,
        &AcademicResourceListFilter {
            organization_unit_ids: vec![context.owner_id],
            ..Default::default()
        },
    )
    .await
    .expect("organization-scoped options should load");
    assert!(scoped
        .catalog_versions
        .iter()
        .all(|item| item.id == context.subject_version_id));
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
    assert!(student
        .homeroom_name
        .as_deref()
        .is_some_and(|name| !name.is_empty()));
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
    assert!(renamed
        .students
        .iter()
        .any(|item| item.display_name.contains("ชื่อใหม่")));
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
             AND version.effective_from <= term.start_date
             AND (version.effective_until IS NULL OR version.effective_until > term.start_date)
           ORDER BY version.id
           LIMIT 1"#,
    )
    .bind(context.term_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let activity_offering = offerings::create(
        &pool,
        context.teacher_id,
        CreateLearningOfferingRequest::Activity(CreateActivityOfferingRequest {
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
    assert!(!organization_unit
        .iter()
        .any(|group| group.id == course_group_a.id));
    assert!(organization_unit
        .iter()
        .any(|group| group.id == activity_group.id));

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
async fn first_timetable_edit_requires_published_opening_and_resumes_its_initial_draft() {
    use school_academic_timetable::models::timetable_version::CreateTimetableVersionRequest;
    use school_academic_timetable::services::timetable_version_service::create_initial_draft;
    let pool = prepare_delivery_runtime_fixture("academic_first_table").await;
    let context = planning_runtime_context(&pool).await;
    let request = CreateTimetableVersionRequest {
        academic_term_id: context.term_id,
    };
    assert!(
        create_initial_draft(&pool, context.teacher_id, request.clone())
            .await
            .is_err()
    );
    let revision = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        0,
        "first:opening",
    )
    .await;
    let offering = offerings::create_for_delivery(
        &pool,
        context.teacher_id,
        Some(revision.target_delivery_version_id),
        course_request(&context),
    )
    .await
    .unwrap();
    let group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            name: "กลุ่มเปิดสอนก่อนจัดตาราง".into(),
            description: None,
            capacity: None,
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
    groups::replace_teachers(
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
    let preview = publication_preview(&pool, revision.id).await.unwrap();
    assert!(
        !preview
            .findings
            .iter()
            .any(|finding| finding.severity == AcademicChangeFindingSeverity::Blocking),
        "{:?}",
        preview.findings
    );
    change_sets::publish_change_set(
        &pool,
        context.teacher_id,
        revision.id,
        PublishAcademicTermChangeSetRequest {
            effective_from: preview.effective_from,
            row_version: preview.change_set_row_version,
            target_delivery_version_row_version: preview.target_delivery_version_row_version,
            preview_hash: preview.preview_hash,
            acknowledged_warning_codes: Vec::new(),
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_versions WHERE academic_term_id=$1",
    )
    .bind(context.term_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 0);
    let table = create_initial_draft(&pool, context.teacher_id, request.clone())
        .await
        .unwrap();
    assert_eq!(
        table.delivery_version_id,
        revision.target_delivery_version_id
    );
    assert_eq!(table.effective_from, None);
    assert_eq!(table.source_version_id, None);
    assert_eq!(
        create_initial_draft(&pool, context.teacher_id, request)
            .await
            .unwrap()
            .id,
        table.id
    );
}

#[tokio::test]
async fn timetable_draft_deletion_serializes_with_autosave_without_partial_children() {
    use school_academic_timetable::{
        models::{
            timetable_block::UpdateTimetableBlockRequest,
            timetable_version::DeleteTimetableDraftRequest,
        },
        services::timetable_lifecycle,
    };
    let pool = prepare_concurrent_delivery_runtime_fixture("table_delete_save_race").await;
    let context = operational_change_runtime_context(&pool).await;
    let draft = independent_table_draft(&pool, &context).await;
    fill_change_set_target_deficits(&pool, context.teacher_id, context.term_id, draft.id).await;
    let block_id:Uuid=sqlx::query_scalar("SELECT id FROM academic_timetable_blocks WHERE timetable_version_id=$1 AND is_active ORDER BY id LIMIT 1").bind(draft.id).fetch_one(&pool).await.unwrap();
    let block = timetable_block_service::get_block(&pool, block_id)
        .await
        .unwrap();
    let current = timetable_version_service::get_version(&pool, draft.id, Utc::now().date_naive())
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_blocks WHERE timetable_version_id=$1",
    )
    .bind(draft.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (saved, deleted) = tokio::join!(
        timetable_block_service::update_block(
            &pool,
            context.teacher_id,
            block.id,
            UpdateTimetableBlockRequest {
                timetable_version_id: draft.id,
                row_version: block.row_version,
                day_of_week: None,
                bell_schedule_period_id: None,
                title: None,
                clear_title: false,
                note: Some("บันทึกแข่งกับลบ".into()),
                clear_note: false,
                room_id: None,
                clear_room: false,
                instructor_ids: None,
                teacher_ids: None
            }
        ),
        timetable_lifecycle::delete_draft(
            &pool,
            context.teacher_id,
            draft.id,
            DeleteTimetableDraftRequest {
                row_version: current.row_version,
                expected_block_count: count
            }
        )
    );
    assert_ne!(
        saved.is_ok(),
        deleted.is_ok(),
        "one serialized writer succeeds, the other sees a deleted or stale draft"
    );
    let remaining: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_blocks WHERE timetable_version_id=$1",
    )
    .bind(draft.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    if deleted.is_ok() {
        assert_eq!(remaining, 0);
    } else {
        assert_eq!(remaining, count);
        assert!(matches!(deleted.unwrap_err(), AppError::Conflict(_)));
    }
}

#[tokio::test]
async fn timetable_draft_deletion_serializes_with_publication() {
    use school_academic_timetable::{
        models::{
            timetable_publication::PublishTimetableVersionRequest,
            timetable_version::DeleteTimetableDraftRequest,
        },
        services::timetable_lifecycle,
    };
    let pool = prepare_concurrent_delivery_runtime_fixture("table_delete_publish_race").await;
    let context = operational_change_runtime_context(&pool).await;
    let draft = independent_table_draft(&pool, &context).await;
    fill_change_set_target_deficits(&pool, context.teacher_id, context.term_id, draft.id).await;
    let preview = table_preview(&pool, draft.id).await;
    assert!(preview.can_publish, "{:?}", preview.findings);
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_blocks WHERE timetable_version_id=$1",
    )
    .bind(draft.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let (published, deleted) = tokio::join!(
        timetable_lifecycle::publish(
            &pool,
            context.teacher_id,
            draft.id,
            PublishTimetableVersionRequest {
                row_version: preview.row_version,
                effective_from: preview.effective_from,
                preview_hash: preview.preview_hash,
                idempotency_key: Uuid::new_v4()
            }
        ),
        timetable_lifecycle::delete_draft(
            &pool,
            context.teacher_id,
            draft.id,
            DeleteTimetableDraftRequest {
                row_version: preview.row_version,
                expected_block_count: count
            }
        )
    );
    assert_ne!(published.is_ok(), deleted.is_ok());
    let remaining: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM academic_timetable_blocks WHERE timetable_version_id=$1",
    )
    .bind(draft.id)
    .fetch_one(&pool)
    .await
    .unwrap();
    if published.is_ok() {
        assert_eq!(remaining, count);
        assert!(matches!(deleted.unwrap_err(), AppError::Conflict(_)));
    } else {
        assert_eq!(remaining, 0);
        assert!(matches!(published.unwrap_err(), AppError::NotFound(_)));
    }
}

fn delete_opening_request(
    change: &super::models::AcademicTermChangeSet,
    version: &school_academic_delivery::models::versions::DeliveryVersion,
) -> school_academic_delivery::models::versions::DeleteDeliveryVersionRequest {
    school_academic_delivery::models::versions::DeleteDeliveryVersionRequest {
        row_version: version.row_version,
        change_set_row_version: change.row_version,
        expected_item_count: change.items.len() as i64,
        expected_offering_count: version.snapshot.offerings.len() as i64,
        expected_group_count: version
            .snapshot
            .offerings
            .iter()
            .map(|o| o.groups.len() as i64)
            .sum(),
    }
}

async fn publication_preview(
    pool: &sqlx::PgPool,
    id: uuid::Uuid,
) -> Result<school_academic_delivery::models::AcademicTermChangeSetPreview, school_errors::AppError>
{
    let revision =
        school_academic_delivery::services::change_sets::get_change_set(pool, id).await?;
    school_academic_delivery::services::change_sets::preview_change_set_at(
        pool,
        id,
        Some(revision.reference_date),
    )
    .await
}

#[tokio::test]
async fn opening_hard_delete_removes_exclusive_children_and_rejects_stale_counts() {
    let pool = prepare_delivery_runtime_fixture("opening_hard_delete_children").await;
    let context = planning_runtime_context(&pool).await;
    let revision = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        1,
        "opening-hard-delete",
    )
    .await;
    assert!(revision.effective_from.is_none());
    let offering = offerings::create_for_delivery(
        &pool,
        context.teacher_id,
        Some(revision.target_delivery_version_id),
        course_request(&context),
    )
    .await
    .unwrap();
    let group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            name: "กลุ่มที่จะลบ".into(),
            description: None,
            capacity: None,
            preferred_room_ids: vec![],
        },
    )
    .await
    .unwrap();
    let current = change_sets::get_change_set(&pool, revision.id)
        .await
        .unwrap();
    let version = school_academic_delivery::services::versions::get_version(
        &pool,
        current.target_delivery_version_id,
    )
    .await
    .unwrap();
    let request = delete_opening_request(&current, &version);
    let mut stale = request.clone();
    stale.expected_group_count += 1;
    assert!(matches!(
        change_sets::delete_version(&pool, context.teacher_id, version.id, stale).await,
        Err(AppError::Conflict(_))
    ));
    let deleted = change_sets::delete_version(&pool, context.teacher_id, version.id, request)
        .await
        .unwrap();
    assert_eq!(deleted.deleted_offering_count, 1);
    let remaining:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM academic_delivery_versions WHERE id=$1) OR EXISTS(SELECT 1 FROM academic_term_change_sets WHERE id=$2) OR EXISTS(SELECT 1 FROM learning_offerings WHERE id=$3) OR EXISTS(SELECT 1 FROM learning_groups WHERE id=$4)").bind(version.id).bind(current.id).bind(offering.id).bind(group.id).fetch_one(&pool).await.unwrap();
    assert!(!remaining);
    let audit:i64=sqlx::query_scalar("SELECT count(*) FROM academic_audit_events WHERE entity_id=$1 AND event_code='academic_delivery_version.deleted'").bind(version.id).fetch_one(&pool).await.unwrap();
    assert_eq!(audit, 1);
}

#[tokio::test]
async fn opening_preview_dates_are_read_only_bound_to_hash_and_validate_teacher_windows() {
    let pool = prepare_delivery_runtime_fixture("opening_candidate_dates").await;
    let context = operational_change_runtime_context(&pool).await;
    let revision = create_runtime_change_set(
        &pool,
        context.teacher_id,
        context.term_id,
        1,
        "opening-candidate-dates",
    )
    .await;
    let before = school_academic_delivery::services::versions::get_version(
        &pool,
        revision.target_delivery_version_id,
    )
    .await
    .unwrap();
    let general = change_sets::preview_change_set(&pool, revision.id)
        .await
        .unwrap();
    assert!(general.preliminary);
    let chosen =
        change_sets::preview_change_set_at(&pool, revision.id, Some(revision.reference_date))
            .await
            .unwrap();
    assert!(!chosen.preliminary);
    assert_ne!(general.preview_hash, chosen.preview_hash);
    let next = change_sets::preview_change_set_at(
        &pool,
        revision.id,
        Some(revision.reference_date + Duration::days(1)),
    )
    .await
    .unwrap();
    assert_ne!(chosen.preview_hash, next.preview_hash);
    let after = school_academic_delivery::services::versions::get_version(
        &pool,
        revision.target_delivery_version_id,
    )
    .await
    .unwrap();
    assert_eq!(before.row_version, after.row_version);
    assert!(after.effective_from.is_none());
    assert_eq!(before.reference_date, after.reference_date);
    let stale = change_sets::publish_change_set(
        &pool,
        context.teacher_id,
        revision.id,
        PublishAcademicTermChangeSetRequest {
            effective_from: revision.reference_date + Duration::days(1),
            row_version: chosen.change_set_row_version,
            target_delivery_version_row_version: chosen.target_delivery_version_row_version,
            preview_hash: chosen.preview_hash,
            acknowledged_warning_codes: vec![],
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap_err();
    assert!(matches!(stale, AppError::Conflict(_)));
}

#[tokio::test]
async fn manual_activity_activation_creates_reviewed_groups_atomically_or_stays_central() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_manual_activity_groups").await;
    let context = planning_runtime_context(&pool).await;
    let activity_version_id: Uuid = sqlx::query_scalar("SELECT version.id FROM activity_versions version JOIN academic_terms term ON term.id=$1 WHERE version.status='published' AND version.scheduling_mode='synchronized' AND version.effective_from <= term.start_date AND (version.effective_until IS NULL OR version.effective_until > term.start_date) ORDER BY version.id LIMIT 1")
        .bind(context.term_id).fetch_one(&pool).await.unwrap();
    let request = CreateActivityOfferingRequest {
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
    assert!(offerings::create(
        &pool,
        context.teacher_id,
        CreateLearningOfferingRequest::Activity(invalid)
    )
    .await
    .is_err());
    let after: i64 = sqlx::query_scalar("SELECT count(*) FROM learning_offerings")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(before, after);
}
