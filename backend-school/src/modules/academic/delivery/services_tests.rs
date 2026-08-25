use crate::{
    modules::academic::{
        cutover_test_preflight::run_academic_core_preflight,
        cutover_test_support::{
            apply_migrations_through, apply_phase_b_runtime_migrations,
            seed_academic_cutover_fixture, CutoverFixture,
        },
    },
    test_helpers::create_named_test_pool,
};
use chrono::NaiveDate;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::{
    models::{
        ActivityAttendanceRequirement, ActivityPassCriteria, ActivityRegistrationType,
        ActivitySchedulingMode, ApplyCurriculumOfferingsRequest, ApplyRosterRequest,
        CourseGradingPolicy, CreateActivityOfferingRequest, CreateCourseOfferingRequest,
        CreateLearningGroupRequest, CreateLearningOfferingRequest, LearningOfferingKind,
        LearningOfferingQuery, LearningOfferingSnapshot, LearningOfferingStatus,
        LearningTeacherRole, OfferingTargetInput, OfferingTargetKind,
        PreviewCurriculumOfferingsRequest, PublishLearningOfferingRequest, PublishRosterRequest,
        ReplaceLearningGroupHomeroomsRequest, ReplaceLearningGroupTeachersRequest,
        RosterOverrideAction, RosterOverrideInput, StudentActivityRegistrationQuery,
        TeacherAssignmentInput,
    },
    services::{activities, groups, offerings},
};
use crate::error::AppError;
use crate::policies::resource_access_policy::AcademicResourceListFilter;

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
    pool
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
                 WHERE homeroom.academic_year_id = term.academic_year_id
                   AND (requirement.recommended_term_code IS NULL
                        OR lower(requirement.recommended_term_code) = lower(term.code))
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

fn course_request(context: &RuntimeContext) -> CreateLearningOfferingRequest {
    CreateLearningOfferingRequest::Course(CreateCourseOfferingRequest {
        academic_term_id: context.term_id,
        subject_version_id: context.subject_version_id,
        curriculum_course_requirement_id: None,
        owning_organization_unit_id: context.owner_id,
        targets: vec![OfferingTargetInput {
            target_kind: OfferingTargetKind::Homeroom,
            homeroom_id: Some(context.homeroom_id),
            grade_level_id: context.grade_level_id,
            study_program_id: context.study_program_id,
        }],
        grading_policy: CourseGradingPolicy {
            policy_code: "school_default".to_string(),
            total_score: "100.00".to_string(),
            passing_score: Some("50.00".to_string()),
        },
    })
}

