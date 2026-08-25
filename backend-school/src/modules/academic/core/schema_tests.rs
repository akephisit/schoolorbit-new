use crate::{
    modules::academic::{
        cutover_preflight::run_academic_core_preflight,
        cutover_test_support::{
            apply_cutover_fixture_fault, apply_migrations_through, seed_academic_cutover_fixture,
            CutoverFixture, CutoverFixtureFault,
        },
    },
    test_helpers::create_named_test_pool,
};
use chrono::NaiveDate;
use serde_json::Value;
use uuid::Uuid;

const ACADEMIC_CORE_NAMESPACE: Uuid = Uuid::from_u128(0x5c33_b984_10df_58db_bf80_62db_c4a0_3d1b);

fn stable_uuid(name: &str) -> Uuid {
    Uuid::new_v5(&ACADEMIC_CORE_NAMESPACE, name.as_bytes())
}

#[tokio::test]
async fn migration_chain_supports_an_empty_new_tenant() {
    let pool = create_named_test_pool("academic_core_empty_tenant").await;

    apply_migrations_through(&pool, 44)
        .await
        .expect("an empty newly provisioned tenant must migrate through Phase A");

    let (year_count, term_count): (i64, i64) = sqlx::query_as(
        r#"SELECT (SELECT COUNT(*) FROM academic_years),
                  (SELECT COUNT(*) FROM academic_terms)"#,
    )
    .fetch_one(&pool)
    .await
    .expect("canonical academic context tables must exist");
    assert_eq!((year_count, term_count), (0, 0));

    let latest_version: i64 =
        sqlx::query_scalar("SELECT COALESCE(MAX(version), 0) FROM _sqlx_migrations")
            .fetch_one(&pool)
            .await
            .expect("migration history must be queryable");
    assert_eq!(latest_version, 44);
}

#[tokio::test]
async fn migration_041_maps_core_fixture() {
    let pool = create_named_test_pool("academic_core_041").await;
    apply_migrations_through(&pool, 40)
        .await
        .expect("legacy migrations must apply");
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .expect("passing cutover fixture must seed");
    let preflight = run_academic_core_preflight(
        &pool,
        "schoolorbit_test_academic_core_041",
        NaiveDate::from_ymd_opt(2025, 8, 23).expect("test cutover date must be valid"),
    )
    .await
    .expect("preflight must run before migration");
    assert!(preflight.can_cut_over);

    apply_migrations_through(&pool, 41)
        .await
        .expect("migration 041 must apply to the passing fixture");

    let relations: Vec<String> = sqlx::query_scalar(
        r#"SELECT relname::text
           FROM pg_class
           WHERE relnamespace = current_schema()::regnamespace
             AND relname = ANY($1)
           ORDER BY relname"#,
    )
    .bind(vec![
        "academic_terms",
        "activities",
        "activity_versions",
        "bell_schedules",
        "curricula",
        "curriculum_versions",
        "study_programs",
        "subjects",
        "subject_versions",
    ])
    .fetch_all(&pool)
    .await
    .expect("target relations must be queryable");
    assert_eq!(relations.len(), 9);

    let stable_subject: (Uuid, String) = sqlx::query_as("SELECT id, identity_key FROM subjects")
        .fetch_one(&pool)
        .await
        .expect("stable subject must exist");
    assert_eq!(stable_subject.0, stable_uuid("subject:math-core"));
    assert_eq!(stable_subject.1, "math-core");

    let subject_versions: Vec<(
        Uuid,
        Uuid,
        i32,
        NaiveDate,
        Option<NaiveDate>,
        String,
        String,
    )> = sqlx::query_as(
        r#"SELECT id, subject_id, version_no, effective_from, effective_until,
                      credit::text, status
               FROM subject_versions
               WHERE subject_id = $1
               ORDER BY version_no"#,
    )
    .bind(stable_subject.0)
    .fetch_all(&pool)
    .await
    .expect("subject versions must be queryable");
    assert_eq!(subject_versions.len(), 2);
    assert_eq!(
        subject_versions.iter().map(|row| row.0).collect::<Vec<_>>(),
        vec![
            Uuid::parse_str("20000000-0000-0000-0000-000000000024").unwrap(),
            Uuid::parse_str("20000000-0000-0000-0000-000000000025").unwrap(),
        ]
    );
    assert!(subject_versions
        .iter()
        .all(|row| row.1 == stable_subject.0 && row.5 == "1.50" && row.6 == "published"));
    assert_eq!(
        subject_versions[0].4,
        Some(NaiveDate::from_ymd_opt(2025, 5, 1).unwrap())
    );
    assert_eq!(subject_versions[1].4, None);

    let summer_term: (i32, String, bool, bool, String) = sqlx::query_as(
        r#"SELECT sequence_no, term_type, included_in_year_result,
                  blocks_year_closure, status
           FROM academic_terms
           WHERE id = '11000000-0000-0000-0000-000000000253'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("summer term must be preserved");
    assert_eq!(
        summer_term,
        (3, "summer".to_string(), true, true, "planning".to_string())
    );

    let active_state: (String, String) = sqlx::query_as(
        r#"SELECT year.status, term.status
           FROM academic_years year
           JOIN academic_terms term ON term.academic_year_id = year.id
           WHERE year.id = '10000000-0000-0000-0000-000000000025'
             AND term.id = '11000000-0000-0000-0000-000000000251'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("active year and term must be queryable");
    assert_eq!(active_state, ("active".to_string(), "active".to_string()));

    let stable_activity_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM activities ORDER BY identity_key")
            .fetch_all(&pool)
            .await
            .expect("stable activities must be queryable");
    assert_eq!(
        stable_activity_ids,
        vec![
            stable_uuid("activity:guidance:แนะแนว"),
            stable_uuid("activity:scout:ลูกเสือ"),
        ]
    );

    let program: (Uuid, Uuid) = sqlx::query_as(
        r#"SELECT id, curriculum_version_id
           FROM study_programs
           WHERE curriculum_version_id = '31000000-0000-0000-0000-000000000025'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("default program must exist");
    assert_eq!(
        program.0,
        stable_uuid("program:31000000-0000-0000-0000-000000000025")
    );

    let preserved_requirement_ids: Vec<Uuid> =
        sqlx::query_scalar("SELECT id FROM curriculum_course_requirements ORDER BY id")
            .fetch_all(&pool)
            .await
            .expect("course requirements must be preserved");
    assert_eq!(preserved_requirement_ids.len(), 2);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM curriculum_activity_requirements")
            .fetch_one(&pool)
            .await
            .unwrap(),
        3
    );

    let default_bell_schedules: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM bell_schedules WHERE is_default")
            .fetch_one(&pool)
            .await
            .expect("bell schedules must be queryable");
    assert_eq!(default_bell_schedules, 4);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM academic_terms WHERE bell_schedule_id IS NOT NULL",
        )
        .fetch_one(&pool)
        .await
        .unwrap(),
        preflight.source_counts["academicTerms"]
    );

    let audit_counts: Value = sqlx::query_scalar(
        "SELECT source_counts FROM academic_core_cutover_audits WHERE migration_version = 41",
    )
    .fetch_one(&pool)
    .await
    .expect("migration audit must exist");
    assert_eq!(
        audit_counts["academicYears"],
        preflight.source_counts["academicYears"]
    );
    assert_eq!(
        audit_counts["subjects"],
        preflight.source_counts["subjects"]
    );
    assert_eq!(
        audit_counts["activities"],
        preflight.source_counts["activities"]
    );
}

