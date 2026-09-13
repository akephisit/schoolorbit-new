use super::*;
use crate::{
    middleware::permission::ActorContext,
    modules::academic::{
        cutover_test_support::apply_migrations_through,
        results::aggregate_revision_tests::ready_aggregate_fixture,
    },
};
use sqlx::PgPool;
use uuid::Uuid;

async fn fixture(
    name: &str,
) -> (
    PgPool,
    ActorContext,
    ResultContext,
    Uuid,
    AggregatePolicyVersion,
    AnnualResultRevision,
) {
    let (pool, actor, context, student) = ready_aggregate_fixture(name).await;
    apply_migrations_through(&pool, 69).await.unwrap();
    sqlx::query(
        "UPDATE academic_terms SET included_in_year_result=(id=$2) WHERE academic_year_id=$1",
    )
    .bind(context.academic_year_id)
    .bind(context.academic_term_id)
    .execute(&pool)
    .await
    .unwrap();
    let policy = create_aggregate_policy(
        &pool,
        &actor,
        AggregatePolicyInput {
            name: "E2E-LIFECYCLE-correction-evidence".into(),
            passing_grade: "1".into(),
            minimum_learner_level: 1,
            allow_reviewed_holds: false,
        },
    )
    .await
    .unwrap();
    let annual = lock_sources(&pool, &actor, &context, student, policy.id, None).await;
    (pool, actor, context, student, policy, annual)
}

