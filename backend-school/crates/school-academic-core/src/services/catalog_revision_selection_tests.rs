use super::*;

#[test]
fn bangkok_day_changes_at_seventeen_utc() {
    let before = chrono::DateTime::parse_from_rfc3339("2026-10-21T16:59:59Z")
        .unwrap()
        .to_utc();
    let after = chrono::DateTime::parse_from_rfc3339("2026-10-21T17:00:00Z")
        .unwrap()
        .to_utc();
    assert_eq!(
        subject_catalog_date_at(before),
        NaiveDate::from_ymd_opt(2026, 10, 21).unwrap()
    );
    assert_eq!(
        subject_catalog_date_at(after),
        NaiveDate::from_ymd_opt(2026, 10, 22).unwrap()
    );
}

#[test]
fn current_subject_revision_has_exclusive_ends_and_no_future_or_expired_fallback() {
    let day = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
    let versions = [
        (day, Some(day.succ_opt().unwrap()), 1),
        (day, None, 2),
        (day.succ_opt().unwrap(), None, 3),
    ];
    assert_eq!(
        select_current_subject_revision(versions.iter(), day, |version| *version),
        Some(&versions[1])
    );
    let unavailable = [
        (day.pred_opt().unwrap(), Some(day), 1),
        (day.succ_opt().unwrap(), None, 2),
    ];
    assert_eq!(
        select_current_subject_revision(unavailable.iter(), day, |version| *version),
        None
    );
}
