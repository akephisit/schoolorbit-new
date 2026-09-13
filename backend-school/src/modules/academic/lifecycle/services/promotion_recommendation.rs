use crate::{
    error::AppError,
    modules::academic::{lifecycle::models::*, results::models::AnnualResultRevision},
};

use crate::modules::academic::{
    core::services::validate_canonical_decimal, learner_evaluation::models::LearnerEvaluationDomain,
};

pub(crate) fn validate_policy(input: &PromotionPolicyInput) -> Result<(), AppError> {
    if input.name.trim().is_empty()
        || input.name.chars().count() > 200
        || input.rules.is_empty()
        || input.rules.len() > 500
    {
        return Err(AppError::ValidationError(
            "ระบุชื่อเกณฑ์ไม่เกิน 200 ตัวอักษรและกฎ 1–500 รายการ".into(),
        ));
    }
    let mut sources = std::collections::BTreeSet::new();
    for rule in &input.rules {
        validate_rule(rule)?;
        if !sources.insert((rule.from_grade_level_id, rule.from_study_program_id)) {
            return Err(AppError::ValidationError(
                "ระดับชั้นและแผนการเรียนต้นทางต้องมีกฎเดียว".into(),
            ));
        }
    }
    Ok(())
}

fn validate_rule(rule: &PromotionRuleInput) -> Result<(), AppError> {
    if rule.from_grade_level_id.is_nil()
        || rule.from_study_program_id.is_nil()
        || rule.target_grade_level_id.is_some_and(|id| id.is_nil())
        || rule.target_study_program_id.is_some_and(|id| id.is_nil())
        || !(0..=3).contains(&rule.minimum_learner_level)
    {
        return Err(AppError::ValidationError(
            "ระดับชั้น แผนการเรียน หรือระดับคุณภาพขั้นต่ำไม่ถูกต้อง".into(),
        ));
    }
    if rule.minimum_earned_credits.len() > 16 {
        return Err(AppError::ValidationError("หน่วยกิตขั้นต่ำเกินขอบเขต".into()));
    }
    validate_canonical_decimal(&rule.minimum_earned_credits, 2)?;
    let valid = match rule.success_outcome {
        PromotionSuccessOutcome::Promote => {
            rule.target_grade_level_id
                .is_some_and(|id| id != rule.from_grade_level_id)
                && rule.target_study_program_id.is_some()
        }
        PromotionSuccessOutcome::Graduate => {
            rule.target_grade_level_id.is_none() && rule.target_study_program_id.is_none()
        }
    };
    if !valid {
        return Err(AppError::ValidationError(
            "กฎเลื่อนชั้นต้องระบุระดับชั้นใหม่และแผนปลายทาง ส่วนกฎจบการศึกษาต้องไม่มีปลายทาง".into(),
        ));
    }
    Ok(())
}

