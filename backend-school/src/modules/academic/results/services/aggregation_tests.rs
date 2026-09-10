use super::*;
use uuid::Uuid;

fn course(
    credits: &str,
    grade: Option<&str>,
    outcome: Option<CourseOfficialOutcome>,
) -> CourseAggregateInput {
    CourseAggregateInput {
        subject_id: Uuid::new_v4(),
        learning_offering_id: Uuid::new_v4(),
        result_id: outcome.map(|_| Uuid::new_v4()),
        effective_version: outcome.map(|_| 1),
        credits: credits.into(),
        outcome,
        numeric_grade: grade.map(str::to_owned),
    }
}

#[test]
fn aggregation_weights_credits_instead_of_averaging_grades() {
    let courses = vec![
        course("1.5", Some("4"), Some(CourseOfficialOutcome::Numeric)),
        course("0.5", Some("2"), Some(CourseOfficialOutcome::Numeric)),
    ];
    let totals = aggregate_course_credits(&courses, "1").unwrap();
    assert_eq!(totals.attempted_credits, "2.00");
    assert_eq!(totals.graded_credits, "2.00");
    assert_eq!(totals.earned_credits, "2.00");
    assert_eq!(totals.weighted_grade_points, "7.0000");
    assert_eq!(totals.provisional_gpa.as_deref(), Some("3.50"));
    assert!(totals.coverage_complete);
    assert!(totals.all_outcomes_numeric);
    let mut reversed = courses;
    reversed.reverse();
    assert_eq!(aggregate_course_credits(&reversed, "1").unwrap(), totals);
}

#[test]
fn aggregation_zero_missing_and_exceptional_results_are_distinct() {
    let courses = vec![
        course("1", Some("0"), Some(CourseOfficialOutcome::Numeric)),
        course("1.5", None, None),
        course("0.5", None, Some(CourseOfficialOutcome::Incomplete)),
        course(
            "0",
            None,
            Some(CourseOfficialOutcome::InsufficientAttendance),
        ),
    ];
    let totals = aggregate_course_credits(&courses, "1").unwrap();
    assert_eq!(totals.attempted_credits, "3.00");
    assert_eq!(totals.graded_credits, "1.00");
    assert_eq!(totals.earned_credits, "0.00");
    assert_eq!(totals.unresolved_credits, "2.00");
    assert_eq!(totals.missing_result_count, 1);
    assert_eq!(totals.exceptional_result_count, 2);
    assert_eq!(totals.provisional_gpa.as_deref(), Some("0.00"));
    assert!(!totals.coverage_complete);
    assert!(!totals.all_outcomes_numeric);
    let exceptional = aggregate_course_credits(&courses[2..], "1").unwrap();
    assert!(exceptional.coverage_complete);
    assert!(!exceptional.all_outcomes_numeric);
    assert_eq!(exceptional.provisional_gpa, None);
}

#[test]
fn aggregation_empty_or_zero_credit_data_never_fabricates_gpa() {
    let empty = aggregate_course_credits(&[], "1").unwrap();
    assert!(!empty.coverage_complete);
    assert!(!empty.all_outcomes_numeric);
    assert_eq!(empty.provisional_gpa, None);
    let zero = aggregate_course_credits(
        &[course("0", Some("4"), Some(CourseOfficialOutcome::Numeric))],
        "1",
    )
    .unwrap();
    assert!(zero.coverage_complete);
    assert_eq!(zero.provisional_gpa, None);
}

#[test]
fn aggregation_uses_explicit_passing_grade_and_rounds_only_the_final_ratio() {
    let courses = vec![
        course("0.01", Some("1.5"), Some(CourseOfficialOutcome::Numeric)),
        course("0.03", Some("2"), Some(CourseOfficialOutcome::Numeric)),
    ];
    let totals = aggregate_course_credits(&courses, "2").unwrap();
    assert_eq!(totals.weighted_grade_points, "0.0750");
    assert_eq!(totals.earned_credits, "0.03");
    assert_eq!(totals.provisional_gpa.as_deref(), Some("1.88"));
    assert_eq!(
        aggregate_course_credits(&courses, "1")
            .unwrap()
            .earned_credits,
        "0.04"
    );
}

#[test]
fn aggregation_rejects_duplicate_subjects_and_inconsistent_result_shapes() {
    let good = course("1", Some("4"), Some(CourseOfficialOutcome::Numeric));
    assert!(aggregate_course_credits(&[good.clone(), good.clone()], "1").is_err());
    let mut bad = good.clone();
    bad.numeric_grade = None;
    assert!(aggregate_course_credits(&[bad], "1").is_err());
    let mut bad = good.clone();
    bad.outcome = Some(CourseOfficialOutcome::Incomplete);
    assert!(aggregate_course_credits(&[bad], "1").is_err());
    let mut bad = good.clone();
    bad.result_id = None;
    assert!(aggregate_course_credits(&[bad], "1").is_err());
    let mut bad = good.clone();
    bad.effective_version = Some(0);
    assert!(aggregate_course_credits(&[bad], "1").is_err());
    let mut bad = good;
    bad.outcome = None;
    bad.numeric_grade = None;
    assert!(aggregate_course_credits(&[bad], "1").is_err());
}

#[test]
fn aggregation_rejects_invalid_decimal_credits_grades_and_thresholds() {
    for invalid in ["-1", "NaN", "1e2", "01", "1.001", " 1", "100000000"] {
        assert!(
            aggregate_course_credits(
                &[course(
                    invalid,
                    Some("4"),
                    Some(CourseOfficialOutcome::Numeric)
                )],
                "1"
            )
            .is_err(),
            "{invalid}"
        );
    }
    for invalid in ["-1", "NaN", "5", "0.25", "1.001", " 1"] {
        assert!(
            aggregate_course_credits(
                &[course(
                    "1",
                    Some(invalid),
                    Some(CourseOfficialOutcome::Numeric)
                )],
                "1"
            )
            .is_err(),
            "{invalid}"
        );
    }
    for invalid in ["0", "0.25", "5", "NaN", "01"] {
        assert!(aggregate_course_credits(&[], invalid).is_err(), "{invalid}");
    }
}
