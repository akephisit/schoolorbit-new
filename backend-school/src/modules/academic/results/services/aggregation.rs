use super::{
    decimal, decimal_wire, CourseAggregateInput, CourseCreditTotals, CourseOfficialOutcome,
};
use crate::error::AppError;
use bigdecimal::{BigDecimal, RoundingMode};
use std::collections::HashSet;

fn grade(value: &str) -> Result<BigDecimal, AppError> {
    let grade = decimal(value)?;
    let doubled = &grade * BigDecimal::from(2);
    if grade > BigDecimal::from(4) || doubled != doubled.with_scale(0) {
        return Err(AppError::ValidationError(
            "ผลการเรียนต้องอยู่ระหว่าง 0–4 ทีละ 0.5".into(),
        ));
    }
    Ok(grade)
}

/// Aggregates only numeric locked outcomes; exceptional/missing results stay visible.
/// The caller supplies its policy's passing grade and expected unique subject coverage.
pub fn aggregate_course_credits(
    courses: &[CourseAggregateInput],
    passing_grade: &str,
) -> Result<CourseCreditTotals, AppError> {
    let passing_grade = grade(passing_grade)?;
    if passing_grade == BigDecimal::from(0) {
        return Err(AppError::ValidationError(
            "เกณฑ์ผลการเรียนผ่านต้องมากกว่า 0".into(),
        ));
    }
    let mut subjects = HashSet::new();
    let mut attempted = BigDecimal::from(0);
    let mut graded = BigDecimal::from(0);
    let mut earned = BigDecimal::from(0);
    let mut points = BigDecimal::from(0);
    let mut missing = 0;
    let mut exceptional = 0;
    for course in courses {
        if !subjects.insert(course.subject_id) {
            return Err(AppError::ValidationError(
                "พบรายวิชาซ้ำในผลรวมของนักเรียนภาคเรียนเดียวกัน".into(),
            ));
        }
        let credits = decimal(&course.credits)?;
        let has_result = course.result_id.is_some()
            && course.effective_version.is_some_and(|version| version > 0);
        let missing_result = course.result_id.is_none() && course.effective_version.is_none();
        match (&course.outcome, &course.numeric_grade) {
            (Some(CourseOfficialOutcome::Numeric), Some(value)) if has_result => {
                let value = grade(value)?;
                graded += &credits;
                points += &credits * &value;
                if value >= passing_grade {
                    earned += &credits;
                }
            }
            (
                Some(
                    CourseOfficialOutcome::Incomplete
                    | CourseOfficialOutcome::InsufficientAttendance,
                ),
                None,
            ) if has_result => {
                exceptional += 1;
            }
            (None, None) if missing_result => missing += 1,
            _ => {
                return Err(AppError::ValidationError(
                    "รูปแบบผลการเรียนหรือรุ่นข้อมูลต้นทางไม่สอดคล้องกัน".into(),
                ))
            }
        }
        attempted += credits;
    }
    let provisional_gpa = if graded == BigDecimal::from(0) {
        None
    } else {
        Some(decimal_wire(
            &(&points / &graded).with_scale_round(2, RoundingMode::HalfUp),
        ))
    };
    let coverage_complete = !courses.is_empty() && missing == 0;
    Ok(CourseCreditTotals {
        attempted_credits: decimal_wire(&attempted),
        graded_credits: decimal_wire(&graded),
        earned_credits: decimal_wire(&earned),
        unresolved_credits: decimal_wire(&(&attempted - &graded)),
        weighted_grade_points: format!("{points:.4}"),
        provisional_gpa,
        missing_result_count: missing,
        exceptional_result_count: exceptional,
        coverage_complete,
        all_outcomes_numeric: coverage_complete && exceptional == 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn corrected_half_grade_is_a_valid_numeric_outcome() {
        assert_eq!(
            grade("0.50").unwrap(),
            BigDecimal::from(1) / BigDecimal::from(2)
        );
        assert!(grade("0.25").is_err());
    }
}
