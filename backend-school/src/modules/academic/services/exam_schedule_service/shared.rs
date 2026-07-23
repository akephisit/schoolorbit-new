use std::collections::HashSet;

use chrono::{Duration, NaiveTime, Timelike};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::academic::models::exam_schedule::BlockedWindow;

const EXAM_SESSION_SLOT_MINUTES: u32 = 5;
const EXAM_SESSION_CLASSROOM_LOCK_NAMESPACE: i64 = 0x4558_5343_4C52_0000;
const EXAM_SESSION_ROOM_LOCK_NAMESPACE: i64 = 0x4558_5352_4F4D_0000;
const EXAM_INVIGILATOR_STAFF_LOCK_NAMESPACE: i64 = 0x4558_5349_4E56_0000;

#[derive(Debug, PartialEq, Eq)]
pub(super) enum SessionValidationError {
    InvalidDuration,
    EndTimeOverflow,
    StartTimeOutsideSlot,
    BeforeDayStart,
    AfterDayEnd,
    BlockedWindow(String),
}

pub(super) fn add_minutes(
    start: NaiveTime,
    minutes: i32,
) -> Result<NaiveTime, SessionValidationError> {
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

pub(super) fn time_ranges_overlap(
    left_start: NaiveTime,
    left_end: NaiveTime,
    right_start: NaiveTime,
    right_end: NaiveTime,
) -> bool {
    left_start < right_end && right_start < left_end
}

fn is_exam_session_start_on_slot(starts_at: NaiveTime) -> bool {
    starts_at
        .num_seconds_from_midnight()
        .is_multiple_of(EXAM_SESSION_SLOT_MINUTES * 60)
}

#[derive(Debug, Clone)]
pub(super) struct CandidateSession {
    pub(super) session_id: Option<Uuid>,
    pub(super) classroom_id: Uuid,
    pub(super) exam_day_id: Uuid,
    pub(super) starts_at: NaiveTime,
    pub(super) ends_at: NaiveTime,
}

#[derive(Debug, Clone)]
pub(super) struct CandidateRoomSession {
    pub(super) session_id: Option<Uuid>,
    pub(super) room_id: Uuid,
    pub(super) exam_day_id: Uuid,
    pub(super) starts_at: NaiveTime,
    pub(super) ends_at: NaiveTime,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct InvigilatorSessionWindow {
    pub(super) assignment_id: Uuid,
    pub(super) exam_day_id: Uuid,
    pub(super) staff_id: Uuid,
    pub(super) starts_at: NaiveTime,
    pub(super) ends_at: NaiveTime,
}

pub(super) fn invigilator_workload_minutes(windows: &[InvigilatorSessionWindow]) -> i32 {
    windows
        .iter()
        .map(|window| minutes_between_times(window.starts_at, window.ends_at))
        .sum()
}

pub(super) fn minutes_between_times(starts_at: NaiveTime, ends_at: NaiveTime) -> i32 {
    let start_minutes = starts_at.num_seconds_from_midnight() / 60;
    let end_minutes = ends_at.num_seconds_from_midnight() / 60;
    end_minutes.saturating_sub(start_minutes) as i32
}

pub(super) fn has_invigilator_time_conflict(
    candidate_assignment_id: Uuid,
    candidate_windows: &[InvigilatorSessionWindow],
    existing_windows: &[InvigilatorSessionWindow],
) -> bool {
    candidate_windows.iter().any(|candidate| {
        existing_windows.iter().any(|existing| {
            existing.assignment_id != candidate_assignment_id
                && existing.staff_id == candidate.staff_id
                && existing.exam_day_id == candidate.exam_day_id
                && time_ranges_overlap(
                    candidate.starts_at,
                    candidate.ends_at,
                    existing.starts_at,
                    existing.ends_at,
                )
        })
    })
}

pub(super) fn has_same_classroom_conflict(
    candidate: &CandidateSession,
    existing: &[CandidateSession],
) -> bool {
    existing.iter().any(|item| {
        item.exam_day_id == candidate.exam_day_id
            && item.classroom_id == candidate.classroom_id
            && item.session_id != candidate.session_id
            && time_ranges_overlap(
                candidate.starts_at,
                candidate.ends_at,
                item.starts_at,
                item.ends_at,
            )
    })
}

pub(super) fn has_same_room_conflict(
    candidate: &CandidateRoomSession,
    existing: &[CandidateRoomSession],
) -> bool {
    existing.iter().any(|item| {
        item.exam_day_id == candidate.exam_day_id
            && item.room_id == candidate.room_id
            && item.session_id != candidate.session_id
            && time_ranges_overlap(
                candidate.starts_at,
                candidate.ends_at,
                item.starts_at,
                item.ends_at,
            )
    })
}

fn exam_session_conflict_lock_key(namespace: i64, exam_day_id: Uuid, resource_id: Uuid) -> i64 {
    let day_bytes = exam_day_id.as_bytes();
    let resource_bytes = resource_id.as_bytes();
    let mut key_bytes = [0_u8; 8];
    for index in 0..8 {
        key_bytes[index] = day_bytes[index]
            ^ day_bytes[index + 8]
            ^ resource_bytes[index]
            ^ resource_bytes[index + 8];
    }
    i64::from_be_bytes(key_bytes) ^ namespace
}

pub(super) fn exam_session_conflict_lock_keys(
    exam_day_id: Uuid,
    classroom_id: Uuid,
    room_id: Uuid,
) -> [i64; 2] {
    let mut keys = [
        exam_session_conflict_lock_key(
            EXAM_SESSION_CLASSROOM_LOCK_NAMESPACE,
            exam_day_id,
            classroom_id,
        ),
        exam_session_conflict_lock_key(EXAM_SESSION_ROOM_LOCK_NAMESPACE, exam_day_id, room_id),
    ];
    keys.sort_unstable();
    keys
}

pub(super) fn exam_invigilator_staff_lock_keys(exam_day_id: Uuid, staff_ids: &[Uuid]) -> Vec<i64> {
    let mut keys = unique_uuids(staff_ids.to_vec())
        .into_iter()
        .map(|staff_id| {
            exam_session_conflict_lock_key(
                EXAM_INVIGILATOR_STAFF_LOCK_NAMESPACE,
                exam_day_id,
                staff_id,
            )
        })
        .collect::<Vec<_>>();
    keys.sort_unstable();
    keys
}

pub(super) fn validate_session_window(
    starts_at: NaiveTime,
    duration_minutes: i32,
    day_start: NaiveTime,
    day_end: NaiveTime,
    blocked_windows: &[BlockedWindow],
) -> Result<NaiveTime, SessionValidationError> {
    let ends_at = add_minutes(starts_at, duration_minutes)?;
    if !is_exam_session_start_on_slot(starts_at) {
        return Err(SessionValidationError::StartTimeOutsideSlot);
    }
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

pub(super) fn validation_error_to_app_error(error: SessionValidationError) -> AppError {
    match error {
        SessionValidationError::InvalidDuration => {
            AppError::BadRequest("Exam duration must be greater than zero".into())
        }
        SessionValidationError::EndTimeOverflow => {
            AppError::BadRequest("Exam end time is outside the valid day range".into())
        }
        SessionValidationError::StartTimeOutsideSlot => AppError::BadRequest(format!(
            "Exam start time must align to a {EXAM_SESSION_SLOT_MINUTES}-minute slot"
        )),
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

pub(super) fn unique_uuids(ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    ids.into_iter().filter(|id| seen.insert(*id)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn t(value: &str) -> NaiveTime {
        NaiveTime::parse_from_str(value, "%H:%M").expect("test time should parse")
    }

    #[test]
    fn computes_end_time_and_rejects_invalid_durations() {
        assert_eq!(add_minutes(t("08:30"), 90).unwrap(), t("10:00"));
        assert_eq!(
            add_minutes(t("08:30"), 0),
            Err(SessionValidationError::InvalidDuration)
        );
        assert_eq!(
            add_minutes(t("23:30"), 60),
            Err(SessionValidationError::EndTimeOverflow)
        );
    }

    #[test]
    fn half_open_time_ranges_allow_touching_boundaries() {
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
    fn classroom_and_room_conflicts_ignore_the_updated_session() {
        let existing_id = Uuid::from_u128(1);
        let classroom = Uuid::from_u128(2);
        let room = Uuid::from_u128(3);
        let day = Uuid::from_u128(4);
        let classroom_candidate = CandidateSession {
            session_id: Some(existing_id),
            classroom_id: classroom,
            exam_day_id: day,
            starts_at: t("09:00"),
            ends_at: t("10:00"),
        };
        let room_candidate = CandidateRoomSession {
            session_id: Some(existing_id),
            room_id: room,
            exam_day_id: day,
            starts_at: t("09:00"),
            ends_at: t("10:00"),
        };

        assert!(!has_same_classroom_conflict(
            &classroom_candidate,
            std::slice::from_ref(&classroom_candidate)
        ));
        assert!(!has_same_room_conflict(
            &room_candidate,
            std::slice::from_ref(&room_candidate)
        ));
    }

    #[test]
    fn invigilator_conflicts_and_workload_use_live_session_ranges() {
        let staff_id = Uuid::from_u128(7);
        let day = Uuid::from_u128(8);
        let candidate = vec![InvigilatorSessionWindow {
            assignment_id: Uuid::from_u128(1),
            exam_day_id: day,
            staff_id,
            starts_at: t("08:30"),
            ends_at: t("09:30"),
        }];
        let existing = vec![InvigilatorSessionWindow {
            assignment_id: Uuid::from_u128(2),
            exam_day_id: day,
            staff_id,
            starts_at: t("09:00"),
            ends_at: t("10:30"),
        }];

        assert!(has_invigilator_time_conflict(
            Uuid::from_u128(1),
            &candidate,
            &existing
        ));
        assert_eq!(invigilator_workload_minutes(&existing), 90);
    }

    #[test]
    fn advisory_lock_keys_are_sorted_deduplicated_and_scoped() {
        let day = Uuid::parse_str("01020304-0506-0708-090a-0b0c0d0e0f10").unwrap();
        let first = Uuid::parse_str("11121314-1516-1718-191a-1b1c1d1e1f20").unwrap();
        let second = Uuid::parse_str("21222324-2526-2728-292a-2b2c2d2e2f30").unwrap();

        let session_keys = exam_session_conflict_lock_keys(day, first, second);
        assert!(session_keys[0] < session_keys[1]);

        let staff_keys = exam_invigilator_staff_lock_keys(day, &[second, first, first]);
        assert_eq!(staff_keys.len(), 2);
        assert!(staff_keys[0] < staff_keys[1]);
        assert_eq!(
            staff_keys,
            exam_invigilator_staff_lock_keys(day, &[first, second])
        );
    }

    #[test]
    fn session_window_enforces_slots_day_bounds_and_blocked_windows() {
        let blocked = BlockedWindow {
            id: None,
            label: "Lunch".to_string(),
            start_time: t("12:00"),
            end_time: t("13:00"),
        };

        assert!(validate_session_window(t("08:35"), 60, t("08:30"), t("16:00"), &[]).is_ok());
        assert_eq!(
            validate_session_window(t("08:37"), 60, t("08:30"), t("16:00"), &[]),
            Err(SessionValidationError::StartTimeOutsideSlot)
        );
        assert_eq!(
            validate_session_window(t("08:00"), 60, t("08:30"), t("16:00"), &[]),
            Err(SessionValidationError::BeforeDayStart)
        );
        assert_eq!(
            validate_session_window(t("15:30"), 60, t("08:30"), t("16:00"), &[]),
            Err(SessionValidationError::AfterDayEnd)
        );
        assert_eq!(
            validate_session_window(t("11:30"), 90, t("08:30"), t("16:00"), &[blocked]),
            Err(SessionValidationError::BlockedWindow("Lunch".to_string()))
        );
    }

    #[test]
    fn unique_uuids_preserves_first_seen_order_and_deduplicates() {
        let first = Uuid::from_u128(1);
        let second = Uuid::from_u128(2);

        assert_eq!(
            unique_uuids(vec![second, first, second]),
            vec![second, first]
        );
    }
}
