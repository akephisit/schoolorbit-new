use super::{aggregate_policy::load_aggregate_policy, *};
use crate::{
    middleware::permission::ActorContext,
    modules::academic::learner_evaluation::{
        models::{EvaluationContext, LearnerEvaluationDomain, StudentEvaluationSummary},
        services::summarize_student_term_in_transaction,
    },
    policies::academic_aggregate_access_policy::require_aggregate_read,
};
use sqlx::PgPool;
use std::collections::BTreeSet;
use uuid::Uuid;

fn assess_sources(
    results: &TermResultPreview,
    learner: &StudentEvaluationSummary,
    policy: &AggregatePolicyVersion,
) -> (Vec<AggregateBlocker>, Vec<AggregateHoldFinding>) {
    let mut blockers = vec![];
    let mut holds = vec![];
    if results.courses.is_empty() {
        blockers.push(AggregateBlocker::NoCourseCoverage);
    }
    if results.totals.missing_result_count > 0 {
        blockers.push(AggregateBlocker::MissingCourseResults);
    }
    if results.activity_totals.missing_result_count > 0 {
        blockers.push(AggregateBlocker::MissingActivityResults);
    }
    let expected: BTreeSet<_> = results
        .courses
        .iter()
        .map(|course| course.subject_id)
        .collect();
    let learner_complete = learner.domains.len() == 2
        && [
            LearnerEvaluationDomain::DesirableCharacteristic,
            LearnerEvaluationDomain::ReadingThinkingWriting,
        ]
        .iter()
        .all(|domain| {
            learner
                .domains
                .iter()
                .filter(|row| row.domain == *domain)
                .count()
                == 1
                && learner.domains.iter().any(|row| {
                    row.domain == *domain
                        && row.complete
                        && row.quality_level.is_some()
                        && row
                            .subjects
                            .iter()
                            .map(|subject| subject.subject_id)
                            .collect::<BTreeSet<_>>()
                            == expected
                })
        });
    if !learner_complete {
        blockers.push(AggregateBlocker::MissingLearnerEvaluations);
    }
    if results.totals.exceptional_result_count > 0 {
        holds.push(AggregateHoldFinding::ExceptionalCourseOutcomes);
    }
    if results.activity_totals.failed_group_count > 0 {
        holds.push(AggregateHoldFinding::FailedActivities);
    }
    if learner.domains.iter().any(|domain| {
        domain
            .quality_level
            .is_some_and(|level| level < policy.minimum_learner_level)
    }) {
        holds.push(AggregateHoldFinding::FailedLearnerEvaluations);
    }
    if !holds.is_empty() && !policy.allow_reviewed_holds {
        blockers.push(AggregateBlocker::ReviewedHoldNotAllowed);
    }
    (blockers, holds)
}

pub(super) async fn aggregate_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    student: Uuid,
    query: &AggregatePreviewQuery,
) -> Result<TermAggregatePreview, AppError> {
    let policy = load_aggregate_policy(tx, query.policy_id).await?;
    let results = preview_student_term_in_transaction(
        tx,
        student,
        &TermResultPreviewQuery {
            academic_year_id: query.academic_year_id,
            academic_term_id: query.academic_term_id,
            passing_grade: policy.passing_grade.clone(),
        },
    )
    .await?;
    let learner_evaluations = summarize_student_term_in_transaction(
        tx,
        &EvaluationContext {
            academic_year_id: query.academic_year_id,
            academic_term_id: query.academic_term_id,
        },
        student,
    )
    .await?;
    let (blockers, hold_findings) = assess_sources(&results, &learner_evaluations, &policy);
    let source_checksum = hash(&(&policy, &results, &learner_evaluations))?;
    Ok(TermAggregatePreview {
        policy,
        results,
        learner_evaluations,
        can_lock: blockers.is_empty(),
        blockers,
        hold_findings,
        source_checksum,
    })
}

