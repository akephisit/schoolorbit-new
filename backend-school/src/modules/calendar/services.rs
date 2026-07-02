use chrono::{Duration, NaiveDate, NaiveTime};
use std::collections::HashSet;
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::calendar::models::{CalendarAudienceType, CalendarEventTargetInput};

pub fn validate_event_date_time(
    start_date: NaiveDate,
    end_date: NaiveDate,
    all_day: bool,
    start_time: Option<NaiveTime>,
    end_time: Option<NaiveTime>,
) -> Result<(), AppError> {
    if end_date < start_date {
        return Err(AppError::BadRequest("วันที่สิ้นสุดต้องไม่ก่อนวันที่เริ่มต้น".to_string()));
    }

    if all_day {
        return Ok(());
    }

    if start_date == end_date {
        match (start_time, end_time) {
            (Some(start), Some(end)) if end > start => Ok(()),
            (Some(_), Some(_)) => Err(AppError::BadRequest("เวลาสิ้นสุดต้องหลังเวลาเริ่มต้น".to_string())),
            _ => Err(AppError::BadRequest(
                "event แบบระบุเวลาต้องมีเวลาเริ่มต้นและสิ้นสุด".to_string(),
            )),
        }
    } else if start_time.is_some() && end_time.is_some() {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "event หลายวันที่ระบุเวลาต้องมีเวลาเริ่มต้นและสิ้นสุด".to_string(),
        ))
    }
}

pub fn reminder_dates(start_date: NaiveDate, offsets: &[i32]) -> Result<Vec<NaiveDate>, AppError> {
    let mut dates = Vec::with_capacity(offsets.len());
    let mut seen = HashSet::new();

    for days_before in offsets {
        if *days_before <= 0 {
            return Err(AppError::BadRequest(
                "จำนวนวันแจ้งเตือนต้องมากกว่า 0".to_string(),
            ));
        }
        if seen.insert(*days_before) {
            let remind_on = start_date
                .checked_sub_signed(Duration::days(i64::from(*days_before)))
                .ok_or_else(|| AppError::BadRequest("วันที่แจ้งเตือนอยู่นอกช่วงที่รองรับ".to_string()))?;
            dates.push(remind_on);
        }
    }

    dates.sort();
    Ok(dates)
}

pub fn validate_targets(targets: &[CalendarEventTargetInput]) -> Result<(), AppError> {
    if targets.is_empty() {
        return Err(AppError::BadRequest("ต้องเลือกผู้เห็นอย่างน้อยหนึ่งกลุ่ม".to_string()));
    }

    let mut seen_targets = HashSet::new();

    for target in targets {
        if target.grade_level_id.is_some() && target.class_room_id.is_some() {
            return Err(AppError::BadRequest(
                "เลือกได้เพียงระดับชั้นหรือห้องเรียนอย่างใดอย่างหนึ่ง".to_string(),
            ));
        }

        match &target.audience_type {
            CalendarAudienceType::All | CalendarAudienceType::Staff => {
                if target.grade_level_id.is_some() || target.class_room_id.is_some() {
                    return Err(AppError::BadRequest(
                        "กลุ่มผู้เห็น all/staff ไม่รองรับการกรองระดับชั้นหรือห้องเรียน".to_string(),
                    ));
                }
            }
            CalendarAudienceType::Student | CalendarAudienceType::Parent => {}
        }

        let target_key = (
            target.audience_type.as_str(),
            target.grade_level_id,
            target.class_room_id,
        );
        if !seen_targets.insert(target_key) {
            return Err(AppError::BadRequest("กลุ่มผู้เห็นซ้ำ".to_string()));
        }
    }

    Ok(())
}

