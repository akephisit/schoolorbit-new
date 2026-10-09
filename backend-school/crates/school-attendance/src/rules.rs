use crate::models::*;
use chrono::{Datelike, NaiveDate};
use school_errors::AppError;
use std::collections::HashSet;
use uuid::Uuid;
pub const FACE_MODEL: &str = "face-api-1.7.15-recognition-128";
pub fn invalid(message: &str) -> AppError {
    AppError::ValidationError(message.to_string())
}
pub fn validate_configuration(c: &AttendanceConfiguration) -> Result<(), AppError> {
    if c.weekdays.iter().any(|d| !(1..=7).contains(d))
        || c.weekdays.iter().collect::<HashSet<_>>().len() != c.weekdays.len()
    {
        return Err(invalid("วันในสัปดาห์ไม่ถูกต้อง"));
    }
    if c.digest_times.len() > 8
        || c.digest_times.iter().collect::<HashSet<_>>().len() != c.digest_times.len()
        || !(1..=365).contains(&c.evidence_days)
        || !c.face_distance.is_finite()
        || !(0.2..=0.6).contains(&c.face_distance)
        || !c.face_margin.is_finite()
        || !(0.05..=0.3).contains(&c.face_margin)
    {
        return Err(invalid("เกณฑ์หรืออายุการเก็บภาพไม่ถูกต้อง"));
    }
    Ok(())
}
pub fn counted(c: &AttendanceConfiguration, date: NaiveDate, day: Option<bool>) -> bool {
    day.unwrap_or_else(|| c.weekdays.contains(&date.weekday().number_from_monday()))
}
pub fn validate_inputs(inputs: &[AttendanceResultInput], roster: &[Uuid]) -> Result<(), AppError> {
    if inputs.is_empty()
        || inputs.len() > 2000
        || !inputs
            .iter()
            .any(|s| s.result != AttendanceResult::Unchecked)
    {
        return Err(invalid("เลือกผลอย่างน้อยหนึ่งคน หรือกดขาดทั้งห้องก่อนบันทึก"));
    }
    let mut seen = HashSet::new();
    for s in inputs {
        if !seen.insert(s.student_id) || !roster.contains(&s.student_id) || s.note.len() > 1000 {
            return Err(invalid("นักเรียนซ้ำ ไม่อยู่ในรอบ หรือหมายเหตุยาวเกินกำหนด"));
        }
    }
    Ok(())
}
pub fn validate_descriptor(d: &FaceDescriptor) -> Result<(), AppError> {
    if d.values.len() != 128 || d.values.iter().any(|v| !v.is_finite() || v.abs() > 10.0) {
        Err(invalid("ข้อมูลใบหน้าไม่ถูกต้อง"))
    } else {
        Ok(())
    }
}
pub fn face_distance(a: &FaceDescriptor, b: &FaceDescriptor) -> f32 {
    a.values
        .iter()
        .zip(&b.values)
        .map(|(a, b)| (a - b).powi(2))
        .sum::<f32>()
        .sqrt()
}
pub fn distinct_groups(groups: &[Vec<Uuid>]) -> Result<(), AppError> {
    let mut seen = HashSet::new();
    for g in groups {
        if g.is_empty() {
            return Err(invalid("กลุ่มเช็คชื่อต้องมีนักเรียน"));
        }
        for id in g {
            if !seen.insert(*id) {
                return Err(invalid("นักเรียนอยู่หลายกลุ่มในรอบเดียวกัน กรุณาจัดกลุ่มใหม่"));
            }
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn excluded_dates_remain_separate_from_capture() {
        let c = AttendanceConfiguration::default();
        let sat = NaiveDate::from_ymd_opt(2026, 10, 10).unwrap();
        assert!(!counted(&c, sat, None));
        assert!(counted(&c, sat, Some(true)));
    }
    #[test]
    fn empty_save_cannot_infer_absence_and_duplicate_students_fail() {
        let id = Uuid::new_v4();
        assert!(validate_inputs(&[], &[id]).is_err());
        let s = AttendanceResultInput {
            student_id: id,
            result: AttendanceResult::Absent,
            note: String::new(),
        };
        assert!(validate_inputs(std::slice::from_ref(&s), &[id]).is_ok());
        assert!(validate_inputs(&[s.clone(), s], &[id]).is_err());
    }
    #[test]
    fn special_partition_is_per_round() {
        let id = Uuid::new_v4();
        assert!(distinct_groups(&[vec![id], vec![id]]).is_err());
        assert!(distinct_groups(&[vec![id]]).is_ok());
    }
    #[test]
    fn face_vectors_reject_nonfinite_or_wrong_model_dimensions() {
        assert!(validate_descriptor(&FaceDescriptor {
            values: vec![f32::NAN; 128]
        })
        .is_err());
        assert!(validate_descriptor(&FaceDescriptor {
            values: vec![0.0; 127]
        })
        .is_err());
        let d = FaceDescriptor {
            values: vec![0.0; 128],
        };
        assert_eq!(face_distance(&d, &d), 0.0);
    }
}