#[test]
fn create_offering_wire_contract_is_strictly_tagged_by_kind() {
    let parsed = serde_json::from_value::<CreateLearningOfferingRequest>(serde_json::json!({
        "kind": "course",
        "academicTermId": Uuid::nil(),
        "subjectVersionId": Uuid::nil(),
        "owningOrganizationUnitId": Uuid::nil(),
        "targets": [],
        "gradingPolicy": { "policyCode": "school_default", "passingScore": "50.00" }
    }))
    .unwrap();
    assert!(matches!(parsed, CreateLearningOfferingRequest::Course(_)));

    let wrong_subtype =
        serde_json::from_value::<CreateLearningOfferingRequest>(serde_json::json!({
            "kind": "course",
            "academicTermId": Uuid::nil(),
            "activityVersionId": Uuid::nil(),
            "owningOrganizationUnitId": Uuid::nil(),
            "targets": [],
            "gradingPolicy": { "policyCode": "school_default" }
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

    let mut invalid_owner_request = course_request(&context);
    let CreateLearningOfferingRequest::Course(course) = &mut invalid_owner_request else {
        unreachable!();
    };
    course.owning_organization_unit_id = Uuid::new_v4();
    let invalid_owner = offerings::create(&pool, context.teacher_id, invalid_owner_request).await;
    assert!(matches!(invalid_owner, Err(AppError::ValidationError(_))));

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

    let group = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            code: "M1-A".to_string(),
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

    let duplicate = groups::create(
        &pool,
        context.teacher_id,
        offering.id,
        CreateLearningGroupRequest {
            code: "m1-a".to_string(),
            name: "กลุ่มซ้ำ".to_string(),
            description: None,
            capacity: None,
            preferred_room_ids: Vec::new(),
        },
    )
    .await;
    assert!(matches!(duplicate, Err(AppError::Conflict(_))));

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

#[tokio::test]
async fn curriculum_preview_apply_is_hash_checked_and_closed_terms_reject_writes() {
    let pool = prepare_delivery_runtime_fixture("academic_delivery_runtime_curriculum").await;
    let context = planning_runtime_context(&pool).await;

    let expired_program_id: Uuid = sqlx::query_scalar(
        r#"SELECT program.id
           FROM study_programs program
           JOIN curriculum_versions version ON version.id = program.curriculum_version_id
           JOIN academic_years ending_year ON ending_year.id = version.end_academic_year_id
           JOIN academic_years selected_year ON selected_year.id = $1
           WHERE ending_year.end_date < selected_year.start_date
           ORDER BY program.id
           LIMIT 1"#,
    )
    .bind(context.year_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let expired_program = offerings::preview_from_curriculum(
        &pool,
        PreviewCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![expired_program_id],
        },
    )
    .await;
    assert!(matches!(expired_program, Err(AppError::ValidationError(_))));

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
    assert!(!preview.items.is_empty());

    let mismatched = offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
            owning_organization_unit_id: context.owner_id,
            source_hash: "stale-source-hash".to_string(),
            idempotency_key: Uuid::new_v4(),
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
            owning_organization_unit_id: context.owner_id,
            source_hash: preview.source_hash.clone(),
            idempotency_key,
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
            owning_organization_unit_id: context.owner_id,
            source_hash: preview.source_hash,
            idempotency_key,
        },
    )
    .await
    .unwrap();
    assert_eq!(retried.offering_ids, applied.offering_ids);

    let retained_preview = offerings::preview_from_curriculum(
        &pool,
        PreviewCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
        },
    )
    .await
    .unwrap();
    let retained = offerings::apply_from_curriculum(
        &pool,
        context.teacher_id,
        ApplyCurriculumOfferingsRequest {
            academic_term_id: context.term_id,
            study_program_ids: vec![context.study_program_id],
            owning_organization_unit_id: context.owner_id,
            source_hash: retained_preview.source_hash,
            idempotency_key: Uuid::new_v4(),
        },
    )
    .await
    .unwrap();
    assert_eq!(retained.created_count, 0);
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
    assert!(matches!(closed, Err(AppError::ValidationError(_))));
}

#[tokio::test]
async fn self_registration_activity_uses_common_delivery_and_reads_migrated_pass_fail_result() {
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
            academic_term_id: context.term_id,
            activity_version_id,
            curriculum_activity_requirement_id: None,
            owning_organization_unit_id: context.owner_id,
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
            code: "ACT-SELF".to_string(),
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
    let students = groups::list_students(&pool, group.id).await.unwrap();
    assert_eq!(students.len(), 1);
    assert_eq!(students[0].roster_source, "manual_add");

    let (migrated_group_id, migrated_student_year_id): (Uuid, Uuid) = sqlx::query_as(
        "SELECT learning_group_id, student_academic_year_id FROM learning_results \
         WHERE kind = 'activity' ORDER BY id LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let migrated_result =
        activities::get_result(&pool, migrated_group_id, migrated_student_year_id)
            .await
            .unwrap()
            .unwrap();
    assert!(matches!(
        migrated_result.outcome.as_deref(),
        Some("pass" | "fail")
    ));
}

#[tokio::test]
async fn student_activity_registration_is_term_scoped_eligible_and_revisioned() {
    let pool =
        prepare_delivery_runtime_fixture("academic_delivery_student_activity_registration").await;
    let context = planning_runtime_context(&pool).await;
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
            academic_term_id: context.term_id,
            activity_version_id,
            curriculum_activity_requirement_id: None,
            owning_organization_unit_id: context.owner_id,
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
            code: "ACT-STUDENT-A".to_string(),
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
            code: "ACT-STUDENT-B".to_string(),
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
    let pool = prepare_delivery_fixture("academic_delivery_batch_list", false).await;
    apply_phase_b_runtime_migrations(&pool).await.unwrap();
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
