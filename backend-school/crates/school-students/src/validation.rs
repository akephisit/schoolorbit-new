use chrono::NaiveDate;
use school_auth::session_policy::validate_new_password;
use school_errors::AppError;

use crate::models::{CreateParentRequest, CreateStudentRequest, UpdateStudentRequest};

pub(crate) fn normalize_create(request: &mut CreateStudentRequest) -> Result<(), AppError> {
    validate_new_password(&request.password)?;
    required(&mut request.student_id, 50, "รหัสนักเรียน")?;
    required(&mut request.first_name, 100, "ชื่อ")?;
    required(&mut request.last_name, 100, "นามสกุล")?;
    optional(&mut request.username, 100, "ชื่อผู้ใช้")?;
    optional(&mut request.title, 50, "คำนำหน้า")?;
    optional(&mut request.phone, 20, "เบอร์โทรศัพท์")?;
    optional(&mut request.address, 2_000, "ที่อยู่")?;
    email(&mut request.email)?;
    national_id(&mut request.national_id)?;
    gender(&mut request.gender)?;
    parse_date(request.date_of_birth.as_deref())?;
    parse_date(request.enrollment_date.as_deref())?;
    if let Some(parents) = &mut request.parents {
        if parents.len() > 10 {
            return Err(invalid("เพิ่มผู้ปกครองได้ไม่เกิน 10 คนต่อครั้ง"));
        }
        for parent in parents {
            normalize_parent(parent)?;
        }
    }
    Ok(())
}

pub(crate) fn normalize_update(request: &mut UpdateStudentRequest) -> Result<(), AppError> {
    if let Some(value) = &mut request.first_name {
        required(value, 100, "ชื่อ")?;
    }
    if let Some(value) = &mut request.last_name {
        required(value, 100, "นามสกุล")?;
    }
    if let Some(value) = &mut request.title {
        optional(value, 50, "คำนำหน้า")?;
    }
    if let Some(value) = &mut request.phone {
        optional(value, 20, "เบอร์โทรศัพท์")?;
    }
    if let Some(value) = &mut request.address {
        optional(value, 2_000, "ที่อยู่")?;
    }
    if let Some(value) = &mut request.email {
        email(value)?;
    }
    if let Some(value) = &mut request.gender {
        gender(value)?;
    }
    for value in [&request.date_of_birth, &request.enrollment_date]
        .into_iter()
        .flatten()
    {
        parse_date(value.as_deref())?;
    }
    Ok(())
}

pub(crate) fn normalize_parent(request: &mut CreateParentRequest) -> Result<(), AppError> {
    required(&mut request.first_name, 100, "ชื่อผู้ปกครอง")?;
    required(&mut request.last_name, 100, "นามสกุลผู้ปกครอง")?;
    required(&mut request.relationship, 50, "ความสัมพันธ์")?;
    required(&mut request.phone, 10, "เบอร์โทรผู้ปกครอง")?;
    if !(9..=10).contains(&request.phone.len())
        || !request.phone.bytes().all(|c| c.is_ascii_digit())
    {
        return Err(invalid("เบอร์โทรผู้ปกครองต้องเป็นตัวเลข 9–10 หลัก"));
    }
    optional(&mut request.title, 50, "คำนำหน้า")?;
    email(&mut request.email)?;
    national_id(&mut request.national_id)
}

pub(crate) fn parse_date(value: Option<&str>) -> Result<Option<NaiveDate>, AppError> {
    let Some(value) = value.filter(|value| !value.is_empty()) else {
        return Ok(None);
    };
    let date = NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| invalid("รูปแบบวันที่ไม่ถูกต้อง (ต้องเป็น YYYY-MM-DD)"))?;
    if date.format("%Y-%m-%d").to_string() != value {
        return Err(invalid("รูปแบบวันที่ไม่ถูกต้อง (ต้องเป็น YYYY-MM-DD)"));
    }
    Ok(Some(date))
}

fn required(value: &mut String, maximum: usize, label: &str) -> Result<(), AppError> {
    *value = value.trim().to_string();
    if value.is_empty() || value.chars().count() > maximum {
        return Err(invalid(&format!("{label}ต้องมีความยาว 1–{maximum} ตัวอักษร")));
    }
    Ok(())
}

