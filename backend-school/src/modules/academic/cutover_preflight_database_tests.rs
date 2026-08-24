use super::{
    cutover_preflight::{run_academic_core_preflight, AcademicCorePreflightCode},
    cutover_test_support::{
        all_cutover_fixture_faults, apply_cutover_fixture_fault, apply_migrations_through,
        repair_cutover_fixture_fault, seed_academic_cutover_fixture, CutoverFixture,
    },
};
use crate::test_helpers::create_named_test_pool;
use chrono::NaiveDate;

async fn academic_fixture_checksum(pool: &sqlx::PgPool) -> String {
    sqlx::query_scalar(
        r#"SELECT md5(concat_ws('|',
                (SELECT COUNT(*)::text FROM academic_years),
                (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM academic_semesters),
                (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM subjects),
                (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM student_class_enrollments),
                (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM classroom_courses),
                (SELECT COALESCE(string_agg(id::text, ',' ORDER BY id), '') FROM activity_group_members)
            ))"#,
    )
    .fetch_one(pool)
    .await
    .expect("fixture checksum must be readable")
}

#[tokio::test]
async fn passing_legacy_fixture_reports_exact_counts_without_writes() {
    let pool = create_named_test_pool("academic_preflight_passing").await;
    apply_migrations_through(&pool, 40)
        .await
        .expect("legacy migrations must apply");
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .expect("passing cutover fixture must seed");
    let before = academic_fixture_checksum(&pool).await;

    let report = run_academic_core_preflight(
        &pool,
        "schoolorbit_test_academic_preflight_passing",
        NaiveDate::from_ymd_opt(2025, 8, 23).expect("test cutover date must be valid"),
    )
    .await
    .expect("passing fixture must be queryable");

    let after = academic_fixture_checksum(&pool).await;
    assert!(report.can_cut_over);
    assert_eq!(report.source_counts["academicYears"], 4);
    assert_eq!(report.source_counts["academicTerms"], 9);
    assert_eq!(report.expected_target_counts["stableSubjects"], 2);
    assert_eq!(report.expected_target_counts["subjectVersions"], 3);
    assert_eq!(report.expected_target_counts["studentAcademicYears"], 3);
    assert_eq!(report.expected_target_counts["homeroomPlacements"], 3);
    assert_eq!(report.expected_target_counts["courseGroups"], 3);
    assert_eq!(report.expected_target_counts["activityOfferings"], 2);
    assert_eq!(report.findings.len(), 1);
    assert_eq!(
        report.findings[0].code,
        AcademicCorePreflightCode::HistoricalResultsUnavailable
    );
    assert_eq!(
        before, after,
        "read-only preflight must not mutate source rows"
    );
}

#[tokio::test]
async fn every_blocking_finding_family_is_detected_by_a_legacy_fixture_fault() {
    let pool = create_named_test_pool("academic_preflight_faults").await;
    apply_migrations_through(&pool, 40)
        .await
        .expect("legacy migrations must apply");
    seed_academic_cutover_fixture(&pool, CutoverFixture::Passing)
        .await
        .expect("passing cutover fixture must seed");
    let cutover_date =
        NaiveDate::from_ymd_opt(2025, 8, 23).expect("test cutover date must be valid");

    for (fault, expected_code) in all_cutover_fixture_faults() {
        apply_cutover_fixture_fault(&pool, *fault)
            .await
            .expect("fixture fault must apply");

        let report = run_academic_core_preflight(
            &pool,
            "schoolorbit_test_academic_preflight_faults",
            cutover_date,
        )
        .await
        .expect("fault fixture must remain queryable");
        assert!(
            report
                .findings
                .iter()
                .any(|finding| finding.code == *expected_code),
            "fault {fault:?} must emit {expected_code:?}; got {:?}",
            report
                .findings
                .iter()
                .map(|finding| finding.code)
                .collect::<Vec<_>>()
        );
        assert!(!report.can_cut_over, "fault {fault:?} must block cutover");

        repair_cutover_fixture_fault(&pool, *fault)
            .await
            .expect("fixture fault must be repairable");
    }
}