#[tokio::test]
async fn migration_041_rejects_blank_subject_identity_with_stable_code() {
    let pool = create_named_test_pool("academic_core_041_blank_subject").await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    sqlx::query("UPDATE subjects SET code = ' ' WHERE id = '20000000-0000-0000-0000-000000000024'")
        .execute(&pool)
        .await
        .unwrap();

    let error = apply_migrations_through(&pool, 41)
        .await
        .expect_err("blank identity must block migration 041");
    assert!(error
        .to_string()
        .contains("ACADEMIC_CORE_041_SUBJECT_IDENTITY_BLANK"));
}

#[tokio::test]
async fn migration_041_enforces_published_range_and_term_invariants() {
    let pool = create_named_test_pool("academic_core_041_invariants").await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_migrations_through(&pool, 41).await.unwrap();

    let published_error = sqlx::query(
        r#"UPDATE subject_versions
           SET name_th = 'ชื่อที่ไม่ควรแก้ได้'
           WHERE id = '20000000-0000-0000-0000-000000000025'"#,
    )
    .execute(&pool)
    .await
    .expect_err("published subject version must be immutable");
    assert!(published_error
        .to_string()
        .contains("ACADEMIC_CORE_PUBLISHED_VERSION_IMMUTABLE"));

    let overlap_error = sqlx::query(
        r#"INSERT INTO subject_versions (
               id, code, name_th, credit, type, is_active, start_academic_year_id,
               subject_id, version_no, effective_from, status, migration_provenance
           )
           SELECT '20000000-0000-0000-0000-000000000225', 'OVERLAP-FIXTURE', 'ฉบับช่วงทับซ้อน',
                  credit, type, true, start_academic_year_id, subject_id, 99,
                  '2025-06-01', 'draft', '{"fixture":"overlap"}'::jsonb
           FROM subject_versions
           WHERE id = '20000000-0000-0000-0000-000000000025'"#,
    )
    .execute(&pool)
    .await
    .expect_err("overlapping version must be rejected");
    assert!(
        overlap_error
            .to_string()
            .contains("ACADEMIC_CORE_SUBJECT_VERSION_RANGE_OVERLAP"),
        "unexpected overlap error: {overlap_error}"
    );

    let term_error = sqlx::query(
        r#"UPDATE academic_terms
           SET end_date = '2027-05-01'
           WHERE id = '11000000-0000-0000-0000-000000000262'"#,
    )
    .execute(&pool)
    .await
    .expect_err("term outside its year must be rejected");
    assert!(term_error
        .to_string()
        .contains("ACADEMIC_CORE_TERM_OUTSIDE_YEAR"));

    let requirement_error = sqlx::query(
        r#"UPDATE curriculum_course_requirements
           SET display_order = display_order + 1
           WHERE id = '32000000-0000-0000-0000-000000000025'"#,
    )
    .execute(&pool)
    .await
    .expect_err("requirements owned by a published curriculum must be immutable");
    assert!(requirement_error
        .to_string()
        .contains("ACADEMIC_CORE_PUBLISHED_CURRICULUM_IMMUTABLE"));
}

