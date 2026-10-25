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
fn current_catalog_revision_uses_start_date_then_revision_without_expiry() {
    let day = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
    let versions = [
        (day.pred_opt().unwrap(), 9),
        (day, 1),
        (day, 2),
        (day.succ_opt().unwrap(), 3),
    ];
    assert_eq!(
        select_current_subject_revision(versions.iter(), day, |version| *version),
        Some(&versions[2])
    );
    assert_eq!(
        select_current_subject_revision(
            versions.iter(),
            day + chrono::Duration::days(400),
            |version| *version
        ),
        Some(&versions[3])
    );
    assert_eq!(
        select_current_subject_revision(
            versions.iter(),
            day - chrono::Duration::days(2),
            |version| *version
        ),
        None
    );
}

#[test]
fn upcoming_catalog_revision_uses_earliest_start_then_highest_revision() {
    let day = NaiveDate::from_ymd_opt(2026, 10, 21).unwrap();
    let versions = [
        (day.succ_opt().unwrap(), 1),
        (day.succ_opt().unwrap(), 2),
        (day + chrono::Duration::days(2), 3),
    ];
    let selected = select_display_version(versions.iter(), day, |v| v.0, |v| v.1).unwrap();
    assert_eq!(selected, (&versions[1], CatalogDisplayState::Upcoming));
}
