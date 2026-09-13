use super::{decimal, decimal_wire, AppError, CourseCreditTotals};
use bigdecimal::{BigDecimal, RoundingMode};

pub(super) fn sum_annual_credit_totals(
    sources: &[CourseCreditTotals],
) -> Result<CourseCreditTotals, AppError> {
    let mut attempted = BigDecimal::from(0);
    let mut graded = BigDecimal::from(0);
    let mut earned = BigDecimal::from(0);
    let mut points = BigDecimal::from(0);
    let mut missing = 0usize;
    let mut exceptional = 0usize;
    for source in sources {
        let source_attempted = decimal(&source.attempted_credits)?;
        let source_graded = decimal(&source.graded_credits)?;
        let source_earned = decimal(&source.earned_credits)?;
        let source_unresolved = decimal(&source.unresolved_credits)?;
        let source_points = crate::modules::academic::core::services::validate_canonical_decimal(
            &source.weighted_grade_points,
            4,
        )?;
        if source_graded > source_attempted
            || source_earned > source_graded
            || source_unresolved != &source_attempted - &source_graded
            || source_points > &source_graded * BigDecimal::from(4)
        {
            return Err(AppError::ValidationError(
                "หน่วยกิตหรือผลรวมเกรดในผลรายภาคไม่สอดคล้องกัน".into(),
            ));
        }
        attempted += source_attempted;
        graded += source_graded;
        earned += source_earned;
        points += source_points;
        missing = missing
            .checked_add(source.missing_result_count)
            .ok_or_else(|| AppError::ValidationError("จำนวนผลที่ขาดเกินขอบเขต".into()))?;
        exceptional = exceptional
            .checked_add(source.exceptional_result_count)
            .ok_or_else(|| AppError::ValidationError("จำนวนผลค้างเกินขอบเขต".into()))?;
    }
    let provisional_gpa = if graded == BigDecimal::from(0) {
        None
    } else {
        Some(decimal_wire(
            &(&points / &graded).with_scale_round(2, RoundingMode::HalfUp),
        ))
    };
    let complete = !sources.is_empty()
        && missing == 0
        && sources.iter().all(|source| source.coverage_complete);
    Ok(CourseCreditTotals {
        attempted_credits: decimal_wire(&attempted),
        graded_credits: decimal_wire(&graded),
        earned_credits: decimal_wire(&earned),
        unresolved_credits: decimal_wire(&(&attempted - &graded)),
        weighted_grade_points: format!("{points:.4}"),
        provisional_gpa,
        missing_result_count: missing,
        exceptional_result_count: exceptional,
        coverage_complete: complete,
        all_outcomes_numeric: complete
            && exceptional == 0
            && sources.iter().all(|source| source.all_outcomes_numeric),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn total(
        attempted: &str,
        graded: &str,
        earned: &str,
        unresolved: &str,
        points: &str,
        gpa: Option<&str>,
    ) -> CourseCreditTotals {
        CourseCreditTotals {
            attempted_credits: attempted.into(),
            graded_credits: graded.into(),
            earned_credits: earned.into(),
            unresolved_credits: unresolved.into(),
            weighted_grade_points: points.into(),
            provisional_gpa: gpa.map(str::to_owned),
            missing_result_count: 0,
            exceptional_result_count: 0,
            coverage_complete: true,
            all_outcomes_numeric: true,
        }
    }

    #[test]
    fn annual_calculation_combines_points_and_credits_not_rounded_term_gpas() {
        let first = total("1.00", "1.00", "1.00", "0.00", "4.0000", Some("4.00"));
        let second = total("3.00", "3.00", "3.00", "0.00", "3.0000", Some("1.00"));
        let result = sum_annual_credit_totals(&[first, second]).unwrap();
        assert_eq!(result.provisional_gpa.as_deref(), Some("1.75"));
        assert_eq!(result.weighted_grade_points, "7.0000");
        assert_eq!(result.attempted_credits, "4.00");
        assert_eq!(result.earned_credits, "4.00");
        assert!(result.coverage_complete && result.all_outcomes_numeric);
        let exact = sum_annual_credit_totals(&[
            total("0.25", "0.25", "0.00", "0.00", "0.1250", Some("0.50")),
            total("0.50", "0.50", "0.50", "0.00", "1.7500", Some("3.50")),
        ])
        .unwrap();
        assert_eq!(exact.weighted_grade_points, "1.8750");
        assert_eq!(exact.provisional_gpa.as_deref(), Some("2.50"));
    }

    #[test]
    fn annual_calculation_keeps_zero_missing_and_exceptional_inputs_distinct() {
        let zero = sum_annual_credit_totals(&[total(
            "1.00",
            "1.00",
            "0.00",
            "0.00",
            "0.0000",
            Some("0.00"),
        )])
        .unwrap();
        assert_eq!(zero.provisional_gpa.as_deref(), Some("0.00"));
        let no_credits =
            sum_annual_credit_totals(&[total("0.00", "0.00", "0.00", "0.00", "0.0000", None)])
                .unwrap();
        assert!(no_credits.provisional_gpa.is_none());
        let empty = sum_annual_credit_totals(&[]).unwrap();
        assert!(
            !empty.coverage_complete
                && !empty.all_outcomes_numeric
                && empty.provisional_gpa.is_none()
        );
        let mut missing = total("1.00", "0.00", "0.00", "1.00", "0.0000", None);
        missing.missing_result_count = 1;
        missing.coverage_complete = false;
        missing.all_outcomes_numeric = false;
        let mut exceptional = total("2.00", "0.00", "0.00", "2.00", "0.0000", None);
        exceptional.exceptional_result_count = 1;
        exceptional.all_outcomes_numeric = false;
        let result = sum_annual_credit_totals(&[missing, exceptional]).unwrap();
        assert_eq!(result.unresolved_credits, "3.00");
        assert_eq!(result.missing_result_count, 1);
        assert_eq!(result.exceptional_result_count, 1);
        assert!(!result.coverage_complete && !result.all_outcomes_numeric);
    }

    #[test]
    fn annual_calculation_rejects_inconsistent_stored_credit_totals() {
        for invalid in [
            total("1.00", "2.00", "1.00", "0.00", "2.0000", Some("1.00")),
            total("1.00", "1.00", "2.00", "0.00", "2.0000", Some("2.00")),
            total("1.00", "1.00", "1.00", "1.00", "2.0000", Some("2.00")),
            total("1.00", "1.00", "1.00", "0.00", "5.0000", Some("5.00")),
            total("-1", "0", "0", "0", "0", None),
        ] {
            assert!(sum_annual_credit_totals(&[invalid]).is_err());
        }
    }
}