#[tokio::test]
async fn migration_041_preconditions_reject_ambiguous_core_data() {
    let cases = [
        (
            CutoverFixtureFault::SubjectIdentityConflict,
            "ACADEMIC_CORE_041_SUBJECT_IDENTITY_CONFLICT",
        ),
        (
            CutoverFixtureFault::TermSequence,
            "ACADEMIC_CORE_041_TERM_SEQUENCE_AMBIGUOUS",
        ),
        (
            CutoverFixtureFault::SubjectVersionOverlap,
            "ACADEMIC_CORE_041_SUBJECT_VERSION_RANGE_OVERLAP",
        ),
        (
            CutoverFixtureFault::CourseTermYear,
            "ACADEMIC_CORE_041_COURSE_TERM_YEAR_MISMATCH",
        ),
    ];

    for (index, (fault, expected_code)) in cases.into_iter().enumerate() {
        let pool = create_named_test_pool(&format!("academic_041_precondition_{index}")).await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_cutover_fixture_fault(&pool, fault).await.unwrap();

        let error = apply_migrations_through(&pool, 41)
            .await
            .expect_err("ambiguous core data must block migration 041");
        assert!(
            error.to_string().contains(expected_code),
            "fault {fault:?} returned an unexpected migration error: {error}"
        );
    }
}

