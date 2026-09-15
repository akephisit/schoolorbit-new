use crate::modules::academic::results::services_tests::fixture;
use school_academic_results::{models::*, services::*};
use uuid::Uuid;

#[tokio::test]
async fn closure_coverage_selected_students_excludes_other_learners_without_inventing_coverage() {
    let (pool, _, context, _) = fixture("closure_coverage_selected").await;
    crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 68)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let all = term_closure_coverage(&mut tx, &context).await.unwrap();
    assert!(!all.students.is_empty());
    let first = all.students[0].student_academic_year_id;
    let selected = term_closure_coverage_for_students(&mut tx, &context, &[first])
        .await
        .unwrap();
    assert_eq!(selected.students.len(), 1);
    assert_eq!(selected.students[0].student_academic_year_id, first);
    assert!(!selected.ready);
    assert!(
        term_closure_coverage_for_students(&mut tx, &context, &[Uuid::new_v4()])
            .await
            .unwrap()
            .students
            .is_empty()
    );
}

#[tokio::test]
async fn closure_coverage_summer_does_not_require_results_for_nonparticipants() {
    let (pool, _, context, _) = fixture("closure_coverage_summer").await;
    crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 68)
        .await
        .unwrap();
    let term:Uuid=sqlx::query_scalar(
            "INSERT INTO academic_terms(academic_year_id,sequence_no,code,name,term_type,start_date,planned_end_date,bell_schedule_id,status,included_in_year_result,blocks_year_closure)
             SELECT academic_year_id,(SELECT max(sequence_no)+1 FROM academic_terms WHERE academic_year_id=$1),'summer-test','ภาคฤดูร้อนทดสอบ','summer',start_date,NULL,bell_schedule_id,'planning',true,true
             FROM academic_terms WHERE id=$2 RETURNING id",
        ).bind(context.academic_year_id).bind(context.academic_term_id).fetch_one(&pool).await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let summer = term_closure_coverage(
        &mut tx,
        &ResultContext {
            academic_year_id: context.academic_year_id,
            academic_term_id: term,
        },
    )
    .await
    .unwrap();
    assert!(
        summer.students.is_empty(),
        "summer coverage comes from its participants, not every student in the year"
    );
    assert!(
        !summer.ready,
        "an empty activated term must not silently become complete"
    );
    let regular = term_closure_coverage(&mut tx, &context).await.unwrap();
    assert!(
        !regular.students.is_empty(),
        "ordinary term coverage still includes year enrollment"
    );
}

#[tokio::test]
async fn closure_coverage_requires_revisions_for_expected_students() {
    let (pool, _, context, _) = fixture("closure_coverage_missing").await;
    crate::modules::academic::cutover_test_support::apply_migrations_through(&pool, 67)
        .await
        .unwrap();
    let expected: Vec<uuid::Uuid> = sqlx::query_scalar(
            "SELECT id FROM student_academic_years WHERE academic_year_id=$1 AND status IN ('active','completed') ORDER BY id"
        ).bind(context.academic_year_id).fetch_all(&pool).await.unwrap();
    assert!(!expected.is_empty());
    let mut tx = pool.begin().await.unwrap();
    let coverage = term_closure_coverage(&mut tx, &context).await.unwrap();
    assert!(!coverage.ready);
    for student in expected {
        let row = coverage
            .students
            .iter()
            .find(|row| row.student_academic_year_id == student)
            .expect("unlocked enrolled students must not disappear from closure readiness");
        assert!(row.revision_id.is_none());
        assert!(!row.is_current);
    }
    assert_eq!(coverage.source_checksum.len(), 64);
    let mut foreign = context;
    foreign.academic_year_id = uuid::Uuid::new_v4();
    assert!(term_closure_coverage(&mut tx, &foreign).await.is_err());
}
