//! Register presentation settings; accepted labels and annotations are concrete snapshots.
use crate::{process::invalid, process_models::*};
use school_errors::AppError;
use uuid::Uuid;

fn tokens<'a>(value: &'a str, allowed: &[&str]) -> Result<Vec<&'a str>, AppError> {
    let mut rest = value;
    let mut found = Vec::new();
    while let Some(start) = rest.find(['{', '}']) {
        if rest.as_bytes()[start] != b'{' {
            return Err(invalid("รูปแบบตัวแปรไม่ถูกต้อง"));
        }
        let tail = &rest[start + 1..];
        let end = tail.find('}').ok_or_else(|| invalid("รูปแบบตัวแปรไม่ถูกต้อง"))?;
        let token = &tail[..end];
        if !allowed.contains(&token) {
            return Err(invalid("พบตัวแปรที่ไม่รองรับ"));
        }
        found.push(token);
        rest = &tail[end + 1..];
    }
    Ok(found)
}

pub fn validate_numbering(value: &CorrespondenceNumberingSettings) -> Result<(), AppError> {
    if value.format.chars().count() > 100
        || value.format.chars().any(char::is_control)
        || !(1..=8).contains(&value.minimum_digits)
    {
        return Err(invalid("รูปแบบเลขทะเบียนไม่ถูกต้อง"));
    }
    let found = tokens(&value.format, &["number", "year"])?;
    if found.iter().filter(|v| **v == "number").count() != 1
        || found.iter().filter(|v| **v == "year").count() > 1
    {
        return Err(invalid("รูปแบบเลขทะเบียนต้องมี {number} หนึ่งตำแหน่ง"));
    }
    Ok(())
}

pub fn register_label(
    value: &CorrespondenceNumberingSettings,
    number: i64,
    year: i32,
) -> Result<String, AppError> {
    validate_numbering(value)?;
    Ok(value
        .format
        .replace(
            "{number}",
            &format!(
                "{number:0width$}",
                width = usize::from(value.minimum_digits)
            ),
        )
        .replace("{year}", &year.to_string()))
}

pub fn validate_templates(values: &[CorrespondenceTextStampTemplate]) -> Result<(), AppError> {
    if values.len() > 30 {
        return Err(invalid("แม่แบบข้อความเกิน 30 รายการ"));
    }
    let mut ids = std::collections::BTreeSet::new();
    for value in values {
        if value.id.get_version_num() != 7
            || !ids.insert(value.id)
            || value.name.trim().is_empty()
            || value.name.chars().count() > 100
            || value.name.chars().any(char::is_control)
            || value.body.trim().is_empty()
            || value.body.chars().count() > 5000
            || value
                .body
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(invalid("ชื่อหรือข้อความแม่แบบไม่ถูกต้อง"));
        }
        tokens(
            &value.body,
            &[
                "schoolName",
                "registerNumber",
                "date",
                "time",
                "registeredDate",
                "registeredTime",
                "signerName",
                "signerTitle",
                "instruction",
            ],
        )?;
    }
    Ok(())
}

