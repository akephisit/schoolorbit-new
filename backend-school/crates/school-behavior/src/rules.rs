use crate::models::*;
use chrono::{Datelike, NaiveDate};
use school_errors::AppError;
use serde::Serialize;
use sha2::{Digest, Sha256};
use uuid::Uuid;

pub fn invalid(message: impl Into<String>) -> AppError {
    AppError::ValidationError(message.into())
}
pub fn validate_reason(reason: &str) -> Result<(), AppError> {
    if reason.trim().is_empty() || reason.chars().count() > 1000 {
        return Err(invalid("กรุณาระบุเหตุผลไม่เกิน 1,000 ตัวอักษร"));
    }
    Ok(())
}
pub fn validate_status(status: &str) -> Result<(), AppError> {
    if !matches!(
        status,
        "present"
            | "absent"
            | "late"
            | "truant"
            | "sick_leave"
            | "personal_leave"
            | "activity"
            | "unchecked"
    ) {
        return Err(invalid("สถานะเช็กชื่อไม่ถูกต้อง"));
    }
    Ok(())
}
pub fn validate_score(action: BehaviorScoreAction, points: i32) -> Result<(), AppError> {
    if !(0..=100000).contains(&points) || (action == BehaviorScoreAction::None) != (points == 0) {
        return Err(invalid("จำนวนคะแนนไม่สอดคล้องกับการคิดคะแนน"));
    }
    Ok(())
}
pub fn normalize_ids(mut ids: Vec<Uuid>) -> Result<Vec<Uuid>, AppError> {
    ids.sort_unstable();
    ids.dedup();
    if ids.is_empty() || ids.len() > 10000 {
        return Err(invalid("เลือกนักเรียนหรือรายการตั้งแต่ 1 ถึง 10,000 รายการ"));
    }
    Ok(ids)
}
pub fn validate_range(start: NaiveDate, end: NaiveDate) -> Result<(), AppError> {
    if start > end || (end - start).num_days() > 3660 {
        return Err(invalid("ช่วงวันที่ไม่ถูกต้องหรือเกิน 10 ปี"));
    }
    Ok(())
}
pub fn pagination(offset: Option<i64>, limit: Option<i64>) -> Result<(i64, i64), AppError> {
    let offset = offset.unwrap_or(0);
    let limit = limit.unwrap_or(50);
    if !(0..=1000000).contains(&offset) || !(1..=10000).contains(&limit) {
        return Err(invalid("จำนวนหรือหน้ารายการไม่ถูกต้อง"));
    }
    Ok((offset, limit))
}
pub fn digest<T: Serialize>(value: &T) -> Result<String, AppError> {
    let bytes = serde_json::to_vec(value)
        .map_err(|_| AppError::InternalServerError("ไม่สามารถตรวจสอบคำขอได้".into()))?;
    Ok(hex::encode(Sha256::digest(bytes)))
}
pub fn normalize_scope(mut scope: BehaviorCleanupScope) -> Result<BehaviorCleanupScope, AppError> {
    scope.sides.sort_unstable();
    scope.sides.dedup();
    scope.sources.sort_unstable();
    scope.sources.dedup();
    if scope.sides.is_empty() || scope.sources.is_empty() {
        return Err(invalid("เลือกฝั่งคะแนนและที่มา"));
    }
    if scope.start.is_some() != scope.end.is_some() {
        return Err(invalid("ระบุวันเริ่มและวันสิ้นสุดพร้อมกัน"));
    }
    if let (Some(start), Some(end)) = (scope.start, scope.end) {
        validate_range(start, end)?;
    }
    Ok(scope)
}
pub fn action_side(action: BehaviorScoreAction) -> Option<BehaviorSide> {
    match action {
        BehaviorScoreAction::None => None,
        BehaviorScoreAction::Merit => Some(BehaviorSide::Merit),
        BehaviorScoreAction::Deduction => Some(BehaviorSide::Deduction),
    }
}
pub fn totals(merit: i64, deduction: i64) -> BehaviorScoreTotals {
    BehaviorScoreTotals {
        merit,
        deduction,
        net: merit - deduction,
    }
}
pub fn reset_boundary(
    schedule: BehaviorResetSchedule,
    starts: NaiveDate,
    today: NaiveDate,
    readiness: &BehaviorResetReadiness,
) -> Option<NaiveDate> {
    let end = match schedule {
        BehaviorResetSchedule::Manual => None,
        BehaviorResetSchedule::Monthly => {
            let next = if starts.month() == 12 {
                NaiveDate::from_ymd_opt(starts.year() + 1, 1, 1)
            } else {
                NaiveDate::from_ymd_opt(starts.year(), starts.month() + 1, 1)
            };
            next.and_then(|d| d.pred_opt())
        }
        BehaviorResetSchedule::Term => readiness
            .term_ends
            .iter()
            .copied()
            .filter(|d| *d >= starts)
            .min(),
        BehaviorResetSchedule::Year => readiness
            .year_ends
            .iter()
            .copied()
            .filter(|d| *d >= starts)
            .min(),
    }?;
    (end < today && end <= readiness.finalized_through && end <= readiness.delivered_through)
        .then_some(end)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dedup_and_score_validation() {
        let id = Uuid::new_v4();
        assert_eq!(normalize_ids(vec![id, id]).unwrap(), vec![id]);
        assert!(normalize_ids(vec![]).is_err());
        assert!(validate_score(BehaviorScoreAction::None, 2).is_err());
        assert!(validate_status("leave").is_err());
    }
    #[test]
    fn monthly_reset_waits_for_both_watermarks_and_catches_up() {
        let start = NaiveDate::from_ymd_opt(2026, 1, 12).unwrap();
        let today = NaiveDate::from_ymd_opt(2026, 3, 2).unwrap();
        let end = NaiveDate::from_ymd_opt(2026, 1, 31).unwrap();
        let mut ready = BehaviorResetReadiness {
            finalized_through: end,
            delivered_through: end.pred_opt().unwrap(),
            term_ends: vec![],
            year_ends: vec![],
        };
        assert_eq!(
            reset_boundary(BehaviorResetSchedule::Monthly, start, today, &ready),
            None
        );
        ready.delivered_through = end;
        assert_eq!(
            reset_boundary(BehaviorResetSchedule::Monthly, start, today, &ready),
            Some(end)
        );
    }
}
