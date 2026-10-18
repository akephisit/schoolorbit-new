use super::models::*;
use chrono::{Datelike, NaiveDate, Timelike};
use school_authorization::ActorContext;
use school_errors::AppError;
use school_permissions::registry::codes;
use school_workflow::process_models::ProcessStepKind;
use std::collections::BTreeSet;

pub fn invalid(message: &str) -> AppError {
    AppError::ValidationError(message.into())
}
pub fn denied() -> AppError {
    AppError::Forbidden("ไม่มีสิทธิ์ดำเนินการคำขอลานี้".into())
}
pub fn stale() -> AppError {
    AppError::Conflict("ข้อมูลเปลี่ยนแล้ว กรุณาตรวจรายการล่าสุด".into())
}
pub fn can_read(actor: &ActorContext, owner: uuid::Uuid, assigned: bool) -> bool {
    actor.has_permission(codes::STAFF_LEAVE_READ_SCHOOL)
        || (owner == actor.user_id && actor.has_permission(codes::STAFF_LEAVE_READ_OWN))
        || (assigned && actor.has_permission(codes::STAFF_LEAVE_READ_ASSIGNED))
}
pub fn validate_policy(input: &SaveLeavePolicyRequest) -> Result<(), AppError> {
    if input.source_reference.trim().is_empty()
        || input.source_reference.chars().count() > 2000
        || input.personnel_types.is_empty()
        || input.personnel_types.len() > 5
        || input.personnel_types.iter().collect::<BTreeSet<_>>().len()
            != input.personnel_types.len()
        || !(1..=12).contains(&input.quota_year_start_month)
        || input.max_per_request.is_some_and(|v| v <= 0)
        || input.annual_limit.is_some_and(|v| v <= 0)
        || input.evidence_from_amount.is_some_and(|n| n <= 0)
        || input
            .minimum_notice_days
            .is_some_and(|n| !(0..=365).contains(&n))
        || input.excluded_dates.len() > 1000
        || input.working_weekdays.len() > 7
        || input.working_weekdays.iter().any(|v| !(1..=7).contains(v))
        || (input.counting_mode == LeaveCountingMode::WorkingDays
            && input.working_weekdays.is_empty())
    {
        return Err(invalid("เกณฑ์สิทธิ์ ประเภทบุคลากร แหล่งอ้างอิง หรือวันทำงานไม่ถูกต้อง"));
    }
    school_workflow::process::validate_steps(&input.steps)?;
    if input.steps.iter().any(|s| {
        s.recipients.is_empty()
            || s.requires_delegated_authority
            || s.requires_evidence
            || !matches!(s.kind, ProcessStepKind::Review | ProcessStepKind::Approve)
    }) || input
        .steps
        .last()
        .is_none_or(|s| s.kind != ProcessStepKind::Approve)
    {
        return Err(invalid(
            "เส้นทางต้องจบด้วยการอนุมัติ และยังไม่รองรับการมอบอำนาจหรือหลักฐานรายขั้นตอน",
        ));
    }
    Ok(())
}
pub fn quota_year(date: NaiveDate, start_month: u32) -> Result<NaiveDate, AppError> {
    NaiveDate::from_ymd_opt(
        date.year() - i32::from(date.month() < start_month),
        start_month,
        1,
    )
    .ok_or_else(|| invalid("ปีสิทธิ์ไม่ถูกต้อง"))
}
pub fn amount(period: &LeavePeriodInput, policy: &LeavePolicyVersion) -> Result<i64, AppError> {
    if period.reason.trim().is_empty()
        || period.reason.chars().count() > 4000
        || period.evidence_file_ids.len() > 20
        || period.end_date < period.start_date
        || (period.end_date - period.start_date).num_days() > 366
    {
        return Err(invalid("กรุณาระบุเหตุผลและช่วงวันที่ไม่เกิน 367 วัน"));
    }
    if quota_year(period.start_date, policy.quota_year_start_month)?
        != quota_year(period.end_date, policy.quota_year_start_month)?
    {
        return Err(invalid("คำขอข้ามปีสิทธิ์ กรุณาแยกคำขอเป็นแต่ละปี"));
    }
    let is_day = |date: NaiveDate| {
        policy.counting_mode == LeaveCountingMode::CalendarDays
            || (policy
                .working_weekdays
                .contains(&date.weekday().number_from_monday())
                && !policy.excluded_dates.contains(&date))
    };
    let amount = match policy.unit {
        LeaveUnit::Day => {
            if period.day_portion != LeaveDayPortion::FullDay
                || period.start_time.is_some()
                || period.end_time.is_some()
            {
                return Err(invalid("หน่วยวันต้องเป็นเต็มวันโดยไม่ระบุเวลา"));
            }
            (0..=(period.end_date - period.start_date).num_days())
                .filter(|n| is_day(period.start_date + chrono::Duration::days(*n)))
                .count() as i64
        }
        LeaveUnit::HalfDay => {
            if period.start_time.is_some() || period.end_time.is_some() {
                return Err(invalid("หน่วยครึ่งวันไม่ใช้เวลา"));
            }
            if period.day_portion == LeaveDayPortion::FullDay {
                2 * (0..=(period.end_date - period.start_date).num_days())
                    .filter(|n| is_day(period.start_date + chrono::Duration::days(*n)))
                    .count() as i64
            } else {
                if period.start_date != period.end_date {
                    return Err(invalid("ครึ่งวันเช้าหรือบ่ายต้องเลือกวันเดียว"));
                }
                i64::from(is_day(period.start_date))
            }
        }
        LeaveUnit::Minute => {
            let (Some(start), Some(end)) = (period.start_time, period.end_time) else {
                return Err(invalid("กรุณาระบุเวลาเริ่มและสิ้นสุด"));
            };
            if period.start_date != period.end_date
                || period.day_portion != LeaveDayPortion::FullDay
                || end <= start
                || start.nanosecond() != 0
                || end.nanosecond() != 0
                || start.second() != 0
                || end.second() != 0
                || !is_day(period.start_date)
            {
                return Err(invalid("ออกนอกสถานศึกษาต้องอยู่ในวันเดียวและสิ้นสุดหลังเวลาเริ่ม"));
            }
            (end - start).num_minutes()
        }
    };
    if amount <= 0 || policy.max_per_request.is_some_and(|cap| amount > cap) {
        return Err(invalid("จำนวนเกินเกณฑ์ต่อครั้งหรือไม่มีวันทำงานในช่วงที่เลือก"));
    }
    Ok(amount)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn policy(unit: LeaveUnit) -> LeavePolicyVersion {
        LeavePolicyVersion {
            id: uuid::Uuid::new_v4(),
            type_id: uuid::Uuid::new_v4(),
            version: 1,
            type_revision: 1,
            effective_from: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            source_reference: "school example".into(),
            enabled: true,
            personnel_types: vec![],
            unit,
            counting_mode: LeaveCountingMode::WorkingDays,
            working_weekdays: vec![1, 2, 3, 4, 5],
            excluded_dates: vec![NaiveDate::from_ymd_opt(2026, 10, 20).unwrap()],
            quota_year_start_month: 10,
            max_per_request: None,
            annual_limit: None,
            evidence_required: false,
            evidence_from_amount: None,
            minimum_notice_days: None,
            steps: vec![],
            template_id: uuid::Uuid::new_v4(),
            created_at: chrono::Utc::now(),
        }
    }
    fn period() -> LeavePeriodInput {
        LeavePeriodInput {
            type_id: uuid::Uuid::new_v4(),
            start_date: NaiveDate::from_ymd_opt(2026, 10, 19).unwrap(),
            end_date: NaiveDate::from_ymd_opt(2026, 10, 21).unwrap(),
            start_time: None,
            end_time: None,
            day_portion: LeaveDayPortion::FullDay,
            reason: "test".into(),
            evidence_file_ids: vec![],
        }
    }
    #[test]
    fn leave_counts_configured_holidays_and_calendar_days() {
        let mut p = policy(LeaveUnit::Day);
        let r = period();
        assert_eq!(amount(&r, &p).unwrap(), 2);
        p.counting_mode = LeaveCountingMode::CalendarDays;
        assert_eq!(amount(&r, &p).unwrap(), 3);
    }
    #[test]
    fn leave_minutes_are_server_derived_and_half_day_is_precise() {
        let mut r = period();
        r.end_date = r.start_date;
        r.start_time = chrono::NaiveTime::from_hms_opt(9, 0, 0);
        r.end_time = chrono::NaiveTime::from_hms_opt(11, 0, 0);
        assert_eq!(amount(&r, &policy(LeaveUnit::Minute)).unwrap(), 120);
        r.start_time = None;
        r.end_time = None;
        assert_eq!(amount(&r, &policy(LeaveUnit::HalfDay)).unwrap(), 2);
        r.day_portion = LeaveDayPortion::Morning;
        assert_eq!(amount(&r, &policy(LeaveUnit::HalfDay)).unwrap(), 1);
    }
    #[test]
    fn leave_scope_union_keeps_own_and_assigned() {
        let id = uuid::Uuid::new_v4();
        let a = ActorContext {
            user_id: id,
            permissions: vec![
                codes::STAFF_LEAVE_READ_OWN.into(),
                codes::STAFF_LEAVE_READ_ASSIGNED.into(),
            ],
        };
        assert!(can_read(&a, id, false));
        assert!(can_read(&a, uuid::Uuid::new_v4(), true));
        assert!(!can_read(&a, uuid::Uuid::new_v4(), false));
    }
    #[test]
    fn leave_year_crossing_requires_split() {
        let mut r = period();
        r.start_date = NaiveDate::from_ymd_opt(2026, 9, 30).unwrap();
        r.end_date = NaiveDate::from_ymd_opt(2026, 10, 1).unwrap();
        assert!(amount(&r, &policy(LeaveUnit::Day)).is_err());
    }
}
