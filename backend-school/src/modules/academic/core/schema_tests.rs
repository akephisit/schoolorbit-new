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
               ORDER BY version_no"#,
    )
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
