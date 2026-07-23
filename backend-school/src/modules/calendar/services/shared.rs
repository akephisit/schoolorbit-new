use std::collections::HashSet;

use chrono::{Datelike, Duration, Months, NaiveDate, NaiveTime, Utc};
use uuid::Uuid;

use crate::error::AppError;
use crate::modules::calendar::models::{
    CalendarAudienceType, CalendarEventQuery, CalendarEventTargetInput,
};

pub(super) const EVENT_NOT_FOUND_MESSAGE: &str = "ไม่พบกำหนดการ";

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

pub(super) fn reminder_schedule(
    start_date: NaiveDate,
    offsets: &[i32],
) -> Result<Vec<(i32, NaiveDate)>, AppError> {
    let mut pairs = Vec::with_capacity(offsets.len());
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
            pairs.push((*days_before, remind_on));
        }
    }

    pairs.sort_by_key(|(_, remind_on)| *remind_on);
    Ok(pairs)
}

#[cfg(test)]
pub fn reminder_dates(start_date: NaiveDate, offsets: &[i32]) -> Result<Vec<NaiveDate>, AppError> {
    Ok(reminder_schedule(start_date, offsets)?
        .into_iter()
        .map(|(_, remind_on)| remind_on)
        .collect())
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

pub(super) fn dedupe_uuid_ids(ids: &[Uuid]) -> Vec<Uuid> {
    let mut seen = HashSet::new();
    ids.iter().copied().filter(|id| seen.insert(*id)).collect()
}

pub(super) fn normalized_event_range(
    query: &CalendarEventQuery,
    today: NaiveDate,
) -> Result<(NaiveDate, NaiveDate), AppError> {
    match (query.from, query.to) {
        (Some(from), Some(to)) if from > to => {
            Err(AppError::BadRequest("วันที่เริ่มต้นต้องไม่หลังวันที่สิ้นสุด".to_string()))
        }
        (Some(from), Some(to)) => Ok((from, to)),
        _ => Ok(current_month_range(today)),
    }
}

pub(super) fn tenant_today() -> NaiveDate {
    (Utc::now() + Duration::hours(7)).date_naive()
}

fn current_month_range(today: NaiveDate) -> (NaiveDate, NaiveDate) {
    let first_day = NaiveDate::from_ymd_opt(today.year(), today.month(), 1).unwrap_or(today);
    let last_day = first_day
        .checked_add_months(Months::new(1))
        .and_then(|next_month| next_month.checked_sub_signed(Duration::days(1)))
        .unwrap_or(first_day);

    (first_day, last_day)
}
