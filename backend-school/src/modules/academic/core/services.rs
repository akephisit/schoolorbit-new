use super::models::{AcademicTermStatus, VersionStatus};
use crate::error::AppError;
use bigdecimal::BigDecimal;
use chrono::NaiveDate;
use std::str::FromStr;

pub mod bell_schedules;
pub mod catalog;
pub mod context;
pub mod curriculum;
pub mod progressions;
pub mod student_years;
pub mod workspaces;
pub mod years_terms;

pub fn validate_canonical_decimal(value: &str, max_scale: usize) -> Result<BigDecimal, AppError> {
    let (integer, fraction) = match value.split_once('.') {
        Some((integer, fraction)) => (integer, Some(fraction)),
        None => (value, None),
    };
    let canonical_integer = integer == "0"
        || (!integer.is_empty()
            && !integer.starts_with('0')
            && integer.bytes().all(|byte| byte.is_ascii_digit()));
    let canonical_fraction = fraction.is_none_or(|fraction| {
        !fraction.is_empty()
            && fraction.len() <= max_scale
            && fraction.bytes().all(|byte| byte.is_ascii_digit())
    });
    if !canonical_integer || !canonical_fraction {
        return Err(AppError::ValidationError(
            "ค่าทศนิยมต้องเป็นข้อความรูปแบบมาตรฐาน".to_string(),
        ));
    }

    BigDecimal::from_str(value).map_err(|_| AppError::ValidationError("ค่าทศนิยมไม่ถูกต้อง".to_string()))
}

pub fn validate_date_containment(
    parent_start: NaiveDate,
    parent_end: NaiveDate,
    child_start: NaiveDate,
    child_end: NaiveDate,
) -> Result<(), AppError> {
    if parent_start > parent_end || child_start > child_end {
        return Err(AppError::ValidationError(
            "วันเริ่มต้นต้องไม่อยู่หลังวันสิ้นสุด".to_string(),
        ));
    }
    if child_start < parent_start || child_end > parent_end {
        return Err(AppError::ValidationError(
            "ช่วงวันที่ต้องอยู่ภายในปีการศึกษา".to_string(),
        ));
    }
    Ok(())
}

pub fn parse_row_version(row_version: i64) -> Result<i64, AppError> {
    if row_version <= 0 {
        return Err(AppError::ValidationError(
            "rowVersion ต้องมากกว่าศูนย์".to_string(),
        ));
    }
    Ok(row_version)
}

pub fn ensure_draft_version(status: VersionStatus) -> Result<(), AppError> {
    if status == VersionStatus::Draft {
        Ok(())
    } else {
        Err(AppError::Conflict(
            "เวอร์ชันที่เผยแพร่หรือเก็บถาวรแล้วแก้ไขไม่ได้".to_string(),
        ))
    }
}

pub fn ensure_planning_delete(
    status: AcademicTermStatus,
    dependency_count: i64,
) -> Result<(), AppError> {
    if status != AcademicTermStatus::Planning {
        return Err(AppError::Conflict(
            "ลบได้เฉพาะภาคเรียนสถานะ planning".to_string(),
        ));
    }
    if dependency_count != 0 {
        return Err(AppError::Conflict(
            "ภาคเรียนมีข้อมูลอ้างอิงและไม่สามารถลบได้".to_string(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decimal_and_optimistic_revision_validators_fail_closed() {
        assert_eq!(
            validate_canonical_decimal("1.50", 2).unwrap().to_string(),
            "1.50"
        );
        assert!(validate_canonical_decimal("01.50", 2).is_err());
        assert_eq!(parse_row_version(1).unwrap(), 1);
        assert!(parse_row_version(0).is_err());
    }
}
