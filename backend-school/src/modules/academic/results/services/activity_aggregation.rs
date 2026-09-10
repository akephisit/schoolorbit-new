use crate::{
    error::AppError,
    modules::academic::results::models::{
        ActivityAggregateInput, ActivityOutcome, ActivityOutcomeTotals,
    },
};
use std::collections::HashSet;

pub(super) fn aggregate_activity_outcomes(
    activities: &[ActivityAggregateInput],
) -> Result<ActivityOutcomeTotals, AppError> {
    let mut groups = HashSet::new();
    let mut passed = 0;
    let mut failed = 0;
    let mut missing = 0;
    for activity in activities {
        if !groups.insert(activity.learning_group_id) {
            return Err(AppError::ValidationError(
                "พบกลุ่มกิจกรรมซ้ำในผลรวมรายภาค".into(),
            ));
        }
        let has_result = activity.result_id.is_some()
            && activity
                .effective_version
                .is_some_and(|version| version > 0);
        let missing_result = activity.result_id.is_none() && activity.effective_version.is_none();
        match activity.outcome {
            Some(ActivityOutcome::Pass) if has_result => passed += 1,
            Some(ActivityOutcome::Fail) if has_result => failed += 1,
            None if missing_result => missing += 1,
            _ => {
                return Err(AppError::ValidationError(
                    "ผลกิจกรรมและรุ่นข้อมูลต้นทางไม่สอดคล้องกัน".into(),
                ))
            }
        }
    }
    let coverage_complete = !activities.is_empty() && missing == 0;
    Ok(ActivityOutcomeTotals {
        expected_group_count: activities.len(),
        passed_group_count: passed,
        failed_group_count: failed,
        missing_result_count: missing,
        coverage_complete,
        all_passed: coverage_complete && failed == 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::academic::results::models::{ActivityAggregateInput, ActivityOutcome};
    use uuid::Uuid;

    fn activity(outcome: Option<ActivityOutcome>) -> ActivityAggregateInput {
        ActivityAggregateInput {
            learning_group_id: Uuid::new_v4(),
            learning_offering_id: Uuid::new_v4(),
            result_id: outcome.map(|_| Uuid::new_v4()),
            effective_version: outcome.map(|_| 1),
            outcome,
        }
    }

    // Catches treating failure or absence as pass and merging separate groups by offering.
    #[test]
    fn activity_aggregation_keeps_pass_fail_and_missing_distinct() {
        let passed = activity(Some(ActivityOutcome::Pass));
        let mut failed = activity(Some(ActivityOutcome::Fail));
        failed.learning_offering_id = passed.learning_offering_id;
        let rows = vec![passed, failed, activity(None)];
        let totals = aggregate_activity_outcomes(&rows).unwrap();
        assert_eq!(totals.expected_group_count, 3);
        assert_eq!(totals.passed_group_count, 1);
        assert_eq!(totals.failed_group_count, 1);
        assert_eq!(totals.missing_result_count, 1);
        assert!(!totals.coverage_complete);
        assert!(!totals.all_passed);
        let resolved = aggregate_activity_outcomes(&rows[..2]).unwrap();
        assert!(resolved.coverage_complete);
        assert!(!resolved.all_passed);
        let passing = aggregate_activity_outcomes(&rows[..1]).unwrap();
        assert!(passing.coverage_complete);
        assert!(passing.all_passed);
        let mut reversed = rows;
        reversed.reverse();
        assert_eq!(aggregate_activity_outcomes(&reversed).unwrap(), totals);
    }

    #[test]
    fn activity_aggregation_empty_is_not_evidence_of_completion() {
        let totals = aggregate_activity_outcomes(&[]).unwrap();
        assert_eq!(totals.expected_group_count, 0);
        assert_eq!(totals.missing_result_count, 0);
        assert!(!totals.coverage_complete);
        assert!(!totals.all_passed);
    }

    #[test]
    fn activity_aggregation_rejects_duplicate_groups_and_inconsistent_sources() {
        let good = activity(Some(ActivityOutcome::Pass));
        assert!(aggregate_activity_outcomes(&[good.clone(), good.clone()]).is_err());
        let mut bad = good.clone();
        bad.result_id = None;
        assert!(aggregate_activity_outcomes(&[bad]).is_err());
        let mut bad = good.clone();
        bad.effective_version = None;
        assert!(aggregate_activity_outcomes(&[bad]).is_err());
        let mut bad = good.clone();
        bad.effective_version = Some(0);
        assert!(aggregate_activity_outcomes(&[bad]).is_err());
        let mut bad = good;
        bad.outcome = None;
        assert!(aggregate_activity_outcomes(&[bad]).is_err());
    }
}
