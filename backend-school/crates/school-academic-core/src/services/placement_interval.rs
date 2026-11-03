//! Civil bounds for a membership already scoped to an explicit academic context.
use chrono::NaiveDate;

/// Annual memberships have no civil lower bound. Actual transfers introduce
/// dated boundaries, including an end on the preceding annual membership.
pub fn contains_date(start: Option<NaiveDate>, end: Option<NaiveDate>, date: NaiveDate) -> bool {
    start.is_none_or(|start| start <= date) && end.is_none_or(|end| date <= end)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn annual_membership_and_actual_transfer_share_inclusive_bounds() {
        let transfer = NaiveDate::from_ymd_opt(2026, 9, 1).unwrap();
        let before = transfer.pred_opt().unwrap();
        assert!(contains_date(None, None, before));
        assert!(contains_date(None, Some(before), before));
        assert!(!contains_date(None, Some(before), transfer));
        assert!(!contains_date(Some(transfer), None, before));
        assert!(contains_date(Some(transfer), None, transfer));
    }
}