pub fn default_stamp_templates() -> Vec<CorrespondenceTextStampTemplate> {
    [
        (
            1,
            "ตรารับหนังสือ",
            "{schoolName}\nเลขรับ {registerNumber}\nวันที่ {registeredDate} เวลา {registeredTime}",
            true,
        ),
        (
            2,
            "ข้อความสั่งการ",
            "{instruction}\n({signerName})\n{signerTitle}\nวันที่ {date}",
            false,
        ),
        (
            3,
            "รับทราบ",
            "ทราบ\n({signerName})\n{signerTitle}\nวันที่ {date}",
            false,
        ),
    ]
    .into_iter()
    .map(|(id, name, body, border)| CorrespondenceTextStampTemplate {
        id: Uuid::from_u128(0x0199_ebd0_0000_7000_8000_000000000100 + id),
        name: name.into(),
        body: body.into(),
        border,
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn register_format_is_bounded_and_never_truncates_reserved_numbers() {
        assert_eq!(
            register_label(&CorrespondenceNumberingSettings::default(), 1, 2569).unwrap(),
            "1/2569"
        );
        let value = CorrespondenceNumberingSettings {
            format: "รับ-{number}/{year}".into(),
            minimum_digits: 4,
        };
        assert_eq!(register_label(&value, 1, 2569).unwrap(), "รับ-0001/2569");
        assert_eq!(
            register_label(&value, 12345, 2569).unwrap(),
            "รับ-12345/2569"
        );
        for invalid_format in [
            "{year}",
            "{number}/{number}",
            "{number}/{year}/{year}",
            "{number}/{secret}",
            "{number",
            "{number}}",
            "{number}\n",
        ] {
            assert!(validate_numbering(&CorrespondenceNumberingSettings {
                format: invalid_format.into(),
                minimum_digits: 1
            })
            .is_err());
        }
        for digits in [0, 9, u8::MAX] {
            assert!(validate_numbering(&CorrespondenceNumberingSettings {
                minimum_digits: digits,
                ..value.clone()
            })
            .is_err());
        }
    }

    #[test]
    fn text_stamp_presets_distinguish_registration_and_action_time_and_validate_tokens() {
        let mut presets = default_stamp_templates();
        assert_eq!(presets.len(), 3);
        assert!(presets.iter().all(|p| p.id.get_version_num() == 7));
        assert!(presets[0].body.contains("{registeredDate}"));
        assert!(presets[0].body.contains("{registeredTime}"));
        assert!(!presets[0].body.contains("{date}"));
        assert!(presets[1].body.contains("{instruction}"));
        assert!(presets[1].body.contains("{date}"));
        assert!(presets[2].body.starts_with("ทราบ\n"));
        validate_templates(&presets).unwrap();
        presets[1].body = "{schoolName}{registerNumber}{date}{time}{registeredDate}{registeredTime}{signerName}{signerTitle}{instruction}".into();
        validate_templates(&presets).unwrap();
        presets[1].body.push_str("{authorizationRole}");
        assert!(validate_templates(&presets).is_err());
        presets = default_stamp_templates();
        presets[1].id = presets[0].id;
        assert!(validate_templates(&presets).is_err());
        presets = default_stamp_templates();
        presets[1].id = Uuid::new_v4();
        assert!(validate_templates(&presets).is_err());
    }

    #[test]
    fn pre_reader_settings_and_annotation_payloads_keep_their_serialization() {
        let legacy_settings = r#"{"requestId":"0199ebd0-0000-7000-8000-000000000001","expectedRevision":0,"register":"incoming","eligibleDecisionMakers":[],"defaultFirstRecipient":null,"defaultPurpose":"execute","defaultRequireReview":false,"defaultReviewer":null,"defaultAllowOnwardAssignment":false}"#;
        let request: SaveCorrespondenceRoutingSettingsRequest =
            serde_json::from_str(legacy_settings).unwrap();
        assert_eq!(serde_json::to_string(&request).unwrap(), legacy_settings);
        let legacy_action = r##"{"requestId":"0199ebd0-0000-7000-8000-000000000001","expectedRevision":2,"action":"report","annotations":[{"id":"0199ebd0-0000-7000-8000-000000000002","kind":"text","page":1,"x":0.1,"y":0.1,"width":0.3,"height":0.2,"text":"ทราบ","color":"#000000","strokes":[]}],"comment":"accepted","evidenceFileIds":[],"signature":null,"reassignUserId":null,"reassignFromUserId":null}"##;
        let request: ProcessActionRequest = serde_json::from_str(legacy_action).unwrap();
        assert!(!request.annotations[0].border);
        assert_eq!(serde_json::to_string(&request).unwrap(), legacy_action);
        let mut bordered = request;
        bordered.annotations[0].border = true;
        assert!(serde_json::to_string(&bordered)
            .unwrap()
            .contains("\"border\":true"));
    }
}
