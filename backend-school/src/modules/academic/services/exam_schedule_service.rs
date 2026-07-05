#![allow(dead_code)]

use chrono::{Duration, NaiveTime};

use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::BlockedWindow;

#[derive(Debug, PartialEq, Eq)]
pub enum SessionValidationError {
    InvalidDuration,
    EndTimeOverflow,
    BeforeDayStart,
    AfterDayEnd,
    BlockedWindow(String),
}

pub fn add_minutes(start: NaiveTime, minutes: i32) -> Result<NaiveTime, SessionValidationError> {
    if minutes <= 0 {
        return Err(SessionValidationError::InvalidDuration);
    }

    let (end_time, day_delta) = start.overflowing_add_signed(Duration::minutes(i64::from(minutes)));
    if day_delta == 0 {
        Ok(end_time)
    } else {
        Err(SessionValidationError::EndTimeOverflow)
    }
}

pub fn time_ranges_overlap(
    left_start: NaiveTime,
    left_end: NaiveTime,
    right_start: NaiveTime,
    right_end: NaiveTime,
) -> bool {
    left_start < right_end && right_start < left_end
}

pub fn validate_session_window(
    starts_at: NaiveTime,
    duration_minutes: i32,
    day_start: NaiveTime,
    day_end: NaiveTime,
    blocked_windows: &[BlockedWindow],
) -> Result<NaiveTime, SessionValidationError> {
    let ends_at = add_minutes(starts_at, duration_minutes)?;
    if starts_at < day_start {
        return Err(SessionValidationError::BeforeDayStart);
    }
    if ends_at > day_end {
        return Err(SessionValidationError::AfterDayEnd);
    }
    for blocked in blocked_windows {
        if time_ranges_overlap(starts_at, ends_at, blocked.start_time, blocked.end_time) {
            return Err(SessionValidationError::BlockedWindow(blocked.label.clone()));
        }
    }
    Ok(ends_at)
}

pub fn validation_error_to_app_error(error: SessionValidationError) -> AppError {
    match error {
        SessionValidationError::InvalidDuration => {
            AppError::BadRequest("Exam duration must be greater than zero".into())
        }
        SessionValidationError::EndTimeOverflow => {
            AppError::BadRequest("Exam end time is outside the valid day range".into())
        }
        SessionValidationError::BeforeDayStart => {
            AppError::BadRequest("Exam starts before the exam day begins".into())
        }
        SessionValidationError::AfterDayEnd => {
            AppError::BadRequest("Exam ends after the exam day ends".into())
        }
        SessionValidationError::BlockedWindow(label) => {
            AppError::BadRequest(format!("Exam overlaps blocked window: {label}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveTime;

    fn t(value: &str) -> NaiveTime {
        NaiveTime::parse_from_str(value, "%H:%M").unwrap()
    }

    #[test]
    fn computes_end_time_from_duration() {
        assert_eq!(add_minutes(t("08:30"), 90).unwrap(), t("10:00"));
    }

    #[test]
    fn rejects_zero_duration() {
        assert_eq!(
            add_minutes(t("08:30"), 0),
            Err(SessionValidationError::InvalidDuration)
        );
    }

    #[test]
    fn rejects_negative_duration() {
        assert_eq!(
            add_minutes(t("08:30"), -30),
            Err(SessionValidationError::InvalidDuration)
        );
    }

    #[test]
    fn rejects_end_time_overflow() {
        assert_eq!(
            add_minutes(t("23:30"), 60),
            Err(SessionValidationError::EndTimeOverflow)
        );
    }

    #[test]
    fn detects_half_open_time_overlap() {
        assert!(time_ranges_overlap(
            t("08:30"),
            t("10:00"),
            t("09:59"),
            t("11:00")
        ));
        assert!(!time_ranges_overlap(
            t("08:30"),
            t("10:00"),
            t("10:00"),
            t("11:00")
        ));
    }

    #[test]
    fn rejects_placement_outside_day_window() {
        let outcome = validate_session_window(
            t("08:00"),
            120,
            t("08:30"),
            t("16:00"),
            &[BlockedWindow {
                id: None,
                label: "Lunch".to_string(),
                start_time: t("12:00"),
                end_time: t("13:00"),
            }],
        );
        assert!(matches!(
            outcome,
            Err(SessionValidationError::BeforeDayStart)
        ));
    }

    #[test]
    fn rejects_placement_after_day_end() {
        let outcome = validate_session_window(t("15:30"), 60, t("08:30"), t("16:00"), &[]);
        assert!(matches!(outcome, Err(SessionValidationError::AfterDayEnd)));
    }

    #[test]
    fn rejects_placement_across_blocked_window() {
        let outcome = validate_session_window(
            t("11:30"),
            90,
            t("08:30"),
            t("16:00"),
            &[BlockedWindow {
                id: None,
                label: "Lunch".to_string(),
                start_time: t("12:00"),
                end_time: t("13:00"),
            }],
        );
        assert!(matches!(
            outcome,
            Err(SessionValidationError::BlockedWindow(_))
        ));
    }
}