pub fn dedupe_user_ids(ids: Vec<Uuid>) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    let mut deduped = Vec::new();

    for id in ids {
        if seen.insert(id) {
            deduped.push(id);
        }
    }

    deduped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_event_date_time_rejects_end_date_before_start() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 7, 9).unwrap();

        assert!(validate_event_date_time(start, end, true, None, None).is_err());
    }

    #[test]
    fn validate_event_date_time_rejects_same_day_end_time_before_start_time() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let start_time = NaiveTime::from_hms_opt(10, 0, 0).unwrap();
        let end_time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();

        assert!(
            validate_event_date_time(date, date, false, Some(start_time), Some(end_time)).is_err()
        );
    }

    #[test]
    fn validate_event_date_time_rejects_same_day_equal_start_and_end_time() {
        let date = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let time = NaiveTime::from_hms_opt(10, 0, 0).unwrap();

        assert!(validate_event_date_time(date, date, false, Some(time), Some(time)).is_err());
    }

    #[test]
    fn validate_event_date_time_accepts_multi_day_timed_event() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 7, 12).unwrap();
        let start_time = NaiveTime::from_hms_opt(15, 0, 0).unwrap();
        let end_time = NaiveTime::from_hms_opt(9, 0, 0).unwrap();

        assert!(
            validate_event_date_time(start, end, false, Some(start_time), Some(end_time)).is_ok()
        );
    }

    #[test]
    fn reminder_dates_are_day_based_and_sorted() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let result = reminder_dates(start, &[1, 7, 3]).unwrap();

        assert_eq!(
            result,
            vec![
                NaiveDate::from_ymd_opt(2026, 7, 3).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 7).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 9).unwrap()
            ]
        );
    }

    #[test]
    fn reminder_dates_dedupes_duplicate_offsets() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();
        let result = reminder_dates(start, &[3, 1, 3, 1]).unwrap();

        assert_eq!(
            result,
            vec![
                NaiveDate::from_ymd_opt(2026, 7, 7).unwrap(),
                NaiveDate::from_ymd_opt(2026, 7, 9).unwrap()
            ]
        );
    }

    #[test]
    fn reminder_dates_reject_zero_or_negative_offsets() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();

        assert!(reminder_dates(start, &[0]).is_err());
        assert!(reminder_dates(start, &[-1]).is_err());
    }

    #[test]
    fn reminder_dates_rejects_offsets_outside_representable_date_range() {
        let start = NaiveDate::from_ymd_opt(2026, 7, 10).unwrap();

        assert!(reminder_dates(start, &[i32::MAX]).is_err());
    }

    #[test]
    fn validate_targets_rejects_empty_targets() {
        assert!(validate_targets(&[]).is_err());
    }

    #[test]
    fn validate_targets_accepts_student_grade_target() {
        let grade_level_id = Uuid::new_v4();
        let targets = vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::Student,
            grade_level_id: Some(grade_level_id),
            class_room_id: None,
        }];

        assert!(validate_targets(&targets).is_ok());
    }

    #[test]
    fn validate_targets_rejects_student_or_parent_with_grade_and_classroom_filter() {
        let targets = vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::Parent,
            grade_level_id: Some(Uuid::new_v4()),
            class_room_id: Some(Uuid::new_v4()),
        }];

        assert!(validate_targets(&targets).is_err());
    }

    #[test]
    fn validate_targets_rejects_all_with_grade_or_classroom_filter() {
        let grade_targets = vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::All,
            grade_level_id: Some(Uuid::new_v4()),
            class_room_id: None,
        }];
        let classroom_targets = vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::All,
            grade_level_id: None,
            class_room_id: Some(Uuid::new_v4()),
        }];

        assert!(validate_targets(&grade_targets).is_err());
        assert!(validate_targets(&classroom_targets).is_err());
    }

    #[test]
    fn validate_targets_rejects_duplicate_global_targets() {
        let targets = vec![
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Student,
                grade_level_id: None,
                class_room_id: None,
            },
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Student,
                grade_level_id: None,
                class_room_id: None,
            },
        ];

        assert!(validate_targets(&targets).is_err());
    }

    #[test]
    fn validate_targets_rejects_duplicate_grade_targets() {
        let grade_level_id = Uuid::new_v4();
        let targets = vec![
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Parent,
                grade_level_id: Some(grade_level_id),
                class_room_id: None,
            },
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Parent,
                grade_level_id: Some(grade_level_id),
                class_room_id: None,
            },
        ];

        assert!(validate_targets(&targets).is_err());
    }

    #[test]
    fn validate_targets_rejects_duplicate_classroom_targets() {
        let class_room_id = Uuid::new_v4();
        let targets = vec![
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Student,
                grade_level_id: None,
                class_room_id: Some(class_room_id),
            },
            CalendarEventTargetInput {
                audience_type: CalendarAudienceType::Student,
                grade_level_id: None,
                class_room_id: Some(class_room_id),
            },
        ];

        assert!(validate_targets(&targets).is_err());
    }

    #[test]
    fn validate_targets_rejects_staff_with_classroom_filter() {
        let targets = vec![CalendarEventTargetInput {
            audience_type: CalendarAudienceType::Staff,
            grade_level_id: None,
            class_room_id: Some(Uuid::new_v4()),
        }];

        assert!(validate_targets(&targets).is_err());
    }

    #[test]
    fn dedupe_user_ids_preserves_first_seen_order() {
        let a = Uuid::new_v4();
        let b = Uuid::new_v4();

        assert_eq!(dedupe_user_ids(vec![a, b, a]), vec![a, b]);
    }
}