pub(crate) fn recommend(
    rule: Option<&PromotionRuleInput>,
    annual: Option<&AnnualResultRevision>,
) -> Result<PromotionRecommendation, AppError> {
    use PromotionRecommendationFinding as Finding;
    let review = |finding| PromotionRecommendation {
        suggested_outcome: None,
        target_grade_level_id: None,
        target_study_program_id: None,
        findings: vec![finding],
    };
    let Some(rule) = rule else {
        return Ok(review(Finding::PolicyRuleMissing));
    };
    validate_rule(rule)?;
    let Some(annual) = annual else {
        return Ok(review(Finding::AnnualResultMissing));
    };
    let source = &annual.snapshot;
    if !annual.is_current
        || !source.can_lock
        || source.terms.is_empty()
        || !source.totals.coverage_complete
        || source.totals.missing_result_count > 0
    {
        return Ok(review(Finding::AnnualResultStale));
    }
    let mut terms = Vec::with_capacity(source.terms.len());
    for term in &source.terms {
        let Some(revision) = term.revision.as_ref() else {
            return Ok(review(Finding::AnnualResultStale));
        };
        if !term.is_current
            || !revision.is_current
            || !revision.snapshot.can_lock
            || revision.snapshot.results.academic_year_id != source.academic_year_id
            || revision.snapshot.results.academic_term_id != term.academic_term_id
            || revision.snapshot.results.student_academic_year_id != source.student_academic_year_id
        {
            return Ok(review(Finding::AnnualResultStale));
        }
        terms.push(&revision.snapshot);
    }
    let mut findings = vec![];
    if annual.hold_reason.is_some() || source.needs_hold {
        findings.push(Finding::ReviewedAnnualHold);
    }
    if validate_canonical_decimal(&source.totals.earned_credits, 2)?
        < validate_canonical_decimal(&rule.minimum_earned_credits, 2)?
    {
        findings.push(Finding::InsufficientEarnedCredits);
    }
    if rule.require_no_exceptional_outcomes && source.totals.exceptional_result_count > 0 {
        findings.push(Finding::ExceptionalOutcomes);
    }
    if rule.require_activities_passed
        && terms.iter().any(|term| {
            !term.results.activity_totals.all_passed
                || !term.results.activity_totals.coverage_complete
        })
    {
        findings.push(Finding::ActivitiesNotPassed);
    }
    let mut missing = false;
    let mut below = false;
    for term in terms {
        for domain in [
            LearnerEvaluationDomain::DesirableCharacteristic,
            LearnerEvaluationDomain::ReadingThinkingWriting,
        ] {
            let rows: Vec<_> = term
                .learner_evaluations
                .domains
                .iter()
                .filter(|row| row.domain == domain)
                .collect();
            if rows.len() != 1 {
                missing = true;
                continue;
            }
            let row = rows[0];
            match row.quality_level {
                Some(level)
                    if row.complete
                        && row.missing_subjects.is_empty()
                        && (0..=3).contains(&level) =>
                {
                    below |= level < rule.minimum_learner_level;
                }
                _ => missing = true,
            }
        }
    }
    if missing {
        findings.push(Finding::LearnerEvaluationMissing);
    }
    if below {
        findings.push(Finding::LearnerEvaluationNotPassed);
    }
    let eligible = findings.is_empty();
    Ok(PromotionRecommendation {
        suggested_outcome: eligible.then_some(rule.success_outcome),
        target_grade_level_id: eligible.then_some(rule.target_grade_level_id).flatten(),
        target_study_program_id: eligible.then_some(rule.target_study_program_id).flatten(),
        findings,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::academic::{
        learner_evaluation::{models::*, services::summarize_domains},
        results::{models::*, services::aggregate_course_credits},
    };
    use uuid::Uuid;
    use PromotionRecommendationFinding as Finding;

    fn rule() -> PromotionRuleInput {
        PromotionRuleInput {
            from_grade_level_id: Uuid::from_u128(1),
            from_study_program_id: Uuid::from_u128(2),
            target_grade_level_id: Some(Uuid::from_u128(3)),
            target_study_program_id: Some(Uuid::from_u128(4)),
            success_outcome: PromotionSuccessOutcome::Promote,
            minimum_earned_credits: "0.50".into(),
            require_no_exceptional_outcomes: true,
            require_activities_passed: true,
            minimum_learner_level: 1,
        }
    }

    fn annual_fixture() -> AnnualResultRevision {
        let student = Uuid::from_u128(5);
        let subject = Uuid::from_u128(6);
        let year = Uuid::from_u128(7);
        let term = Uuid::from_u128(8);
        let courses = vec![CourseAggregateInput {
            subject_id: subject,
            learning_offering_id: Uuid::from_u128(9),
            result_id: Some(Uuid::from_u128(10)),
            effective_version: Some(1),
            credits: "0.50".into(),
            outcome: Some(CourseOfficialOutcome::Numeric),
            numeric_grade: Some("4".into()),
        }];
        let totals = aggregate_course_credits(&courses, "1").unwrap();
        let domains = [
            LearnerEvaluationDomain::DesirableCharacteristic,
            LearnerEvaluationDomain::ReadingThinkingWriting,
        ];
        let criteria: Vec<_> = domains
            .iter()
            .enumerate()
            .map(|(index, domain)| LockedCriterionValue {
                id: Uuid::from_u128(20 + index as u128),
                subject_id: subject,
                domain: *domain,
                subject_term_criterion_id: Uuid::from_u128(30 + index as u128),
                school_criterion_id: None,
                name: "Review criterion".into(),
                quality_level: 2,
                row_version: 1,
            })
            .collect();
        let evaluations = StudentEvaluationSummary {
            student_academic_year_id: student,
            policy_version_id: Uuid::from_u128(40),
            domains: summarize_domains(
                &criteria,
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
        let results = TermResultPreview {
            academic_year_id: year,
            academic_term_id: term,
            student_academic_year_id: student,
            passing_grade: "1".into(),
            courses,
            totals: totals.clone(),
            activities: vec![ActivityAggregateInput {
                learning_group_id: Uuid::from_u128(50),
                learning_offering_id: Uuid::from_u128(51),
                result_id: Some(Uuid::from_u128(52)),
                effective_version: Some(1),
                outcome: Some(ActivityOutcome::Pass),
            }],
            activity_totals: ActivityOutcomeTotals {
                expected_group_count: 1,
                passed_group_count: 1,
                failed_group_count: 0,
                missing_result_count: 0,
                coverage_complete: true,
                all_passed: true,
            },
            source_checksum: "a".repeat(64),
        };
        let snapshot = TermAggregatePreview {
            policy: AggregatePolicyVersion {
                id: Uuid::from_u128(60),
                name: "Reviewed source policy".into(),
                passing_grade: "1".into(),
                minimum_learner_level: 1,
                allow_reviewed_holds: true,
                approved_by: Uuid::from_u128(61),
                approved_at: chrono::Utc::now(),
            },
            results,
            learner_evaluations: evaluations,
            blockers: vec![],
            hold_findings: vec![],
            can_lock: true,
            source_checksum: "b".repeat(64),
        };
        AnnualResultRevision {
            id: Uuid::from_u128(70),
            revision: 1,
            snapshot: AnnualResultPreview {
                academic_year_id: year,
                student_academic_year_id: student,
                terms: vec![AnnualTermSource {
                    academic_term_id: term,
                    term_name: "ภาคเรียนที่ 1".into(),
                    sequence: 1,
                    is_current: true,
                    revision: Some(TermAggregateRevision {
                        id: Uuid::from_u128(71),
                        revision: 1,
                        snapshot,
                        official_gpa: Some("4.00".into()),
                        hold_reason: None,
                        locked_by: Uuid::from_u128(61),
                        locked_at: chrono::Utc::now(),
                        is_current: true,
                    }),
                }],
                totals,
                can_lock: true,
                needs_hold: false,
                source_checksum: "c".repeat(64),
            },
            official_gpa: Some("4.00".into()),
            hold_reason: None,
            locked_by: Uuid::from_u128(61),
            locked_at: chrono::Utc::now(),
            is_current: true,
        }
    }

    #[test]
    fn promotion_recommendation_requires_exact_reviewed_inputs() {
        let rule = rule();
        let mut annual = annual_fixture();
        assert_eq!(
            recommend(None, Some(&annual)).unwrap().findings,
            vec![Finding::PolicyRuleMissing]
        );
        assert_eq!(
            recommend(Some(&rule), None).unwrap().findings,
            vec![Finding::AnnualResultMissing]
        );
        annual.is_current = false;
        let result = recommend(Some(&rule), Some(&annual)).unwrap();
        assert!(result.suggested_outcome.is_none() && result.target_grade_level_id.is_none());
        assert_eq!(result.findings, vec![Finding::AnnualResultStale]);
        annual.is_current = true;
        annual.hold_reason = Some("Reviewed pending work".into());
        annual.snapshot.needs_hold = true;
        annual.official_gpa = None;
        let result = recommend(Some(&rule), Some(&annual)).unwrap();
        assert!(result.suggested_outcome.is_none());
        assert_eq!(result.findings, vec![Finding::ReviewedAnnualHold]);
    }

    #[test]
    fn promotion_recommendation_does_not_accept_incomplete_or_cross_context_term_snapshots() {
        let rule = rule();
        for case in 0..11 {
            let mut annual = annual_fixture();
            match case {
                0 => annual.snapshot.can_lock = false,
                1 => annual.snapshot.terms.clear(),
                2 => annual.snapshot.totals.coverage_complete = false,
                3 => annual.snapshot.totals.missing_result_count = 1,
                4 => annual.snapshot.terms[0].revision = None,
                5 => annual.snapshot.terms[0].is_current = false,
                _ => {
                    let revision = annual.snapshot.terms[0].revision.as_mut().unwrap();
                    match case {
                        6 => revision.is_current = false,
                        7 => revision.snapshot.can_lock = false,
                        8 => revision.snapshot.results.student_academic_year_id = Uuid::new_v4(),
                        9 => revision.snapshot.results.academic_year_id = Uuid::new_v4(),
                        _ => revision.snapshot.results.academic_term_id = Uuid::new_v4(),
                    }
                }
            }
            let result = recommend(Some(&rule), Some(&annual)).unwrap();
            assert_eq!(
                result.findings,
                vec![Finding::AnnualResultStale],
                "case {case}"
            );
            assert!(result.suggested_outcome.is_none());
        }
    }

    #[test]
    fn promotion_recommendation_does_not_accept_partial_or_duplicate_learner_domains() {
        let rule = rule();
        for case in 0..5 {
            let mut annual = annual_fixture();
            let domains = &mut annual.snapshot.terms[0]
                .revision
                .as_mut()
                .unwrap()
                .snapshot
                .learner_evaluations
                .domains;
            match case {
                0 => domains[0].quality_level = None,
                1 => domains[0].complete = false,
                2 => domains[0].quality_level = Some(4),
                3 => domains[0].missing_subjects.push(MissingSubject {
                    subject_id: Uuid::new_v4(),
                    reason: "ผลรายวิชายังไม่ครบ".into(),
                }),
                _ => domains.push(domains[0].clone()),
            }
            assert_eq!(
                recommend(Some(&rule), Some(&annual)).unwrap().findings,
                vec![Finding::LearnerEvaluationMissing],
                "case {case}"
            );
        }
    }

    #[test]
    fn promotion_recommendation_compares_exact_credits_and_does_not_mistake_zero_for_missing() {
        let mut rule = rule();
        let mut annual = annual_fixture();
        let passing = recommend(Some(&rule), Some(&annual)).unwrap();
        assert_eq!(
            passing.suggested_outcome,
            Some(PromotionSuccessOutcome::Promote)
        );
        assert_eq!(passing.target_grade_level_id, Some(Uuid::from_u128(3)));
        assert!(passing.findings.is_empty());
        rule.minimum_earned_credits = "0.51".into();
        assert_eq!(
            recommend(Some(&rule), Some(&annual)).unwrap().findings,
            vec![Finding::InsufficientEarnedCredits]
        );
        let term = annual.snapshot.terms[0].revision.as_mut().unwrap();
        term.snapshot.results.courses[0].numeric_grade = Some("0".into());
        term.snapshot.results.totals =
            aggregate_course_credits(&term.snapshot.results.courses, "1").unwrap();
        term.official_gpa = Some("0.00".into());
        annual.snapshot.totals = term.snapshot.results.totals.clone();
        annual.official_gpa = Some("0.00".into());
        assert_eq!(
            recommend(Some(&rule), Some(&annual)).unwrap().findings,
            vec![Finding::InsufficientEarnedCredits]
        );
        rule.minimum_earned_credits = "0".into();
        assert_eq!(
            recommend(Some(&rule), Some(&annual))
                .unwrap()
                .suggested_outcome,
            Some(PromotionSuccessOutcome::Promote)
        );
        rule.success_outcome = PromotionSuccessOutcome::Graduate;
        rule.target_grade_level_id = None;
        rule.target_study_program_id = None;
        let graduation = recommend(Some(&rule), Some(&annual)).unwrap();
        assert_eq!(
            graduation.suggested_outcome,
            Some(PromotionSuccessOutcome::Graduate)
        );
        assert!(
            graduation.target_grade_level_id.is_none()
                && graduation.target_study_program_id.is_none()
        );
    }

    #[test]
    fn promotion_recommendation_requires_both_evaluation_domains_and_explicit_activity_review() {
        let mut rule = rule();
        let mut annual = annual_fixture();
        rule.minimum_learner_level = 3;
        assert_eq!(
            recommend(Some(&rule), Some(&annual)).unwrap().findings,
            vec![Finding::LearnerEvaluationNotPassed]
        );
        rule.minimum_learner_level = 1;
        let term = annual.snapshot.terms[0].revision.as_mut().unwrap();
        term.snapshot.learner_evaluations.domains.pop();
        assert_eq!(
            recommend(Some(&rule), Some(&annual)).unwrap().findings,
            vec![Finding::LearnerEvaluationMissing]
        );
        let mut annual = annual_fixture();
        let term = annual.snapshot.terms[0].revision.as_mut().unwrap();
        term.snapshot.results.activities[0].outcome = Some(ActivityOutcome::Fail);
        term.snapshot.results.activity_totals.all_passed = false;
        term.snapshot.results.activity_totals.failed_group_count = 1;
        term.snapshot.results.activity_totals.passed_group_count = 0;
        term.hold_reason = Some("Reviewed activity".into());
        term.snapshot.hold_findings = vec![AggregateHoldFinding::FailedActivities];
        annual.hold_reason = Some("Reviewed annual activity hold".into());
        annual.snapshot.needs_hold = true;
        annual.official_gpa = None;
        assert_eq!(
            recommend(Some(&rule), Some(&annual)).unwrap().findings,
            vec![Finding::ReviewedAnnualHold, Finding::ActivitiesNotPassed]
        );
        rule.require_activities_passed = false;
        assert_eq!(
            recommend(Some(&rule), Some(&annual)).unwrap().findings,
            vec![Finding::ReviewedAnnualHold]
        );
    }

    #[test]
    fn promotion_recommendation_reports_exceptional_results_without_inventing_a_numeric_grade() {
        let mut rule = rule();
        rule.minimum_earned_credits = "0".into();
        let mut annual = annual_fixture();
        let term = annual.snapshot.terms[0].revision.as_mut().unwrap();
        term.snapshot.results.courses[0].outcome = Some(CourseOfficialOutcome::Incomplete);
        term.snapshot.results.courses[0].numeric_grade = None;
        term.snapshot.results.totals =
            aggregate_course_credits(&term.snapshot.results.courses, "1").unwrap();
        term.snapshot.hold_findings = vec![AggregateHoldFinding::ExceptionalCourseOutcomes];
        term.hold_reason = Some("Reviewed incomplete work".into());
        term.official_gpa = None;
        annual.snapshot.totals = term.snapshot.results.totals.clone();
        annual.snapshot.needs_hold = true;
        annual.official_gpa = None;
        annual.hold_reason = Some("Reviewed pending result".into());
        let result = recommend(Some(&rule), Some(&annual)).unwrap();
        assert!(result.suggested_outcome.is_none());
        assert_eq!(
            result.findings,
            vec![Finding::ReviewedAnnualHold, Finding::ExceptionalOutcomes]
        );
        rule.require_no_exceptional_outcomes = false;
        assert_eq!(
            recommend(Some(&rule), Some(&annual)).unwrap().findings,
            vec![Finding::ReviewedAnnualHold]
        );
    }

    #[test]
    fn promotion_recommendation_rejects_incomplete_ambiguous_or_invalid_policy_rules() {
        let good = PromotionPolicyInput {
            name: "Reviewed school policy".into(),
            rules: vec![rule()],
        };
        assert!(validate_policy(&good).is_ok());
        for name in ["".to_string(), " ".to_string(), "a".repeat(201)] {
            let mut input = good.clone();
            input.name = name;
            assert!(validate_policy(&input).is_err());
        }
        let mut empty = good.clone();
        empty.rules.clear();
        assert!(validate_policy(&empty).is_err());
        let mut duplicate = good.clone();
        duplicate.rules.push(rule());
        assert!(validate_policy(&duplicate).is_err());
        let mut oversized = good.clone();
        oversized.rules = (0..501)
            .map(|index| {
                let mut row = rule();
                row.from_study_program_id = Uuid::from_u128(100 + index);
                row
            })
            .collect();
        assert!(validate_policy(&oversized).is_err());
        for value in ["-1", "01", "1.001", "NaN", "1e2"] {
            let mut input = good.clone();
            input.rules[0].minimum_earned_credits = value.into();
            assert!(validate_policy(&input).is_err());
        }
        for level in [-1, 4] {
            let mut input = good.clone();
            input.rules[0].minimum_learner_level = level;
            assert!(validate_policy(&input).is_err());
        }
        let mut input = good.clone();
        input.rules[0].target_grade_level_id = None;
        assert!(validate_policy(&input).is_err());
        input = good.clone();
        input.rules[0].success_outcome = PromotionSuccessOutcome::Graduate;
        assert!(validate_policy(&input).is_err());
        input = good.clone();
        input.rules[0].target_grade_level_id = Some(input.rules[0].from_grade_level_id);
        assert!(validate_policy(&input).is_err());
        input = good;
        input.rules[0].from_study_program_id = Uuid::nil();
        assert!(validate_policy(&input).is_err());
    }
}