pub async fn preview_aggregate(
    pool: &PgPool,
    actor: &ActorContext,
    student: Uuid,
    query: &AggregatePreviewQuery,
) -> Result<TermAggregatePreview, AppError> {
    require_aggregate_read(actor)?;
    let mut tx = pool.begin().await?;
    sqlx::query("SET TRANSACTION ISOLATION LEVEL REPEATABLE READ READ ONLY")
        .execute(&mut *tx)
        .await?;
    let preview = aggregate_in_transaction(&mut tx, student, query).await?;
    tx.commit().await?;
    Ok(preview)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::academic::learner_evaluation::{
        models::LockedCriterionValue, services::summarize_domains,
    };

    fn source_fixture(
        level: i16,
    ) -> (
        TermResultPreview,
        StudentEvaluationSummary,
        AggregatePolicyVersion,
    ) {
        let subject = Uuid::new_v4();
        let student = Uuid::new_v4();
        let courses = vec![CourseAggregateInput {
            subject_id: subject,
            learning_offering_id: Uuid::new_v4(),
            result_id: Some(Uuid::new_v4()),
            effective_version: Some(1),
            credits: "1".into(),
            outcome: Some(CourseOfficialOutcome::Numeric),
            numeric_grade: Some("0".into()),
        }];
        let results = TermResultPreview {
            academic_year_id: Uuid::new_v4(),
            academic_term_id: Uuid::new_v4(),
            student_academic_year_id: student,
            passing_grade: "1".into(),
            totals: aggregate_course_credits(&courses, "1").unwrap(),
            courses,
            activities: vec![],
            activity_totals: super::super::activity_aggregation::aggregate_activity_outcomes(&[])
                .unwrap(),
            source_checksum: "a".repeat(64),
        };
        let domains = [
            LearnerEvaluationDomain::DesirableCharacteristic,
            LearnerEvaluationDomain::ReadingThinkingWriting,
        ];
        let rows: Vec<_> = domains
            .iter()
            .map(|domain| LockedCriterionValue {
                id: Uuid::new_v4(),
                subject_id: subject,
                domain: *domain,
                subject_term_criterion_id: Uuid::new_v4(),
                school_criterion_id: None,
                name: "Criterion".into(),
                quality_level: level,
                row_version: 1,
            })
            .collect();
        let learner = StudentEvaluationSummary {
            student_academic_year_id: student,
            policy_version_id: Uuid::new_v4(),
            domains: summarize_domains(
                &rows,
                &[subject],
                &domains.map(|domain| (subject, domain)),
                &[
                    (0, "0".into()),
                    (1, "1".into()),
                    (2, "2".into()),
                    (3, "3".into()),
                ],
            )
            .unwrap(),
        };
        let policy = AggregatePolicyVersion {
            id: Uuid::new_v4(),
            name: "Policy".into(),
            passing_grade: "1".into(),
            minimum_learner_level: 1,
            allow_reviewed_holds: false,
            approved_by: Uuid::new_v4(),
            approved_at: chrono::Utc::now(),
        };
        (results, learner, policy)
    }

    #[test]
    fn numeric_zero_is_complete_but_recorded_failures_need_policy_review() {
        let (results, learner, policy) = source_fixture(3);
        assert_eq!(
            assess_sources(&results, &learner, &policy),
            (vec![], vec![])
        );
        let (mut results, learner, mut policy) = source_fixture(0);
        results.courses[0].outcome = Some(CourseOfficialOutcome::Incomplete);
        results.courses[0].numeric_grade = None;
        results.totals = aggregate_course_credits(&results.courses, "1").unwrap();
        results.activities = vec![ActivityAggregateInput {
            learning_group_id: Uuid::new_v4(),
            learning_offering_id: Uuid::new_v4(),
            result_id: Some(Uuid::new_v4()),
            effective_version: Some(1),
            outcome: Some(ActivityOutcome::Fail),
        }];
        results.activity_totals =
            super::super::activity_aggregation::aggregate_activity_outcomes(&results.activities)
                .unwrap();
        let (blockers, holds) = assess_sources(&results, &learner, &policy);
        assert_eq!(blockers, vec![AggregateBlocker::ReviewedHoldNotAllowed]);
        assert_eq!(
            holds,
            vec![
                AggregateHoldFinding::ExceptionalCourseOutcomes,
                AggregateHoldFinding::FailedActivities,
                AggregateHoldFinding::FailedLearnerEvaluations
            ]
        );
        policy.allow_reviewed_holds = true;
        assert!(assess_sources(&results, &learner, &policy).0.is_empty());
    }

    #[test]
    fn held_policy_never_overrides_missing_sources_or_missing_domain_coverage() {
        let (mut results, mut learner, mut policy) = source_fixture(3);
        policy.allow_reviewed_holds = true;
        results.courses[0].outcome = None;
        results.courses[0].numeric_grade = None;
        results.courses[0].result_id = None;
        results.courses[0].effective_version = None;
        results.totals = aggregate_course_credits(&results.courses, "1").unwrap();
        results.activities = vec![ActivityAggregateInput {
            learning_group_id: Uuid::new_v4(),
            learning_offering_id: Uuid::new_v4(),
            result_id: None,
            effective_version: None,
            outcome: None,
        }];
        results.activity_totals =
            super::super::activity_aggregation::aggregate_activity_outcomes(&results.activities)
                .unwrap();
        learner.domains.pop();
        assert_eq!(
            assess_sources(&results, &learner, &policy).0,
            vec![
                AggregateBlocker::MissingCourseResults,
                AggregateBlocker::MissingActivityResults,
                AggregateBlocker::MissingLearnerEvaluations
            ]
        );
        results.courses.clear();
        results.totals = aggregate_course_credits(&[], "1").unwrap();
        assert!(assess_sources(&results, &learner, &policy)
            .0
            .contains(&AggregateBlocker::NoCourseCoverage));
        let (results, mut learner, policy) = source_fixture(3);
        learner.domains[0].subjects[0].subject_id = Uuid::new_v4();
        assert_eq!(
            assess_sources(&results, &learner, &policy).0,
            vec![AggregateBlocker::MissingLearnerEvaluations]
        );
    }

    #[test]
    fn unknown_hold_policy_fields_are_rejected() {
        assert!(
            serde_json::from_value::<AggregatePolicyInput>(serde_json::json!({
                "name": "Policy", "passingGrade": "1", "minimumLearnerLevel": 1,
                "allowReviewedHolds": false, "ignoreMissingResults": true
            }))
            .is_err()
        );
    }
}
