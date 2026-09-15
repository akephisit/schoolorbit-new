use crate::modules::academic::{
    cutover_test_support::apply_migrations_through,
    results::aggregate_revision_tests::ready_aggregate_fixture,
};
use school_academic_results::{models::*, services::*};
use uuid::Uuid;

#[tokio::test]
async fn promotion_annual_source_batch_preserves_missing_zero_and_stale_evidence() {
    let (pool, actor, context, student) = ready_aggregate_fixture("promotion_annual_sources").await;
    apply_migrations_through(&pool, 69).await.unwrap();
    sqlx::query(
        "UPDATE academic_terms SET included_in_year_result=(id=$2) WHERE academic_year_id=$1",
    )
    .bind(context.academic_year_id)
    .bind(context.academic_term_id)
    .execute(&pool)
    .await
    .unwrap();
    let extra_user = Uuid::new_v4();
    sqlx::query("INSERT INTO users(id,username,password_hash,first_name,last_name,user_type,status) VALUES ($1,$2,'fixture-not-a-login','E2E-LIFECYCLE','No result','student','active')")
            .bind(extra_user).bind(format!("promotion-no-result-{extra_user}")).execute(&pool).await.unwrap();
    let extra_student = Uuid::new_v4();
    sqlx::query("INSERT INTO student_academic_years(id,student_id,academic_year_id,grade_level_id,study_program_id,status) SELECT $1,$2,academic_year_id,grade_level_id,study_program_id,'active' FROM student_academic_years WHERE id=$3")
            .bind(extra_student).bind(extra_user).bind(student).execute(&pool).await.unwrap();
    let students: Vec<Uuid> = sqlx::query_scalar(
        "SELECT id FROM student_academic_years WHERE academic_year_id=$1 ORDER BY id LIMIT 2",
    )
    .bind(context.academic_year_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(students.len(), 2);
    let mut tx = pool.begin().await.unwrap();
    let empty = promotion_annual_sources(&mut tx, context.academic_year_id, &students)
        .await
        .unwrap();
    assert_eq!(empty.len(), 2);
    assert!(empty.values().all(Option::is_none));
    for bad in [
        vec![],
        vec![student, student],
        vec![Uuid::new_v4()],
        vec![student; 501],
    ] {
        assert!(
            promotion_annual_sources(&mut tx, context.academic_year_id, &bad)
                .await
                .is_err()
        );
    }
    assert!(promotion_annual_sources(&mut tx, Uuid::new_v4(), &students)
        .await
        .is_err());
    tx.rollback().await.unwrap();
    let policy = create_aggregate_policy(
        &pool,
        &actor,
        AggregatePolicyInput {
            name: "Promotion evidence".into(),
            passing_grade: "1".into(),
            minimum_learner_level: 1,
            allow_reviewed_holds: false,
        },
    )
    .await
    .unwrap();
    let query = AggregatePreviewQuery {
        academic_year_id: context.academic_year_id,
        academic_term_id: context.academic_term_id,
        policy_id: policy.id,
    };
    let preview = preview_aggregate(&pool, &actor, student, &query)
        .await
        .unwrap();
    lock_term_aggregate(
        &pool,
        &actor,
        &context,
        student,
        AggregateLockInput {
            policy_id: policy.id,
            source_checksum: preview.source_checksum,
            expected_revision: None,
            request_id: Uuid::new_v4(),
            hold_reason: None,
        },
    )
    .await
    .unwrap();
    let annual = preview_annual(&pool, &actor, context.academic_year_id, student)
        .await
        .unwrap();
    let locked = lock_annual(
        &pool,
        &actor,
        context.academic_year_id,
        student,
        AnnualLockInput {
            expected_revision: None,
            source_checksum: annual.source_checksum,
            request_id: Uuid::new_v4(),
            hold_reason: None,
        },
    )
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let batch = promotion_annual_sources(&mut tx, context.academic_year_id, &students)
        .await
        .unwrap();
    assert!(batch.get(&extra_student).unwrap().is_none());
    let row = batch.get(&student).unwrap().as_ref().unwrap();
    assert_eq!(row.id, locked.id);
    assert_eq!(row.official_gpa.as_deref(), Some("0.00"));
    assert!(row.is_current);
    tx.rollback().await.unwrap();
    let course = &preview.results.courses[0];
    correct_result(
        &pool,
        &actor,
        &context,
        ResultCorrectionInput::Course {
            course_result_id: course.result_id.unwrap(),
            outcome: CourseOfficialOutcome::Numeric,
            numeric_grade: Some("4".into()),
            expected_effective_version: 1,
        },
    )
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let stale = promotion_annual_sources(&mut tx, context.academic_year_id, &[student])
        .await
        .unwrap();
    let row = stale.get(&student).unwrap().as_ref().unwrap();
    assert!(!row.is_current);
    assert_eq!(row.id, locked.id);
    assert_eq!(row.official_gpa.as_deref(), Some("0.00"));
    tx.rollback().await.unwrap();
}
