use school_errors::AppError;

pub fn normalize_name_part(value: &str, limit: usize) -> Result<String, AppError> {
    if value.chars().any(char::is_control) || value.trim().chars().count() > limit {
        return Err(AppError::BadRequest("ข้อมูลยาวเกินกำหนดหรือมีอักขระควบคุม".into()));
    }
    Ok(value.trim().to_owned())
}

pub fn has_complete_name(parts: [&str; 6]) -> bool {
    parts[..3].iter().all(|v| !v.trim().is_empty())
        || parts[3..].iter().all(|v| !v.trim().is_empty())
}

pub fn require_complete_name(parts: [&str; 6]) -> Result<(), AppError> {
    if has_complete_name(parts) {
        Ok(())
    } else {
        Err(AppError::BadRequest(
            "กรุณากรอกคำนำหน้า ชื่อ และนามสกุลให้ครบอย่างน้อยหนึ่งภาษา".into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bilingual_name_validation_requires_one_whole_set() {
        for parts in [
            ["นาย", "สมชาย", "ทดสอบ", "", "", ""],
            ["", "", "", "Mr.", "John", "Smith"],
            ["นาย", "สมชาย", "ทดสอบ", "Dr.", "John", "Smith"],
        ] {
            assert!(require_complete_name(parts).is_ok());
        }
        for parts in [
            ["นาย", "สมชาย", "", "Mr.", "", "Smith"],
            ["", "สมชาย", "ทดสอบ", "", "", ""],
            ["", "", "", "Mr.", "John", " "],
        ] {
            assert!(require_complete_name(parts).is_err());
        }
        assert_eq!(normalize_name_part("  John  ", 100).unwrap(), "John");
        assert!(normalize_name_part("Name\nOther", 100).is_err());
        assert!(normalize_name_part("Name\n", 100).is_err());
        assert!(normalize_name_part(&"😀".repeat(100), 100).is_ok());
        assert!(normalize_name_part(&"ก".repeat(101), 100).is_err());
    }
}