#[tokio::test]
async fn migration_043_maps_all_consumers() {
    let pool = create_named_test_pool("academic_core_043_consumers").await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_migrations_through(&pool, 42).await.unwrap();

    apply_migrations_through(&pool, 43)
        .await
        .expect("migration 043 must map every affected consumer");

    let assessment_plan: (Uuid, Uuid, Uuid, Uuid) = sqlx::query_as(
        r#"SELECT id, learning_offering_id, academic_term_id, academic_year_id
           FROM course_assessment_plans
           WHERE id = '80000000-0000-0000-0000-000000000001'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("assessment plan ID and context must be preserved");
    assert_eq!(
        assessment_plan,
        (
            Uuid::parse_str("80000000-0000-0000-0000-000000000001").unwrap(),
            stable_uuid(
                "course-offering:11000000-0000-0000-0000-000000000251:20000000-0000-0000-0000-000000000025",
            ),
            Uuid::parse_str("11000000-0000-0000-0000-000000000251").unwrap(),
            Uuid::parse_str("10000000-0000-0000-0000-000000000025").unwrap(),
        )
    );

    let plan_group_shape: (i64, i64) = sqlx::query_as(
        r#"SELECT
               (SELECT COUNT(*) FROM course_assessment_plans
                WHERE learning_offering_id = $1),
               (SELECT COUNT(*) FROM learning_groups
                WHERE learning_offering_id = $1)"#,
    )
    .bind(assessment_plan.1)
    .fetch_one(&pool)
    .await
    .expect("one offering-level assessment plan must serve every delivery group");
    assert_eq!(plan_group_shape, (1, 2));

    let category_score: String = sqlx::query_scalar(
        r#"SELECT max_score::text
           FROM course_assessment_categories
           WHERE id = '81000000-0000-0000-0000-000000000001'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("category score must remain exact");
    assert_eq!(category_score, "12.50");

    let item_scores: Vec<String> = sqlx::query_scalar(
        r#"SELECT max_score::text
           FROM course_assessment_items
           ORDER BY display_order"#,
    )
    .fetch_all(&pool)
    .await
    .expect("assessment item scores must remain exact");
    assert_eq!(item_scores, vec!["7.25", "0.10"]);

    let timetable: (Uuid, Uuid, Uuid, Uuid) = sqlx::query_as(
        r#"SELECT id, academic_term_id, learning_group_id, homeroom_id
           FROM academic_timetable_entries
           WHERE id = '83000000-0000-0000-0000-000000000001'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("timetable entry must use the delivery context");
    assert_eq!(
        timetable,
        (
            Uuid::parse_str("83000000-0000-0000-0000-000000000001").unwrap(),
            Uuid::parse_str("11000000-0000-0000-0000-000000000251").unwrap(),
            Uuid::parse_str("60000000-0000-0000-0000-000000000025").unwrap(),
            Uuid::parse_str("40000000-0000-0000-0000-000000000025").unwrap(),
        )
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM timetable_entry_instructors")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );

    let exam_item: (Uuid, Uuid, Uuid, Uuid, Uuid) = sqlx::query_as(
        r#"SELECT id, academic_term_id, learning_offering_id, learning_group_id,
                  course_assessment_plan_id
           FROM academic_exam_schedule_items
           WHERE id = '86000000-0000-0000-0000-000000000001'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("exam item must use one term/offering/group context");
    assert_eq!(
        exam_item.0,
        Uuid::parse_str("86000000-0000-0000-0000-000000000001").unwrap()
    );
    assert_eq!(exam_item.1, timetable.1);
    assert_eq!(exam_item.2, assessment_plan.1);
    assert_eq!(exam_item.3, timetable.2);
    assert_eq!(exam_item.4, assessment_plan.0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM academic_exam_sessions")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM academic_exam_day_room_assignments")
            .fetch_one(&pool)
            .await
            .unwrap(),
        1
    );

    let supervision_cycle: (Uuid, Uuid, Option<Uuid>) = sqlx::query_as(
        r#"SELECT id, academic_year_id, academic_term_id
           FROM supervision_cycles
           WHERE id = '88000000-0000-0000-0000-000000000001'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("supervision cycle must have explicit year and optional term context");
    assert_eq!(
        supervision_cycle,
        (
            Uuid::parse_str("88000000-0000-0000-0000-000000000001").unwrap(),
            Uuid::parse_str("10000000-0000-0000-0000-000000000025").unwrap(),
            Some(Uuid::parse_str("11000000-0000-0000-0000-000000000251").unwrap()),
        )
    );

    let observation_context: (Option<Uuid>, Option<Uuid>) = sqlx::query_as(
        r#"SELECT learning_group_id, homeroom_id
           FROM supervision_observations
           WHERE id = '89000000-0000-0000-0000-000000000001'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("timetable-backed supervision must resolve delivery context");
    assert_eq!(observation_context, (Some(timetable.2), Some(timetable.3)));

    let question: (Uuid, Uuid, String) = sqlx::query_as(
        r#"SELECT id, subject_id, points::text
           FROM academic_question_bank_questions
           WHERE id = '93000000-0000-0000-0000-000000000001'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("question bank must use stable subject identity and exact points");
    assert_eq!(
        question,
        (
            Uuid::parse_str("93000000-0000-0000-0000-000000000001").unwrap(),
            stable_uuid("subject:math-core"),
            "0.10".to_string(),
        )
    );

    let admission_track_program: Uuid = sqlx::query_scalar(
        r#"SELECT study_program_id
           FROM admission_tracks
           WHERE id = '91000000-0000-0000-0000-000000000001'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("admission track must resolve the year-specific study program");
    assert_eq!(
        admission_track_program,
        stable_uuid("program:31000000-0000-0000-0000-000000000025")
    );

    let admission_assignment: (Uuid, Uuid, Uuid, Uuid, String, String) = sqlx::query_as(
        r#"SELECT id, homeroom_id, student_academic_year_id, homeroom_placement_id,
                  total_score::text, full_score::text
           FROM admission_room_assignments
           WHERE id = '94100000-0000-0000-0000-000000000001'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("successful admission must resolve placement semantics");
    assert_eq!(
        admission_assignment,
        (
            Uuid::parse_str("94100000-0000-0000-0000-000000000001").unwrap(),
            timetable.3,
            stable_uuid(
                "student-year:50000000-0000-0000-0000-000000000001:10000000-0000-0000-0000-000000000025",
            ),
            Uuid::parse_str("51000000-0000-0000-0000-000000000025").unwrap(),
            "12.50".to_string(),
            "12.50".to_string(),
        )
    );

    let calendar_context: (Option<Uuid>, Option<Uuid>, Uuid) = sqlx::query_as(
        r#"SELECT event.academic_year_id, event.academic_term_id, target.homeroom_id
           FROM calendar_events event
           JOIN calendar_event_targets target ON target.event_id = event.id
           WHERE event.id = '95100000-0000-0000-0000-000000000001'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("calendar event and audience target must carry explicit context");
    assert_eq!(
        calendar_context,
        (
            Some(Uuid::parse_str("10000000-0000-0000-0000-000000000025").unwrap()),
            Some(Uuid::parse_str("11000000-0000-0000-0000-000000000251").unwrap()),
            timetable.3,
        )
    );

    let certificate_year: Uuid = sqlx::query_scalar(
        r#"SELECT academic_year_id
           FROM certificate_campaigns
           WHERE id = '96000000-0000-0000-0000-000000000001'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("certificate campaign year ownership must remain intact");
    assert_eq!(
        certificate_year,
        Uuid::parse_str("10000000-0000-0000-0000-000000000025").unwrap()
    );

    let active_target_permissions: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM permissions
           WHERE is_active
             AND code IN (
                 'academic_context.read.school',
                 'academic_year.read.school',
                 'academic_year.manage.school',
                 'academic_term.read.school',
                 'academic_term.manage.school',
                 'academic_catalog.read.school',
                 'academic_catalog.manage.organization_unit',
                 'academic_catalog.manage.organization_tree',
                 'academic_catalog.manage.school',
                 'academic_curriculum.read.organization_unit',
                 'academic_curriculum.read.organization_tree',
                 'academic_curriculum.read.school',
                 'academic_curriculum.manage.organization_unit',
                 'academic_curriculum.manage.organization_tree',
                 'academic_curriculum.manage.school',
                 'homeroom.read.school',
                 'homeroom.manage.school',
                 'student_academic_year.read.school',
                 'student_academic_year.manage.school',
                 'learning_offering.read.assigned',
                 'learning_offering.read.organization_unit',
                 'learning_offering.read.organization_tree',
                 'learning_offering.read.school',
                 'learning_offering.manage.assigned',
                 'learning_offering.manage.organization_unit',
                 'learning_offering.manage.organization_tree',
                 'learning_offering.manage.school'
             )"#,
    )
    .fetch_one(&pool)
    .await
    .expect("new permission definitions must be queryable");
    assert_eq!(active_target_permissions, 27);

    let academic_permission_modules = vec![
        "academic_context",
        "academic_year",
        "academic_term",
        "academic_catalog",
        "academic_curriculum",
        "homeroom",
        "student_academic_year",
        "learning_offering",
        "academic_assessment",
        "academic_exam_schedule",
        "academic_question_bank",
        "academic_timetable_today",
    ];
    let permission_lock: Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../contracts/permissions.lock.json"
    )))
    .expect("generated permission lock must contain valid JSON");
    let mut contract_academic_codes: Vec<String> = permission_lock["permission_codes"]
        .as_array()
        .expect("permission lock must contain permission_codes")
        .iter()
        .filter_map(Value::as_str)
        .filter(|code| {
            academic_permission_modules
                .iter()
                .any(|module| *code == *module || code.starts_with(&format!("{module}.")))
        })
        .map(str::to_string)
        .collect();
    contract_academic_codes.sort();

    let active_database_academic_codes: Vec<String> = sqlx::query_scalar(
        r#"SELECT code
           FROM permissions
           WHERE is_active
             AND module = ANY($1)
           ORDER BY code"#,
    )
    .bind(&academic_permission_modules)
    .fetch_all(&pool)
    .await
    .expect("active database permissions must be comparable with the generated contract");
    assert_eq!(active_database_academic_codes, contract_academic_codes);

    let unmapped_old_role_principals: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM role_permissions old_grant
           JOIN permissions old_permission ON old_permission.id = old_grant.permission_id
           WHERE old_permission.code = 'academic_structure.read.all'
             AND (
                 SELECT COUNT(DISTINCT target_permission.code)
                 FROM role_permissions target_grant
                 JOIN permissions target_permission ON target_permission.id = target_grant.permission_id
                 WHERE target_grant.role_id = old_grant.role_id
                   AND target_permission.code IN (
                       'academic_context.read.school',
                       'academic_year.read.school',
                       'academic_term.read.school',
                       'academic_catalog.read.school'
                   )
             ) <> 4"#,
    )
    .fetch_one(&pool)
    .await
    .expect("permission principal reconciliation must be queryable");
    assert_eq!(unmapped_old_role_principals, 0);

    let organization_permission_codes: Vec<String> = sqlx::query_scalar(
        r#"SELECT permission.code
           FROM organization_permission_grants grant_row
           JOIN permissions permission ON permission.id = grant_row.permission_id
           WHERE grant_row.organization_unit_id = 'c5e06a47-ebf6-40f6-bbf9-59c509e842f2'
             AND grant_row.position_code = 'head'
             AND permission.code IN (
                 'academic_curriculum.manage.organization_unit',
                 'academic_context.read.school'
             )
           ORDER BY permission.code"#,
    )
    .fetch_all(&pool)
    .await
    .expect("organization-scoped academic grants must retain their exact principal scope");
    assert_eq!(
        organization_permission_codes,
        vec![
            "academic_context.read.school".to_string(),
            "academic_curriculum.manage.organization_unit".to_string(),
        ]
    );

    let timetable_only_permission_codes: Vec<String> = sqlx::query_scalar(
        r#"SELECT permission.code
           FROM organization_permission_grants grant_row
           JOIN permissions permission ON permission.id = grant_row.permission_id
           WHERE grant_row.organization_unit_id = 'c5e06a47-ebf6-40f6-bbf9-59c509e842f2'
             AND grant_row.position_code = 'coordinator'
             AND permission.code IN (
                 'academic_timetable_today.read.school',
                 'academic_context.read.school'
             )
           ORDER BY permission.code"#,
    )
    .fetch_all(&pool)
    .await
    .expect("timetable-only principals must also receive academic context labels");
    assert_eq!(
        timetable_only_permission_codes,
        vec![
            "academic_context.read.school".to_string(),
            "academic_timetable_today.read.school".to_string(),
        ]
    );

    let promotion_only_principal: (bool, i64, i64) = sqlx::query_as(
        r#"SELECT promotion.is_active,
                  (SELECT COUNT(*)
                   FROM organization_permission_grants legacy_grant
                   WHERE legacy_grant.organization_unit_id =
                             'c5e06a47-ebf6-40f6-bbf9-59c509e842f2'
                     AND legacy_grant.position_code = 'member'
                     AND legacy_grant.permission_id = promotion.id),
                  (SELECT COUNT(*)
                   FROM organization_permission_grants target_grant
                   JOIN permissions target ON target.id = target_grant.permission_id
                   WHERE target_grant.organization_unit_id =
                             'c5e06a47-ebf6-40f6-bbf9-59c509e842f2'
                     AND target_grant.position_code = 'member'
                     AND target.is_active
                     AND target.module = ANY($1))
           FROM permissions promotion
           WHERE promotion.code = 'academic_promotion.execute.all'"#,
    )
    .bind(&academic_permission_modules)
    .fetch_one(&pool)
    .await
    .expect("promotion-only migration evidence must remain queryable");
    assert_eq!(promotion_only_principal, (false, 1, 0));

    let delegated_permission_codes: Vec<String> = sqlx::query_scalar(
        r#"SELECT permission.code
           FROM organization_permission_delegations delegation
           JOIN permissions permission ON permission.id = delegation.permission_id
           WHERE delegation.from_user_id = '50000000-0000-0000-0000-000000000002'
             AND delegation.to_user_id = '50000000-0000-0000-0000-000000000003'
             AND delegation.organization_unit_id = 'c5e06a47-ebf6-40f6-bbf9-59c509e842f2'
             AND permission.code IN (
                 'learning_offering.read.school',
                 'academic_context.read.school'
             )
           ORDER BY permission.code"#,
    )
    .fetch_all(&pool)
    .await
    .expect("delegated academic grants must retain scope and receive context labels");
    assert_eq!(
        delegated_permission_codes,
        vec![
            "academic_context.read.school".to_string(),
            "learning_offering.read.school".to_string(),
        ]
    );

    let mapped_consumers: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(DISTINCT source_table)
           FROM academic_core_entity_map
           WHERE migration_version = 43"#,
    )
    .fetch_one(&pool)
    .await
    .expect("consumer entity mappings must be queryable");
    assert!(mapped_consumers >= 12);
}

#[tokio::test]
async fn migration_043_enforces_consumer_context_after_cutover() {
    let pool = create_named_test_pool("academic_core_043_consumer_context").await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_migrations_through(&pool, 43).await.unwrap();

    sqlx::query(
        r#"UPDATE academic_timetable_entries
           SET day_of_week = 'TUE'
           WHERE id = '83000000-0000-0000-0000-000000000001'"#,
    )
    .execute(&pool)
    .await
    .expect("post-cutover timetable move trigger must use canonical period columns");

    sqlx::query(
        r#"INSERT INTO timetable_entry_instructors (id, entry_id, instructor_id, role)
           VALUES (
               '83100000-0000-0000-0000-000000000002',
               '83000000-0000-0000-0000-000000000001',
               '50000000-0000-0000-0000-000000000003',
               'secondary'
           )"#,
    )
    .execute(&pool)
    .await
    .expect("post-cutover instructor trigger must use canonical group and period columns");

    let exam_error = sqlx::query(
        r#"UPDATE academic_exam_schedule_items
           SET academic_term_id = '11000000-0000-0000-0000-000000000252'
           WHERE id = '86000000-0000-0000-0000-000000000001'"#,
    )
    .execute(&pool)
    .await
    .expect_err("an exam item cannot move outside its group and offering term");
    let exam_error_text = exam_error.to_string();
    assert!(
        exam_error_text.contains("academic_exam_schedule_items_group_context_fkey")
            || exam_error_text.contains("academic_exam_schedule_items_offering_context_fkey")
            || exam_error_text.contains("academic_exam_schedule_items_plan_offering_context_fkey")
            || exam_error_text.contains("academic_exam_schedule_items_round_semester_fkey"),
        "unexpected cross-term exam error: {exam_error_text}"
    );

    sqlx::query(
        r#"DELETE FROM academic_exam_schedule_items
           WHERE id = '86000000-0000-0000-0000-000000000001'"#,
    )
    .execute(&pool)
    .await
    .expect("the isolated assessment constraint check must remove its exam consumer");

    let assessment_error = sqlx::query(
        r#"UPDATE course_assessment_plans
           SET academic_term_id = '11000000-0000-0000-0000-000000000252'
           WHERE id = '80000000-0000-0000-0000-000000000001'"#,
    )
    .execute(&pool)
    .await
    .expect_err("an assessment plan cannot move outside its offering term");
    let assessment_error_text = assessment_error.to_string();
    assert!(
        assessment_error_text.contains("course_assessment_plans_offering_context_fkey"),
        "unexpected cross-term assessment error: {assessment_error_text}"
    );

    let assessment_subject_error = sqlx::query(
        r#"UPDATE course_assessment_plans
           SET subject_version_id = '20000000-0000-0000-0000-000000000024'
           WHERE id = '80000000-0000-0000-0000-000000000001'"#,
    )
    .execute(&pool)
    .await
    .expect_err("an assessment plan must use the subject version of its offering");
    assert!(assessment_subject_error
        .to_string()
        .contains("course_assessment_plans_offering_subject_context_fkey"));

    let timetable_group_offering_error = sqlx::query(
        r#"UPDATE academic_timetable_entries
           SET learning_offering_id = '70000000-0000-0000-0000-000000000001'
           WHERE id = '83000000-0000-0000-0000-000000000001'"#,
    )
    .execute(&pool)
    .await
    .expect_err("a timetable group must belong to its exact offering");
    assert!(timetable_group_offering_error
        .to_string()
        .contains("academic_timetable_entries_group_offering_context_fkey"));

    let exam_room_context_error = sqlx::query(
        r#"UPDATE academic_exam_day_room_assignments
           SET academic_term_id = '11000000-0000-0000-0000-000000000252'
           WHERE id = '92100000-0000-0000-0000-000000000001'"#,
    )
    .execute(&pool)
    .await
    .expect_err("an exam room assignment must remain in its exam-day context");
    assert!(exam_room_context_error
        .to_string()
        .contains("academic_exam_day_room_assignments_day_context_fkey"));

    let supervision_context_error = sqlx::query(
        r#"UPDATE supervision_observations
           SET learning_group_id = '60000000-0000-0000-0000-000000000024'
           WHERE id = '89000000-0000-0000-0000-000000000001'"#,
    )
    .execute(&pool)
    .await
    .expect_err("a supervision observation group must remain in its cycle context");
    assert!(supervision_context_error
        .to_string()
        .contains("supervision_observations_learning_group_context_fkey"));

    let question_subject_error = sqlx::query(
        r#"UPDATE academic_question_bank_questions
           SET subject_id = (
               SELECT subject_id
               FROM subject_versions
               WHERE id = '20000000-0000-0000-0000-000000000026'
           )
           WHERE id = '93000000-0000-0000-0000-000000000001'"#,
    )
    .execute(&pool)
    .await
    .expect_err("a question must remain attached to its version's canonical subject");
    let question_subject_error_text = question_subject_error.to_string();
    assert!(
        question_subject_error_text
            .contains("academic_question_bank_questions_version_subject_fkey"),
        "unexpected question subject/version error: {question_subject_error_text}"
    );

    let admission_program_error = sqlx::query(
        r#"UPDATE admission_tracks track
           SET study_program_id = (
               SELECT program.id
               FROM study_programs program
               JOIN curriculum_versions version
                 ON version.id = program.curriculum_version_id
               WHERE version.start_academic_year_id =
                     '10000000-0000-0000-0000-000000000024'
               LIMIT 1
           )
           WHERE track.id = '91000000-0000-0000-0000-000000000001'"#,
    )
    .execute(&pool)
    .await
    .expect_err("an admission track must retain the program resolved for its round year");
    let admission_program_error_text = admission_program_error.to_string();
    assert!(
        admission_program_error_text.contains("admission_tracks_program_version_fkey")
            || admission_program_error_text
                .contains("ACADEMIC_ADMISSION_TRACK_PROGRAM_CONTEXT_MISMATCH"),
        "unexpected admission program context error: {admission_program_error_text}"
    );

    let admission_student_error = sqlx::query(
        r#"UPDATE admission_applications
           SET created_user_id = '50000000-0000-0000-0000-000000000003'
           WHERE id = '94000000-0000-0000-0000-000000000001'"#,
    )
    .execute(&pool)
    .await
    .expect_err("an enrolled application must retain its student's academic-year identity");
    let admission_student_error_text = admission_student_error.to_string();
    assert!(
        admission_student_error_text.contains("admission_applications_student_identity_fkey")
            || admission_student_error_text
                .contains("admission_room_assignments_application_identity_fkey"),
        "unexpected admission application/student error: {admission_student_error_text}"
    );

    let admission_placement_error = sqlx::query(
        r#"UPDATE admission_room_assignments assignment
           SET homeroom_placement_id = (
               SELECT placement.id
               FROM homeroom_placements placement
               WHERE placement.academic_year_id = '10000000-0000-0000-0000-000000000026'
               LIMIT 1
           )
           WHERE assignment.id = '94100000-0000-0000-0000-000000000001'"#,
    )
    .execute(&pool)
    .await
    .expect_err("an admission placement must match the student year and homeroom");
    assert!(admission_placement_error
        .to_string()
        .contains("admission_room_assignments_placement_context_fkey"));
}

#[tokio::test]
async fn migration_043_rejects_ambiguous_consumer_and_permission_data() {
    let cases = [
        (
            CutoverFixtureFault::AssessmentReference,
            "ACADEMIC_CORE_043_ASSESSMENT_OFFERING_MISMATCH",
        ),
        (
            CutoverFixtureFault::TimetableReference,
            "ACADEMIC_CORE_043_TIMETABLE_CONTEXT_MISMATCH",
        ),
        (
            CutoverFixtureFault::SupervisionReference,
            "ACADEMIC_CORE_043_SUPERVISION_CONTEXT_MISMATCH",
        ),
        (
            CutoverFixtureFault::AdmissionProgram,
            "ACADEMIC_CORE_043_ADMISSION_PROGRAM_UNRESOLVED",
        ),
        (
            CutoverFixtureFault::AdmissionPlacement,
            "ACADEMIC_CORE_043_ADMISSION_PLACEMENT_UNRESOLVED",
        ),
        (
            CutoverFixtureFault::PermissionMapping,
            "ACADEMIC_CORE_043_PERMISSION_MAPPING_UNRESOLVED",
        ),
    ];

    for (index, (fault, expected_code)) in cases.into_iter().enumerate() {
        let pool = create_named_test_pool(&format!("academic_043_precondition_{index}")).await;
        apply_migrations_through(&pool, 40).await.unwrap();
        seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
            .await
            .unwrap();
        apply_cutover_fixture_fault(&pool, fault).await.unwrap();
        apply_migrations_through(&pool, 42).await.unwrap();

        let error = apply_migrations_through(&pool, 43)
            .await
            .expect_err("ambiguous consumer data must block migration 043");
        assert!(
            error.to_string().contains(expected_code),
            "fault {fault:?} returned an unexpected migration error: {error}"
        );
    }
}

#[tokio::test]
async fn migration_runner_preserves_permission_cutover_evidence_and_active_contract() {
    let pool = create_named_test_pool("academic_core_043_runner_permissions").await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();

    crate::db::migration::run_tenant_migrations(&pool)
        .await
        .expect("the centralized runner must preserve the cutover permission contract");

    let active_target_permissions: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM permissions
           WHERE is_active
             AND code IN (
                 'academic_context.read.school',
                 'academic_year.read.school',
                 'academic_year.manage.school',
                 'academic_term.read.school',
                 'academic_term.manage.school',
                 'academic_catalog.read.school',
                 'academic_catalog.manage.organization_unit',
                 'academic_catalog.manage.organization_tree',
                 'academic_catalog.manage.school',
                 'academic_curriculum.read.organization_unit',
                 'academic_curriculum.read.organization_tree',
                 'academic_curriculum.read.school',
                 'academic_curriculum.manage.organization_unit',
                 'academic_curriculum.manage.organization_tree',
                 'academic_curriculum.manage.school',
                 'homeroom.read.school',
                 'homeroom.manage.school',
                 'student_academic_year.read.school',
                 'student_academic_year.manage.school',
                 'learning_offering.read.assigned',
                 'learning_offering.read.organization_unit',
                 'learning_offering.read.organization_tree',
                 'learning_offering.read.school',
                 'learning_offering.manage.assigned',
                 'learning_offering.manage.organization_unit',
                 'learning_offering.manage.organization_tree',
                 'learning_offering.manage.school'
             )"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(active_target_permissions, 27);

    let legacy_definition_and_grant: (bool, i64) = sqlx::query_as(
        r#"SELECT permission.is_active,
                  (SELECT COUNT(*)
                   FROM role_permissions grant_row
                   WHERE grant_row.permission_id = permission.id)
           FROM permissions permission
           WHERE permission.code = 'academic_structure.read.all'"#,
    )
    .fetch_one(&pool)
    .await
    .expect("inactive legacy permission evidence must remain queryable");
    assert_eq!(legacy_definition_and_grant, (false, 1));

    let target_role_grant_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM role_permissions grant_row
           JOIN permissions permission ON permission.id = grant_row.permission_id
           WHERE grant_row.role_id = 'a1b2c957-bf35-47f8-bbf4-8a67ce6b777f'
             AND permission.code = 'academic_context.read.school'
             AND permission.is_active"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(target_role_grant_count, 1);
}

#[tokio::test]
async fn migration_044_exposes_the_clean_academic_core_runtime_contract() {
    let pool = create_named_test_pool("academic_core_044_runtime_contract").await;
    apply_migrations_through(&pool, 40).await.unwrap();
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .unwrap();
    apply_migrations_through(&pool, 44)
        .await
        .expect("migration 044 must make the clean core API writable before legacy cleanup");

    let nullable_transition_columns: Vec<(String, String, String)> = sqlx::query_as(
        r#"SELECT table_name::text, column_name::text, is_nullable::text
           FROM information_schema.columns
           WHERE table_schema = current_schema()
             AND (table_name, column_name) IN (
                 ('academic_terms', 'legacy_term'),
                 ('bell_schedule_periods', 'academic_year_id')
             )
           ORDER BY table_name, column_name"#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        nullable_transition_columns,
        vec![
            (
                "academic_terms".to_string(),
                "legacy_term".to_string(),
                "YES".to_string(),
            ),
            (
                "bell_schedule_periods".to_string(),
                "academic_year_id".to_string(),
                "YES".to_string(),
            ),
        ]
    );

    let runtime_columns: Vec<(String, String)> = sqlx::query_as(
        r#"SELECT table_name::text, column_name::text
           FROM information_schema.columns
           WHERE table_schema = current_schema()
             AND (table_name, column_name) IN (
                 ('activities', 'archived_at'),
                 ('grade_level_progression_sets', 'row_version'),
                 ('subject_groups', 'row_version'),
                 ('subjects', 'archived_at')
             )
           ORDER BY table_name, column_name"#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(runtime_columns.len(), 4);

    let unique_contract: (bool, bool, bool, bool) = sqlx::query_as(
        r#"SELECT
               to_regclass('study_programs_curriculum_version_id_key') IS NULL,
               to_regclass('study_programs_one_default_per_version') IS NOT NULL,
               EXISTS (
                   SELECT 1 FROM pg_constraint
                   WHERE conname = 'curriculum_course_requirements_program_resource_key'
                     AND connamespace = current_schema()::regnamespace
               ),
               EXISTS (
                   SELECT 1 FROM pg_constraint
                   WHERE conname = 'curriculum_activity_requirements_program_resource_key'
                     AND connamespace = current_schema()::regnamespace
               )"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(unique_contract, (true, true, true, true));

    let audit_delete_action: String = sqlx::query_scalar(
        r#"SELECT confdeltype::text
           FROM pg_constraint
           WHERE conname = 'academic_audit_events_academic_term_id_fkey'
             AND connamespace = current_schema()::regnamespace"#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_delete_action, "n");
}