fn optional(value: &mut Option<String>, maximum: usize, label: &str) -> Result<(), AppError> {
    if let Some(text) = value {
        *text = text.trim().to_string();
        if text.chars().count() > maximum {
            return Err(invalid(&format!("{label}ต้องไม่เกิน {maximum} ตัวอักษร")));
        }
        if text.is_empty() {
            *value = None;
        }
    }
    Ok(())
}

fn email(value: &mut Option<String>) -> Result<(), AppError> {
    optional(value, 255, "อีเมล")?;
    if let Some(value) = value {
        let valid = value.split_once('@').is_some_and(|(local, domain)| {
            !local.is_empty()
                && domain.contains('.')
                && !domain.starts_with('.')
                && !domain.ends_with('.')
                && !domain.contains('@')
                && !value.chars().any(char::is_whitespace)
        });
        if !valid {
            return Err(invalid("รูปแบบอีเมลไม่ถูกต้อง"));
        }
    }
    Ok(())
}

fn national_id(value: &mut Option<String>) -> Result<(), AppError> {
    optional(value, 13, "เลขประจำตัวประชาชน")?;
    if value
        .as_ref()
        .is_some_and(|value| value.len() != 13 || !value.bytes().all(|c| c.is_ascii_digit()))
    {
        return Err(invalid("เลขประจำตัวประชาชนต้องเป็นตัวเลข 13 หลัก"));
    }
    Ok(())
}

fn gender(value: &mut Option<String>) -> Result<(), AppError> {
    optional(value, 20, "เพศ")?;
    if value
        .as_ref()
        .is_some_and(|value| !matches!(value.as_str(), "male" | "female" | "other"))
    {
        return Err(invalid("ข้อมูลเพศไม่ถูกต้อง"));
    }
    Ok(())
}

fn invalid(message: &str) -> AppError {
    AppError::BadRequest(message.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn nullable_profile_updates_distinguish_omitted_null_and_values() {
        let empty: UpdateStudentRequest = serde_json::from_value(json!({})).unwrap();
        assert!(empty.email.is_none());
        assert!(empty.date_of_birth.is_none());
        let mut clear: UpdateStudentRequest = serde_json::from_value(json!({
            "email": null, "title": null, "phone": "  ", "address": null,
            "date_of_birth": null, "gender": null, "enrollment_date": null
        }))
        .unwrap();
        normalize_update(&mut clear).unwrap();
        for field in [
            clear.email,
            clear.title,
            clear.phone,
            clear.address,
            clear.date_of_birth,
            clear.gender,
            clear.enrollment_date,
        ] {
            assert_eq!(field, Some(None));
        }
        let mut values: UpdateStudentRequest = serde_json::from_value(json!({
            "first_name": "  Test  ", "email": " test@example.invalid ",
            "date_of_birth": "2012-02-29", "enrollment_date": "2026-05-01"
        }))
        .unwrap();
        normalize_update(&mut values).unwrap();
        assert_eq!(values.first_name.as_deref(), Some("Test"));
        assert_eq!(values.email, Some(Some("test@example.invalid".into())));
    }

    #[test]
    fn profile_validation_rejects_invalid_names_dates_email_gender_and_immutable_fields() {
        for value in [
            json!({"first_name":"  "}),
            json!({"last_name":"x".repeat(101)}),
            json!({"date_of_birth":"2011-02-29"}),
            json!({"enrollment_date":"2026-5-1"}),
            json!({"email":"missing-at"}),
            json!({"gender":"invalid"}),
        ] {
            let mut request: UpdateStudentRequest = serde_json::from_value(value).unwrap();
            assert!(normalize_update(&mut request).is_err());
        }
        for value in [
            json!({"student_id":"changed"}),
            json!({"student_number":3}),
            json!({"registration_status":"not-a-status"}),
            json!({"registration_status_updated_by":"caller-supplied"}),
        ] {
            assert!(serde_json::from_value::<UpdateStudentRequest>(value).is_err());
        }
    }

    #[test]
    fn create_validates_nontruncating_passwords_before_storage() {
        for password in ["short".to_string(), "ก".repeat(24), "a".repeat(72)] {
            let mut request: CreateStudentRequest = serde_json::from_value(json!({
                "first_name":"Test", "last_name":"Student", "student_id":"S001", "password":password
            }))
            .unwrap();
            assert!(normalize_create(&mut request).is_err());
        }
    }
}