async fn lock_sources(
    pool: &PgPool,
    actor: &ActorContext,
    context: &ResultContext,
    student: Uuid,
    policy: Uuid,
    previous: Option<i64>,
) -> AnnualResultRevision {
    let preview = preview_aggregate(
        pool,
        actor,
        student,
        &AggregatePreviewQuery {
            academic_year_id: context.academic_year_id,
            academic_term_id: context.academic_term_id,
            policy_id: policy,
        },
    )
    .await
    .unwrap();
    lock_term_aggregate(
        pool,
        actor,
        context,
        student,
        AggregateLockInput {
            policy_id: policy,
            source_checksum: preview.source_checksum,
            expected_revision: previous,
            request_id: Uuid::new_v4(),
            hold_reason: None,
        },
    )
    .await
    .unwrap();
    let preview = preview_annual(pool, actor, context.academic_year_id, student)
        .await
        .unwrap();
    lock_annual(
        pool,
        actor,
        context.academic_year_id,
        student,
        AnnualLockInput {
            source_checksum: preview.source_checksum,
            expected_revision: previous,
            request_id: Uuid::new_v4(),
            hold_reason: None,
        },
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn annual_correction_evidence_tracks_exact_versions_and_keeps_reversals() {
    let (pool, actor, context, student, policy, original) =
        fixture("annual_correction_versions").await;
    let result = original.snapshot.terms[0]
        .revision
        .as_ref()
        .unwrap()
        .snapshot
        .results
        .courses[0]
        .result_id
        .unwrap();
    let before: String = sqlx::query_scalar("SELECT md5((SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM student_academic_years s)::text || (SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM homeroom_placements p)::text)").fetch_one(&pool).await.unwrap();
    let changed = correct_result(
        &pool,
        &actor,
        &context,
        ResultCorrectionInput::Course {
            course_result_id: result,
            outcome: CourseOfficialOutcome::Numeric,
            numeric_grade: Some("4".into()),
            expected_effective_version: 1,
        },
    )
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let evidence = corrections_after_annuals(&mut tx, context.academic_year_id, &[original.id])
        .await
        .unwrap();
    assert_eq!(evidence.len(), 1);
    assert_eq!(evidence[0].annual_revision_id, original.id);
    assert_eq!(evidence[0].academic_term_id, context.academic_term_id);
    assert_eq!(evidence[0].source_effective_version, 1);
    assert_eq!(evidence[0].term_name, original.snapshot.terms[0].term_name);
    assert!(!evidence[0].offering_code.is_empty());
    assert!(!evidence[0].offering_name.is_empty());
    assert!(evidence[0].criterion_name.is_none());
    assert!(evidence[0].evaluation_domain.is_none());
    assert_eq!(evidence[0].result_id, result);
    assert_eq!(
        evidence[0].correction.id,
        changed.corrections.last().unwrap().id
    );
    assert!(
        matches!(&evidence[0].correction.corrected, EffectiveResultValue::Course { outcome: CourseOfficialOutcome::Numeric, numeric_grade: Some(grade) } if grade.parse::<BigDecimal>().unwrap() == BigDecimal::from(4))
    );
    tx.rollback().await.unwrap();
    let latest = lock_sources(&pool, &actor, &context, student, policy.id, Some(1)).await;
    let mut tx = pool.begin().await.unwrap();
    assert!(
        corrections_after_annuals(&mut tx, context.academic_year_id, &[latest.id])
            .await
            .unwrap()
            .is_empty()
    );
    tx.rollback().await.unwrap();
    let reversal = correct_result(
        &pool,
        &actor,
        &context,
        ResultCorrectionInput::Course {
            course_result_id: result,
            outcome: CourseOfficialOutcome::Numeric,
            numeric_grade: Some("0".into()),
            expected_effective_version: 2,
        },
    )
    .await
    .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let evidence =
        corrections_after_annuals(&mut tx, context.academic_year_id, &[original.id, latest.id])
            .await
            .unwrap();
    assert_eq!(evidence.len(), 3);
    assert_eq!(
        evidence
            .iter()
            .filter(|row| row.annual_revision_id == original.id)
            .count(),
        2
    );
    let current = evidence
        .iter()
        .find(|row| row.annual_revision_id == latest.id)
        .unwrap();
    assert_eq!(current.source_effective_version, 2);
    assert_eq!(
        current.correction.id,
        reversal.corrections.last().unwrap().id
    );
    tx.rollback().await.unwrap();
    let after: String = sqlx::query_scalar("SELECT md5((SELECT jsonb_agg(to_jsonb(s) ORDER BY id) FROM student_academic_years s)::text || (SELECT jsonb_agg(to_jsonb(p) ORDER BY id) FROM homeroom_placements p)::text)").fetch_one(&pool).await.unwrap();
    assert_eq!(before, after);
}

#[tokio::test]
async fn annual_correction_evidence_includes_activities_and_both_evaluation_domains() {
    let (pool, actor, context, _, _, annual) = fixture("annual_correction_domains").await;
    let snapshot = &annual.snapshot.terms[0].revision.as_ref().unwrap().snapshot;
    let activity = snapshot.results.activities[0].result_id.unwrap();
    let corrected = correct_result(
        &pool,
        &actor,
        &context,
        ResultCorrectionInput::Activity {
            activity_result_id: activity,
            outcome: ActivityOutcome::Fail,
            expected_effective_version: 1,
        },
    )
    .await
    .unwrap();
    let mut expected = vec![corrected.corrections.last().unwrap().id];
    assert_eq!(snapshot.learner_evaluations.domains.len(), 2);
    for domain in &snapshot.learner_evaluations.domains {
        let criterion = &domain.subjects[0].criteria[0];
        let corrected = correct_result(
            &pool,
            &actor,
            &context,
            ResultCorrectionInput::LearnerEvaluation {
                subject_student_evaluation_id: criterion.id,
                quality_level: 2.try_into().unwrap(),
                expected_effective_version: criterion.row_version,
            },
        )
        .await
        .unwrap();
        expected.push(corrected.corrections.last().unwrap().id);
    }
    let mut tx = pool.begin().await.unwrap();
    let evidence = corrections_after_annuals(&mut tx, context.academic_year_id, &[annual.id])
        .await
        .unwrap();
    assert_eq!(evidence.len(), 3);
    let mut actual: Vec<_> = evidence.iter().map(|row| row.correction.id).collect();
    for row in &evidence {
        assert!(!row.offering_code.is_empty());
        assert!(!row.offering_name.is_empty());
        assert_eq!(row.term_name, annual.snapshot.terms[0].term_name);
        if matches!(
            row.correction.corrected,
            EffectiveResultValue::LearnerEvaluation { .. }
        ) {
            let criterion = snapshot
                .learner_evaluations
                .domains
                .iter()
                .flat_map(|domain| &domain.subjects)
                .flat_map(|subject| &subject.criteria)
                .find(|criterion| criterion.id == row.result_id)
                .unwrap();
            assert_eq!(row.criterion_name.as_deref(), Some(criterion.name.as_str()));
            assert_eq!(row.evaluation_domain, Some(criterion.domain));
        }
    }
    actual.sort();
    expected.sort();
    assert_eq!(actual, expected);
    assert_eq!(
        evidence
            .iter()
            .filter(|row| matches!(
                row.correction.corrected,
                EffectiveResultValue::LearnerEvaluation { .. }
            ))
            .count(),
        2
    );
    assert_eq!(
        evidence
            .iter()
            .filter(|row| matches!(
                row.correction.corrected,
                EffectiveResultValue::Activity {
                    outcome: ActivityOutcome::Fail
                }
            ))
            .count(),
        1
    );
}

#[tokio::test]
async fn annual_correction_evidence_rejects_ambiguous_or_cross_year_sources() {
    let (pool, _, context, _, _, annual) = fixture("annual_correction_scope").await;
    let mut tx = pool.begin().await.unwrap();
    for invalid in [
        vec![],
        vec![annual.id, annual.id],
        vec![Uuid::nil()],
        vec![annual.id; 501],
    ] {
        assert!(matches!(
            corrections_after_annuals(&mut tx, context.academic_year_id, &invalid).await,
            Err(AppError::ValidationError(_))
        ));
    }
    assert!(matches!(
        corrections_after_annuals(&mut tx, Uuid::new_v4(), &[annual.id]).await,
        Err(AppError::NotFound(_))
    ));
    assert!(matches!(
        corrections_after_annuals(&mut tx, context.academic_year_id, &[Uuid::new_v4()]).await,
        Err(AppError::NotFound(_))
    ));
}
